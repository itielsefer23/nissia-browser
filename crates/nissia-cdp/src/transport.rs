//! CDP WebSocket transport layer.
//! Handles command/response correlation and event broadcasting.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

use crate::error::CdpResult;
use crate::types::{CdpCommand, CdpEvent, CdpResponse};

type WsStream = WebSocketStream<TcpStream>;

/// A pending command awaiting its response.
struct PendingCommand {
    tx: oneshot::Sender<CdpResponse>,
}

/// CDP transport manages a WebSocket connection to the browser.
pub struct CdpTransport {
    /// Channel to send outgoing WebSocket messages.
    outgoing_tx: mpsc::Sender<Message>,
    /// Monotonically increasing command ID.
    next_id: AtomicU64,
    /// Pending commands awaiting responses, keyed by command ID.
    pending: Arc<Mutex<HashMap<u64, PendingCommand>>>,
    /// Broadcast channel for CDP events.
    event_tx: broadcast::Sender<CdpEvent>,
    /// Handle to the background reader/writer tasks.
    _tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl CdpTransport {
    /// Connect to a CDP WebSocket endpoint (ws:// only, no TLS needed for local Chrome).
    pub async fn connect(ws_url: &str) -> CdpResult<Self> {
        // Parse the URL to extract host:port for TCP connection
        let url = ws_url
            .strip_prefix("ws://")
            .unwrap_or(ws_url.strip_prefix("wss://").unwrap_or(ws_url));
        let host_port = url.split('/').next().unwrap_or("127.0.0.1:9222");

        let tcp_stream = TcpStream::connect(host_port).await.map_err(|e| {
            crate::error::CdpTransportError::ConnectionFailed(format!(
                "Failed to connect to {host_port}: {e}"
            ))
        })?;

        let (ws_stream, _) = tokio_tungstenite::client_async(ws_url, tcp_stream)
            .await
            .map_err(|e| {
                crate::error::CdpTransportError::ConnectionFailed(format!(
                    "WebSocket handshake failed for {ws_url}: {e}"
                ))
            })?;

        Self::from_stream(ws_stream)
    }

    fn from_stream(ws_stream: WsStream) -> CdpResult<Self> {
        use futures_util::{SinkExt, StreamExt};

        let (ws_write, ws_read) = ws_stream.split();
        let pending: Arc<Mutex<HashMap<u64, PendingCommand>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let (event_tx, _) = broadcast::channel(4096);
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<Message>(64);

        // Writer task: forwards messages from the mpsc channel to the WebSocket.
        let writer_handle = tokio::spawn(async move {
            let mut ws_write = ws_write;
            while let Some(msg) = outgoing_rx.recv().await {
                if ws_write.send(msg).await.is_err() {
                    break;
                }
            }
        });

        // Reader task: reads from WebSocket, routes responses and broadcasts events.
        let pending_clone = pending.clone();
        let event_tx_clone = event_tx.clone();
        let reader_handle = tokio::spawn(async move {
            let mut ws_read = ws_read;
            while let Some(Ok(msg)) = ws_read.next().await {
                let Message::Text(text) = msg else {
                    continue;
                };

                // Try to determine if this is a response (has "id") or an event (has "method" but no "id").
                let raw: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                if raw.get("id").is_some() {
                    // This is a response.
                    if let Ok(resp) = serde_json::from_value::<CdpResponse>(raw) {
                        let mut pending = pending_clone.lock().await;
                        if let Some(cmd) = pending.remove(&resp.id) {
                            let _ = cmd.tx.send(resp);
                        }
                    }
                } else if raw.get("method").is_some() {
                    // This is an event.
                    if let Ok(event) = serde_json::from_value::<CdpEvent>(raw) {
                        let _ = event_tx_clone.send(event);
                    }
                }
            }
        });

        Ok(Self {
            outgoing_tx,
            next_id: AtomicU64::new(1),
            pending,
            event_tx,
            _tasks: vec![writer_handle, reader_handle],
        })
    }

    /// Send a CDP command and wait for its response.
    pub async fn send<C: CdpCommand>(&self, command: &C) -> CdpResult<C::Response> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let payload = serde_json::json!({
            "id": id,
            "method": command.method_name(),
            "params": serde_json::to_value(command).unwrap_or(serde_json::Value::Null),
        });

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(id, PendingCommand { tx });
        }

        self.outgoing_tx
            .send(Message::Text(payload.to_string()))
            .await
            .map_err(|_| {
                crate::error::CdpTransportError::ConnectionFailed(
                    "WebSocket send channel closed".into(),
                )
            })?;

        let resp = rx.await.map_err(|_| {
            crate::error::CdpTransportError::ConnectionFailed(
                "Response channel dropped (connection closed?)".into(),
            )
        })?;

        if let Some(err) = resp.error {
            return Err(crate::error::CdpTransportError::CommandFailed {
                method: command.method_name().to_string(),
                code: err.code,
                message: err.message,
            });
        }

        let result = resp.result.unwrap_or(serde_json::Value::Null);
        serde_json::from_value(result).map_err(|e| {
            crate::error::CdpTransportError::DeserializationFailed {
                method: command.method_name().to_string(),
                message: e.to_string(),
            }
        })
    }

    /// Subscribe to CDP events.
    pub fn subscribe_events(&self) -> broadcast::Receiver<CdpEvent> {
        self.event_tx.subscribe()
    }

    /// Wait for a specific CDP event by method name.
    pub async fn wait_for_event(
        &self,
        method: &str,
        timeout: std::time::Duration,
    ) -> CdpResult<CdpEvent> {
        let mut rx = self.subscribe_events();
        let method_owned = method.to_string();
        let method_for_err = method_owned.clone();

        tokio::time::timeout(timeout, async move {
            loop {
                match rx.recv().await {
                    Ok(event) if event.method == method_owned => return Ok(event),
                    Ok(_) => continue,
                    // Lagged means we missed some events but channel is still open — keep going
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        return Err(crate::error::CdpTransportError::ConnectionFailed(
                            "Event channel closed".into(),
                        ))
                    }
                }
            }
        })
        .await
        .map_err(|_| crate::error::CdpTransportError::Timeout {
            method: method_for_err,
            timeout_ms: timeout.as_millis() as u64,
        })?
    }
}

// Need futures_util for stream splitting
// Add to Cargo.toml: futures-util = "0.3"
