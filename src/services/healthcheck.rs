use crate::reminder::service::ReminderService;
use std::env;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub async fn run_healthcheck_server(reminder_service: ReminderService) -> anyhow::Result<()> {
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let port: u16 = port.parse()?;
    run_healthcheck_server_on(port, Some(reminder_service)).await
}

pub async fn run_passive_healthcheck_server() -> anyhow::Result<()> {
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let port: u16 = port.parse()?;
    run_healthcheck_server_on(port, None).await
}

pub async fn run_healthcheck_server_on(
    port: u16,
    reminder_service: Option<ReminderService>,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    tracing::info!("Healthcheck server listening on 0.0.0.0:{}", port);

    loop {
        let (mut stream, _) = listener.accept().await?;
        let reminder_service = reminder_service.clone();

        tokio::spawn(async move {
            let mut buffer = [0_u8; 1024];
            // タイムアウトを設定して、リクエストを送ってこない接続でリソースが枯渇するのを防ぐ
            let read_result =
                tokio::time::timeout(std::time::Duration::from_secs(5), stream.read(&mut buffer))
                    .await;

            let read_size = match read_result {
                Ok(Ok(0)) => return, // Connection closed
                Ok(Ok(n)) => n,
                Ok(Err(e)) => {
                    tracing::warn!("Failed to read from stream: {}", e);
                    return;
                }
                Err(_) => {
                    tracing::debug!("Healthcheck request timed out");
                    return;
                }
            };

            let request = String::from_utf8_lossy(&buffer[..read_size]);
            tracing::debug!(%request, "received healthcheck request");
            let response = handle_request(&request, reminder_service.as_ref()).await;
            tracing::debug!(?response, "healthcheck response");

            if let Err(e) = stream.write_all(response.as_bytes()).await {
                tracing::warn!("Failed to write healthcheck response: {}", e);
            }

            // 明示的にフラッシュとシャットダウンを行い、クライアントにレスポンス完了を伝える
            let _ = stream.flush().await;
            let _ = stream.shutdown().await;
        });
    }
}

pub(crate) fn response_bytes() -> &'static [u8] {
    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK"
}

async fn handle_request(request: &str, reminder_service: Option<&ReminderService>) -> String {
    let request_line = request.lines().next().unwrap_or_default();
    if request_line.starts_with("POST /internal/reminder/scan ") {
        let Some(reminder_service) = reminder_service else {
            return json_response(503, r#"{"error":"reminder scan disabled"}"#);
        };
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
        503 => "Service Unavailable",
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
