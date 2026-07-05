use crate::reminder::service::ReminderService;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::Deserialize;
use std::env;
use std::str;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const DISCORD_PUBLIC_KEY_ENV: &str = "DISCORD_PUBLIC_KEY";
const MAX_REQUEST_BYTES: usize = 64 * 1024;

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
            let request = match read_http_request(&mut stream).await {
                Ok(Some(request)) => request,
                Ok(None) => return,
                Err(e) => {
                    tracing::warn!("Failed to read healthcheck request: {}", e);
                    return;
                }
            };

            let request_head = request_head_for_log(&request);
            tracing::debug!(%request_head, "received healthcheck request");
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

async fn read_http_request(stream: &mut TcpStream) -> anyhow::Result<Option<Vec<u8>>> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 2048];

    loop {
        let read_result =
            tokio::time::timeout(std::time::Duration::from_secs(5), stream.read(&mut buffer)).await;

        let read_size = match read_result {
            Ok(Ok(0)) if request.is_empty() => return Ok(None),
            Ok(Ok(0)) => return Ok(Some(request)),
            Ok(Ok(n)) => n,
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => anyhow::bail!("request read timed out"),
        };

        request.extend_from_slice(&buffer[..read_size]);
        if request.len() > MAX_REQUEST_BYTES {
            anyhow::bail!("request exceeded {} bytes", MAX_REQUEST_BYTES);
        }

        if request_is_complete(&request) {
            return Ok(Some(request));
        }
    }
}

