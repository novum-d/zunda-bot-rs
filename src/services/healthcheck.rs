use std::env;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

pub async fn run_healthcheck_server() -> anyhow::Result<()> {
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let port: u16 = port.parse()?;
    run_healthcheck_server_on(port).await
}

pub async fn run_healthcheck_server_on(port: u16) -> anyhow::Result<()> {
    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await?;

    loop {
        let (mut stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = stream.write_all(response_bytes()).await {
                tracing::warn!("Failed to write healthcheck response: {}", e);
            }
        });
    }
}

pub(crate) fn response_bytes() -> &'static [u8] {
    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK"
}

#[cfg(test)]
mod tests {
    use super::response_bytes;

    #[test]
    fn response_bytes_returns_http_200_ok() {
        let response = response_bytes();

        assert!(response.starts_with(b"HTTP/1.1 200 OK"));
        assert!(response.ends_with(b"\r\n\r\nOK"));
    }
}
