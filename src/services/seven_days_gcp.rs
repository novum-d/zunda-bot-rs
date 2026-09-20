use anyhow::{Context as _, Result};
use reqwest::{header::CONTENT_LENGTH, Client, RequestBuilder, StatusCode};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestRuntimeState {
    pub state: String,
    pub boot_id: String,
    pub started_at: String,
}

#[derive(Deserialize)]
struct AccessToken {
    access_token: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceResponse {
    status: String,
    last_start_timestamp: Option<String>,
    #[serde(default)]
    network_interfaces: Vec<NetworkInterface>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NetworkInterface {
    #[serde(default)]
    access_configs: Vec<AccessConfig>,
}

#[derive(Deserialize)]
struct AccessConfig {
    #[serde(rename = "natIP")]
    nat_i_p: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GuestAttributeResponse {
    query_value: Option<GuestAttributeQueryValue>,
}

#[derive(Deserialize)]
struct GuestAttributeQueryValue {
    #[serde(default)]
    items: Vec<GuestAttributeItem>,
}

#[derive(Deserialize)]
struct GuestAttributeItem {
    namespace: String,
    key: String,
    value: String,
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

    pub async fn guest_runtime_state(&self) -> Result<Option<GuestRuntimeState>> {
        let response = self
            .http
            .get(format!("{}/getGuestAttributes", self.instance_url()))
            .query(&[("queryPath", "seven-days/runtime-state")])
            .bearer_auth(self.token().await?)
            .send()
            .await?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let response = response
            .error_for_status()
            .context("Compute Engine guest attribute lookup failed")?
            .json::<GuestAttributeResponse>()
            .await?;
        parse_guest_attribute_response(response)
    }

    pub async fn start(&self) -> Result<()> {
        self.action("start").await
    }
    pub async fn stop(&self) -> Result<()> {
        self.action("stop").await
    }

    async fn action(&self, action: &str) -> Result<()> {
        let response = self
            .action_request(action, &self.token().await?)
            .send()
            .await?;
        if response.status() != StatusCode::OK {
            response.error_for_status()?;
        }
        Ok(())
    }

    fn action_request(&self, action: &str, token: &str) -> RequestBuilder {
        self.http
            .post(format!("{}/{}", self.instance_url(), action))
            .bearer_auth(token)
            .header(CONTENT_LENGTH, 0)
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
            .await
            .map_err(redact_request_url)?
            .error_for_status()
            .map_err(redact_request_url)?
            .text()
            .await
            .map_err(redact_request_url)?;
        anyhow::ensure!(response.trim() == "OK", "DuckDNS update was rejected");
        Ok(())
    }
}

fn redact_request_url(error: reqwest::Error) -> anyhow::Error {
    anyhow::Error::new(error.without_url())
}

fn parse_guest_runtime_state(value: &str) -> Result<GuestRuntimeState> {
    let mut fields = value.split('|');
    let state = fields.next().unwrap_or_default();
    let boot_id = fields.next().unwrap_or_default();
    let started_at = fields.next().unwrap_or_default();
    anyhow::ensure!(
        !state.is_empty()
            && !boot_id.is_empty()
            && !started_at.is_empty()
            && fields.next().is_none(),
        "invalid 7DTD guest runtime state"
    );
    Ok(GuestRuntimeState {
        state: state.into(),
        boot_id: boot_id.into(),
        started_at: started_at.into(),
    })
}

fn parse_guest_attribute_response(
    response: GuestAttributeResponse,
) -> Result<Option<GuestRuntimeState>> {
    response
        .query_value
        .into_iter()
        .flat_map(|query| query.items)
        .find(|item| item.namespace == "seven-days" && item.key == "runtime-state")
        .map(|item| parse_guest_runtime_state(&item.value))
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_compute_external_ipv4() {
        let response: InstanceResponse = serde_json::from_str(
            r#"{"status":"RUNNING","lastStartTimestamp":"2026-01-01T00:00:00Z","networkInterfaces":[{"accessConfigs":[{"natIP":"203.0.113.10"}]}]}"#,
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

    #[test]
    fn parses_guest_runtime_state() {
        assert_eq!(
            parse_guest_runtime_state("READY|boot-id|2026-08-12T03:00:00Z")
                .expect("guest state should parse"),
            GuestRuntimeState {
                state: "READY".into(),
                boot_id: "boot-id".into(),
                started_at: "2026-08-12T03:00:00Z".into(),
            }
        );
        assert!(parse_guest_runtime_state("READY|boot-id").is_err());
    }

    #[test]
    fn parses_guest_attribute_api_response() {
        let response: GuestAttributeResponse = serde_json::from_str(
            r#"{
                "queryPath": "seven-days/runtime-state",
                "queryValue": {
                    "items": [{
                        "namespace": "seven-days",
                        "key": "runtime-state",
                        "value": "READY|boot-id|2026-09-20T16:08:12Z"
                    }]
                }
            }"#,
        )
        .expect("guest attribute response should deserialize");

        assert_eq!(
            parse_guest_attribute_response(response).expect("guest runtime state should parse"),
            Some(GuestRuntimeState {
                state: "READY".into(),
                boot_id: "boot-id".into(),
                started_at: "2026-09-20T16:08:12Z".into(),
            })
        );
    }

    #[test]
    fn compute_action_sends_an_explicit_empty_body_length() {
        let client = ComputeClient::new(
            "test-project".into(),
            "asia-northeast1-b".into(),
            "test-instance".into(),
        )
        .expect("Compute client should be created");

        let request = client
            .action_request("start", "test-token")
            .build()
            .expect("Compute action request should be built");

        assert_eq!(request.method(), reqwest::Method::POST);
        assert_eq!(request.headers().get(CONTENT_LENGTH).unwrap(), "0");
        assert_eq!(
            request.url().as_str(),
            "https://compute.googleapis.com/compute/v1/projects/test-project/zones/asia-northeast1-b/instances/test-instance/start"
        );
    }
}