async fn handle_request(request: &[u8], reminder_service: Option<&ReminderService>) -> String {
    let Some(request) = HttpRequest::parse(request) else {
        return json_response(400, r#"{"error":"bad request"}"#);
    };

    if request.method == "POST" && request.path == "/interactions" {
        return handle_discord_interaction(&request);
    }

    if request.method == "POST" && request.path == "/internal/reminder/scan" {
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

fn handle_discord_interaction(request: &HttpRequest<'_>) -> String {
    if let Err(e) = verify_discord_signature(request) {
        tracing::warn!("Discord interaction signature verification failed: {}", e);
        return json_response(401, r#"{"error":"invalid request signature"}"#);
    }

    let interaction = match serde_json::from_slice::<DiscordInteraction>(request.body) {
        Ok(interaction) => interaction,
        Err(e) => {
            tracing::warn!("Discord interaction JSON parse failed: {}", e);
            return json_response(400, r#"{"error":"bad request"}"#);
        }
    };

    match interaction.interaction_type {
        1 => json_response(200, r#"{"type":1}"#),
        _ => json_response(400, r#"{"error":"unsupported interaction type"}"#),
    }
}

fn verify_discord_signature(request: &HttpRequest<'_>) -> Result<(), &'static str> {
    let public_key = env::var(DISCORD_PUBLIC_KEY_ENV)
        .map_err(|_| "DISCORD_PUBLIC_KEY is not configured")
        .and_then(|value| decode_fixed_hex::<32>(value.trim()))?;
    let signature = request
        .header("X-Signature-Ed25519")
        .ok_or("X-Signature-Ed25519 header is missing")
        .and_then(decode_fixed_hex::<64>)?;
    let timestamp = request
        .header("X-Signature-Timestamp")
        .ok_or("X-Signature-Timestamp header is missing")?;

    let verifying_key =
        VerifyingKey::from_bytes(&public_key).map_err(|_| "public key is invalid")?;
    let signature = Signature::from_bytes(&signature);
    let mut message = Vec::with_capacity(timestamp.len() + request.body.len());
    message.extend_from_slice(timestamp.as_bytes());
    message.extend_from_slice(request.body);

    verifying_key
        .verify(&message, &signature)
        .map_err(|_| "signature is invalid")
}

fn decode_fixed_hex<const N: usize>(value: &str) -> Result<[u8; N], &'static str> {
    let mut bytes = [0_u8; N];
    hex::decode_to_slice(value, &mut bytes).map_err(|_| "hex value is invalid")?;
    Ok(bytes)
}

fn request_is_complete(request: &[u8]) -> bool {
    let Some(header_end) = header_end(request) else {
        return false;
    };
    let header_text = match str::from_utf8(&request[..header_end]) {
        Ok(header_text) => header_text,
        Err(_) => return true,
    };
    let content_length = parse_content_length(header_text).unwrap_or(0);

    request.len() >= header_end + 4 + content_length
}

fn header_end(request: &[u8]) -> Option<usize> {
    request.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_content_length(header_text: &str) -> Option<usize> {
    header_text.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.eq_ignore_ascii_case("Content-Length") {
            value.trim().parse().ok()
        } else {
            None
        }
    })
}

fn request_head_for_log(request: &[u8]) -> String {
    let head_end = header_end(request).unwrap_or(request.len());
    String::from_utf8_lossy(&request[..head_end]).to_string()
}

fn json_response(status: u16, body: &str) -> String {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        503 => "Service Unavailable",
        500 => "Internal Server Error",
        _ => "OK",
    };
    format!(
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[derive(Debug)]
struct HttpRequest<'a> {
    method: &'a str,
    path: &'a str,
    headers: Vec<(&'a str, &'a str)>,
    body: &'a [u8],
}

impl<'a> HttpRequest<'a> {
    fn parse(request: &'a [u8]) -> Option<Self> {
        let header_end = header_end(request)?;
        let header_text = str::from_utf8(&request[..header_end]).ok()?;
        let mut lines = header_text.lines();
        let mut request_line = lines.next()?.split_whitespace();
        let method = request_line.next()?;
        let path = request_line.next()?;
        let headers = lines
            .filter_map(|line| {
                let (name, value) = line.split_once(':')?;
                Some((name.trim(), value.trim()))
            })
            .collect();
        let body = &request[header_end + 4..];

        Some(Self {
            method,
            path,
            headers,
            body,
        })
    }

    fn header(&self, name: &str) -> Option<&'a str> {
        self.headers.iter().find_map(|(header_name, value)| {
            header_name.eq_ignore_ascii_case(name).then_some(*value)
        })
    }
}

#[derive(Debug, Deserialize)]
struct DiscordInteraction {
    #[serde(rename = "type")]
    interaction_type: u8,
}

#[cfg(test)]
mod tests {
    use super::{
        handle_request, json_response, request_is_complete, response_bytes, DISCORD_PUBLIC_KEY_ENV,
    };
    use ed25519_dalek::{Signer, SigningKey};
    use std::env;

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

    #[test]
    fn request_is_complete_uses_content_length() {
        let partial = b"POST /interactions HTTP/1.1\r\nContent-Length: 10\r\n\r\n{\"type\":1";
        let complete = b"POST /interactions HTTP/1.1\r\nContent-Length: 10\r\n\r\n{\"type\":1}";

        assert!(!request_is_complete(partial));
        assert!(request_is_complete(complete));
    }

    #[tokio::test]
    async fn discord_ping_interaction_returns_pong_after_signature_verification() {
        let request = signed_interaction_request(r#"{"type":1}"#);

        let response = handle_request(request.as_bytes(), None).await;

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.ends_with(r#"{"type":1}"#));
    }

    #[tokio::test]
    async fn discord_interaction_rejects_invalid_signature() {
        let body = r#"{"type":1}"#;
        let mut request = signed_interaction_request(body);
        request = request.replace(body, r#"{"type":2}"#);

        let response = handle_request(request.as_bytes(), None).await;

        assert!(response.starts_with("HTTP/1.1 401 Unauthorized"));
        assert!(response.ends_with(r#"{"error":"invalid request signature"}"#));
    }

    #[tokio::test]
    async fn discord_interaction_returns_error_for_unsupported_type() {
        let request = signed_interaction_request(r#"{"type":2}"#);

        let response = handle_request(request.as_bytes(), None).await;

        assert!(response.starts_with("HTTP/1.1 400 Bad Request"));
        assert!(response.ends_with(r#"{"error":"unsupported interaction type"}"#));
    }

    fn signed_interaction_request(body: &str) -> String {
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let verifying_key = signing_key.verifying_key();
        env::set_var(
            DISCORD_PUBLIC_KEY_ENV,
            hex::encode(verifying_key.to_bytes()),
        );

        let timestamp = "1700000000";
        let mut message = Vec::new();
        message.extend_from_slice(timestamp.as_bytes());
        message.extend_from_slice(body.as_bytes());
        let signature = signing_key.sign(&message);

        format!(
            "POST /interactions HTTP/1.1\r\nHost: example.com\r\nX-Signature-Ed25519: {}\r\nX-Signature-Timestamp: {timestamp}\r\nContent-Length: {}\r\n\r\n{body}",
            hex::encode(signature.to_bytes()),
            body.len()
        )
    }
}
