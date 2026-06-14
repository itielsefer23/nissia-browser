use serde::{Deserialize, Serialize};

/// Trait for all CDP commands. Provides type-safe request/response mapping.
pub trait CdpCommand: Serialize {
    type Response: for<'de> Deserialize<'de>;
    fn method_name(&self) -> &'static str;
}

/// A raw CDP message received from the browser.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum CdpMessage {
    Response(CdpResponse),
    Event(CdpEvent),
}

/// A CDP command response (has an `id` field).
#[derive(Debug, Deserialize)]
pub struct CdpResponse {
    pub id: u64,
    pub result: Option<serde_json::Value>,
    pub error: Option<CdpError>,
}

/// A CDP event (has a `method` field, no `id`).
#[derive(Debug, Clone, Deserialize)]
pub struct CdpEvent {
    pub method: String,
    pub params: Option<serde_json::Value>,
}

/// CDP error info.
#[derive(Debug, Deserialize)]
pub struct CdpError {
    pub code: i64,
    pub message: String,
    pub data: Option<String>,
}

impl std::fmt::Display for CdpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CDP error {}: {}", self.code, self.message)
    }
}

/// Browser version info from /json/version endpoint.
#[derive(Debug, Deserialize)]
pub struct BrowserVersion {
    #[serde(rename = "Browser")]
    pub browser: String,
    #[serde(rename = "Protocol-Version")]
    pub protocol_version: String,
    #[serde(rename = "User-Agent")]
    pub user_agent: String,
    #[serde(rename = "V8-Version")]
    pub v8_version: String,
    #[serde(rename = "webSocketDebuggerUrl")]
    pub web_socket_debugger_url: String,
}

/// Target info from /json/list endpoint.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetInfo {
    pub id: String,
    #[serde(rename = "type")]
    pub target_type: String,
    pub title: String,
    pub url: String,
    pub web_socket_debugger_url: Option<String>,
}

// --- Common CDP types used across commands ---

/// A node ID in the DOM tree.
pub type NodeId = i64;

/// A backend node ID (stable across DOM mutations within a page load).
pub type BackendNodeId = i64;

/// A remote object ID from Runtime domain.
pub type RemoteObjectId = String;

/// RGBA color.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a: Option<f64>,
}

/// A bounding box (quad).
#[derive(Debug, Clone, Deserialize)]
pub struct BoxModel {
    pub content: Vec<f64>,
    pub padding: Vec<f64>,
    pub border: Vec<f64>,
    pub margin: Vec<f64>,
    pub width: i64,
    pub height: i64,
}

/// A remote object returned by Runtime.evaluate.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteObject {
    #[serde(rename = "type")]
    pub object_type: String,
    pub subtype: Option<String>,
    pub value: Option<serde_json::Value>,
    pub object_id: Option<RemoteObjectId>,
    pub description: Option<String>,
}

/// Exception details from Runtime.evaluate.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExceptionDetails {
    pub exception_id: i64,
    pub text: String,
    pub line_number: i64,
    pub column_number: i64,
}
