use anyhow::{Context as _, Result};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::{net::Ipv4Addr, time::Duration};

const METADATA_TOKEN_URL: &str =
    "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";
const MANAGED_FIREWALL_DESCRIPTION: &str = "Managed by zunda-bot-rs /7dtd ip";

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

#[derive(Deserialize)]
struct GuestAttributeResponse {
    #[serde(rename = "variableValue")]
    variable_value: Option<String>,
}

#[derive(Deserialize)]
struct FirewallListResponse {
    #[serde(default)]
    items: Vec<FirewallResponse>,
}

#[derive(Deserialize)]
struct FirewallResponse {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default, rename = "sourceRanges")]
    source_ranges: Vec<String>,
}

#[derive(Serialize)]
struct CreateFirewallRequest {
    name: String,
    description: &'static str,
    network: String,
    #[serde(rename = "sourceRanges")]
    source_ranges: Vec<String>,
    #[serde(rename = "targetTags")]
    target_tags: Vec<String>,
    allowed: Vec<FirewallAllowed>,
}

#[derive(Serialize)]
struct FirewallAllowed {
    #[serde(rename = "IPProtocol")]
    ip_protocol: &'static str,
    ports: Vec<&'static str>,
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

    fn firewalls_url(&self) -> String {
        format!(
            "https://compute.googleapis.com/compute/v1/projects/{}/global/firewalls",
            self.project
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
        let value = response
            .error_for_status()
            .context("Compute Engine guest attribute lookup failed")?
            .json::<GuestAttributeResponse>()
            .await?
            .variable_value;
        value.as_deref().map(parse_guest_runtime_state).transpose()
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

    pub async fn allowed_ips(&self) -> Result<Vec<Ipv4Addr>> {
        let response = self
            .http
            .get(self.firewalls_url())
            .bearer_auth(self.token().await?)
            .send()
            .await?
            .error_for_status()
            .context("Compute Engine firewall list failed")?
            .json::<FirewallListResponse>()
            .await?;
        let prefix = self.managed_firewall_prefix();
        let mut ips = response
            .items
            .into_iter()
            .filter(|rule| {
                rule.name.starts_with(&prefix) && rule.description == MANAGED_FIREWALL_DESCRIPTION
            })
            .flat_map(|rule| rule.source_ranges)
            .filter_map(|range| range.strip_suffix("/32").map(str::to_owned))
            .filter_map(|address| address.parse().ok())
            .collect::<Vec<Ipv4Addr>>();
        ips.sort_unstable();
        ips.dedup();
        Ok(ips)
    }

    pub async fn add_allowed_ip(&self, ip: Ipv4Addr, network: &str) -> Result<bool> {
        let request = CreateFirewallRequest {
            name: self.managed_firewall_name(ip),
            description: MANAGED_FIREWALL_DESCRIPTION,
            network: format!("global/networks/{network}"),
            source_ranges: vec![format!("{ip}/32")],
            target_tags: vec![self.instance.clone()],
            allowed: vec![
                FirewallAllowed {
                    ip_protocol: "tcp",
                    ports: vec!["26900"],
                },
                FirewallAllowed {
                    ip_protocol: "udp",
                    ports: vec!["26900-26903"],
                },
            ],
        };
        let response = self
            .http
            .post(self.firewalls_url())
            .bearer_auth(self.token().await?)
            .json(&request)
            .send()
            .await?;
        if response.status() == StatusCode::CONFLICT {
            anyhow::ensure!(
                self.managed_firewall_matches(ip).await?,
                "firewall name is already used by an unmanaged rule"
            );
            return Ok(false);
        }
        response
            .error_for_status()
            .context("Compute Engine firewall creation failed")?;
        Ok(true)
    }

    pub async fn remove_allowed_ip(&self, ip: Ipv4Addr) -> Result<bool> {
        let name = self.managed_firewall_name(ip);
        let Some(rule) = self.firewall(&name).await? else {
            return Ok(false);
        };
        anyhow::ensure!(
            is_managed_firewall_for_ip(&rule, ip),
            "refusing to delete an unmanaged firewall rule"
        );
        let response = self
            .http
            .delete(format!("{}/{name}", self.firewalls_url()))
            .bearer_auth(self.token().await?)
            .send()
            .await?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(false);
        }
        response
            .error_for_status()
            .context("Compute Engine firewall deletion failed")?;
        Ok(true)
    }

    async fn managed_firewall_matches(&self, ip: Ipv4Addr) -> Result<bool> {
        Ok(self
            .firewall(&self.managed_firewall_name(ip))
            .await?
            .as_ref()
            .is_some_and(|rule| is_managed_firewall_for_ip(rule, ip)))
    }

    async fn firewall(&self, name: &str) -> Result<Option<FirewallResponse>> {
        let response = self
            .http
            .get(format!("{}/{name}", self.firewalls_url()))
            .bearer_auth(self.token().await?)
            .send()
            .await?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        Ok(Some(
            response
                .error_for_status()
                .context("Compute Engine firewall lookup failed")?
                .json::<FirewallResponse>()
                .await?,
        ))
    }

    fn managed_firewall_prefix(&self) -> String {
        let instance = self.instance.chars().take(47).collect::<String>();
        format!("{instance}-player-")
    }

    fn managed_firewall_name(&self, ip: Ipv4Addr) -> String {
        format!("{}{:08x}", self.managed_firewall_prefix(), u32::from(ip))
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

fn is_managed_firewall_for_ip(rule: &FirewallResponse, ip: Ipv4Addr) -> bool {
    rule.description == MANAGED_FIREWALL_DESCRIPTION && rule.source_ranges == [format!("{ip}/32")]
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
    fn creates_stable_managed_firewall_name() {
        let client = ComputeClient::new("project".into(), "zone".into(), "zunda-7dtd".into())
            .expect("client should build");
        assert_eq!(
            client.managed_firewall_name(Ipv4Addr::new(1, 2, 3, 4)),
            "zunda-7dtd-player-01020304"
        );
    }

    #[test]
    fn only_matching_firewall_is_managed() {
        let ip = Ipv4Addr::new(8, 8, 8, 8);
        let managed = FirewallResponse {
            name: "zunda-7dtd-player-08080808".into(),
            description: MANAGED_FIREWALL_DESCRIPTION.into(),
            source_ranges: vec!["8.8.8.8/32".into()],
        };
        assert!(is_managed_firewall_for_ip(&managed, ip));

        let unmanaged = FirewallResponse {
            description: "created manually".into(),
            ..managed
        };
        assert!(!is_managed_firewall_for_ip(&unmanaged, ip));
    }
}
