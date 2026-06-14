//! Wait action: wait for navigation, selector, or timeout.

use std::time::Duration;

use nissia_cdp::commands::RuntimeEvaluate;
use nissia_cdp::CdpTransport;

pub enum WaitCondition<'a> {
    Navigation,
    Selector(&'a str),
    Timeout(u64),
}

pub async fn execute(
    transport: &CdpTransport,
    condition: WaitCondition<'_>,
) -> Result<(), nissia_cdp::CdpTransportError> {
    match condition {
        WaitCondition::Navigation => {
            transport
                .wait_for_event("Page.loadEventFired", Duration::from_secs(30))
                .await?;
        }
        WaitCondition::Selector(selector) => {
            let timeout = Duration::from_secs(10);
            let start = std::time::Instant::now();

            loop {
                let js = format!(
                    "document.querySelector({}) !== null",
                    serde_json::to_string(selector).unwrap()
                );
                let resp = transport
                    .send(&RuntimeEvaluate {
                        expression: js,
                        return_by_value: Some(true),
                        await_promise: None,
                        context_id: None,
                    })
                    .await?;

                if resp.result.value == Some(serde_json::Value::Bool(true)) {
                    return Ok(());
                }

                if start.elapsed() > timeout {
                    return Err(nissia_cdp::CdpTransportError::Timeout {
                        method: format!("wait for selector: {selector}"),
                        timeout_ms: timeout.as_millis() as u64,
                    });
                }

                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
        WaitCondition::Timeout(ms) => {
            tokio::time::sleep(Duration::from_millis(ms)).await;
        }
    }

    Ok(())
}
