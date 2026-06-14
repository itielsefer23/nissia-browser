//! Screenshot action.

use std::path::PathBuf;

use nissia_cdp::commands::PageCaptureScreenshot;
use nissia_cdp::CdpTransport;

pub async fn execute(
    transport: &CdpTransport,
    output_path: Option<&str>,
) -> Result<String, nissia_cdp::CdpTransportError> {
    let resp = transport
        .send(&PageCaptureScreenshot {
            format: Some("png".to_string()),
            quality: None,
        })
        .await?;

    let path = output_path
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::data_dir().join("screenshot.png"));

    let bytes = base64_decode(&resp.data).map_err(|e| {
        nissia_cdp::CdpTransportError::ConnectionFailed(format!("Base64 decode failed: {e}"))
    })?;

    std::fs::write(&path, bytes).map_err(|e| {
        nissia_cdp::CdpTransportError::ConnectionFailed(format!("Failed to write screenshot: {e}"))
    })?;

    Ok(path.display().to_string())
}

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    // Simple base64 decode without pulling in a dependency
    use std::io::Read;
    let mut output = Vec::new();

    // Use a basic implementation
    let chars: Vec<u8> = input
        .bytes()
        .filter(|&b| b != b'\n' && b != b'\r')
        .collect();
    let mut i = 0;

    while i < chars.len() {
        let mut buf = [0u8; 4];
        let mut count = 0;

        while count < 4 && i < chars.len() {
            buf[count] = decode_b64_char(chars[i]);
            count += 1;
            i += 1;
        }

        if count >= 2 {
            output.push((buf[0] << 2) | (buf[1] >> 4));
        }
        if count >= 3 && chars.get(i - 2) != Some(&b'=') {
            output.push((buf[1] << 4) | (buf[2] >> 2));
        }
        if count >= 4 && chars.get(i - 1) != Some(&b'=') {
            output.push((buf[2] << 6) | buf[3]);
        }
    }

    let _ = output.as_slice().read(&mut []);
    Ok(output)
}

fn decode_b64_char(c: u8) -> u8 {
    match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => 0,
    }
}
