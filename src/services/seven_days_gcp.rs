use anyhow::{Context as _, Result};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use std::time::Duration;

const METADATA_TOKEN_URL: &str =
    "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";

#[derive(Clone)]
pub struct ComputeClient {
    http: Client,
    project: String,
    zone: String,
    instance: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceStatus {
    pub state: String,
    pub external_ip: Option<String>,
    pub last_start: Option<String>,
}

#[derive(Deserialize)]
struct AccessToken {
    access_token: String,
}

#[derive(Deserialize)]
struct InstanceResponse {
    status: String,
    #[serde(rename = "lastStartTimestamp")]
    last_start_timestamp: Option<String>,
    #[serde(default)]
    network_interfaces: Vec<NetworkInterface>,
}

#[derive(Deserialize)]
struct NetworkInterface {
    #[serde(default)]
    access_configs: Vec<AccessConfig>,
}

#[derive(Deserialize)]
struct AccessConfig {
    #[serde(rename = "natIP")]
    nat_i_p: Option<String>,
}

impl ComputeClient {
    pub fn new(project: String, zone: String, instance: String) -> Result<Self> {
        let http = Client::builder().timeout(Duration::from_secs(15)).build()?;
        Ok(Self {
            http,
            project,
            zone,
            instance,
        })
    }

    async fn token(&self) -> Result<String> {
        Ok(self
            .http
            .get(METADATA_TOKEN_URL)
            .header("Metadata-Flavor", "Google")
            .send()
            .await?
            .error_for_status()?
            .json::<AccessToken>()
            .await?
            .access_token)
    }

    fn instance_url(&self) -> String {
        format!(
            "https://compute.googleapis.com/compute/v1/projects/{}/zones/{}/instances/{}",
            self.project, self.zone, self.instance
        )
    }

    pub async fn status(&self) -> Result<InstanceStatus> {
        let response = self
            .http
            .get(self.instance_url())
            .bearer_auth(self.token().await?)
            .send()
            .await?
            .error_for_status()
            .context("Compute Engine instance lookup failed")?
            .json::<InstanceResponse>()
            .await?;
        let external_ip = response
            .network_interfaces
            .first()
            .and_then(|interface| interface.access_configs.first())
            .and_then(|config| config.nat_i_p.clone());
        Ok(InstanceStatus {
            state: response.status,
            external_ip,
            last_start: response.last_start_timestamp,
        })
    }

    pub async fn start(&self) -> Result<()> {
        self.action("start").await
    }
    pub async fn stop(&self) -> Result<()> {
        self.action("stop").await
    }

    async fn action(&self, action: &str) -> Result<()> {
        let response = self
            .http
            .post(format!("{}/{}", self.instance_url(), action))
            .bearer_auth(self.token().await?)
            .send()
            .await?;
        if response.status() != StatusCode::OK {
            response.error_for_status()?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct DuckDnsClient {
    http: Client,
    domain: String,
    token: String,
}

impl DuckDnsClient {
    pub fn new(domain: String, token: String) -> Self {
        Self {
            http: Client::new(),
            domain,
            token,
        }
    }
    pub async fn update(&self, ip: &str) -> Result<()> {
        let response = self
            .http
            .get("https://www.duckdns.org/update")
            .query(&[
                ("domains", self.domain.as_str()),
                ("token", self.token.as_str()),
                ("ip", ip),
            ])
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        anyhow::ensure!(response.trim() == "OK", "DuckDNS update was rejected");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_compute_external_ipv4() {
        let response: InstanceResponse = serde_json::from_str(
            r#"{"status":"RUNNING","lastStartTimestamp":"2026-01-01T00:00:00Z","network_interfaces":[{"access_configs":[{"natIP":"203.0.113.10"}]}]}"#,
        )
        .expect("Compute response should parse");

        assert_eq!(response.status, "RUNNING");
        assert_eq!(
            response.network_interfaces[0].access_configs[0]
                .nat_i_p
                .as_deref(),
            Some("203.0.113.10")
        );
    }
}
