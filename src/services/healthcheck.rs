use crate::reminder::service::ReminderService;
use std::env;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub async fn run_healthcheck_server(reminder_service: ReminderService) -> anyhow::Result<()> {
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let port: u16 = port.parse()?;
    run_healthcheck_server_on(port, reminder_service).await
}

pub async fn run_healthcheck_server_on(
    port: u16,
    reminder_service: ReminderService,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await?;

    loop {
        let (mut stream, _) = listener.accept().await?;
        let mut buffer = [0_u8; 1024];
        let read_size = stream.read(&mut buffer).await?;
        let request = String::from_utf8_lossy(&buffer[..read_size]);
        let response = handle_request(&request, &reminder_service).await;

        if let Err(e) = stream.write_all(response.as_bytes()).await {
            tracing::warn!("Failed to write healthcheck response: {}", e);
        }
    }
}

pub(crate) fn response_bytes() -> &'static [u8] {
    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK"
}

async fn handle_request(request: &str, reminder_service: &ReminderService) -> String {
    let request_line = request.lines().next().unwrap_or_default();
    if request_line.starts_with("POST /internal/reminder/scan ") {
        return match reminder_service.scan_and_send().await {
            Ok(sent_count) => json_response(200, &format!(r#"{{"sent":{sent_count}}}"#)),
            Err(e) => {
                tracing::error!("birthday reminder scan failed: {}", e);
                json_response(500, r#"{"error":"reminder scan failed"}"#)
            }
        };
    }

    String::from_utf8_lossy(response_bytes()).to_string()
}

fn json_response(status: u16, body: &str) -> String {
    let status_text = match status {
        200 => "OK",
        500 => "Internal Server Error",
        _ => "OK",
    };
    format!(
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[cfg(test)]
mod tests {
    use super::{json_response, response_bytes};

    #[test]
    fn response_bytes_returns_http_200_ok() {
        let response = response_bytes();

        assert!(response.starts_with(b"HTTP/1.1 200 OK"));
        assert!(response.ends_with(b"\r\n\r\nOK"));
    }

    #[test]
    fn json_response_sets_content_length() {
        let response = json_response(200, r#"{"sent":1}"#);

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("Content-Length: 10"));
        assert!(response.ends_with(r#"{"sent":1}"#));
    }
}
