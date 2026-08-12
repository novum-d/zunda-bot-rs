use crate::services::seven_days_gcp::{ComputeClient, DuckDnsClient, InstanceStatus};
use crate::services::seven_days_operation_lock::SevenDaysOperationLock;
use anyhow::{Context as _, Result};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::{collections::HashSet, env, sync::Arc, time::Duration};
use tokio::net::TcpStream;
use tokio::time::Instant;

const START_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const POLL_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct SevenDaysUsecase {
    compute: ComputeClient,
    duckdns: DuckDnsClient,
    domain: String,
    port: u16,
    auth: Authorization,
    operation_pool: Arc<PgPool>,
}

#[derive(Clone)]
struct Authorization {
    guild_id: i64,
    channel_id: i64,
    users: HashSet<i64>,
    roles: HashSet<i64>,
    admin_users: HashSet<i64>,
    admin_roles: HashSet<i64>,
}

impl Authorization {
    fn allowed(&self, caller: &Caller<'_>, admin: bool) -> bool {
        if caller.guild_id != Some(self.guild_id) || caller.channel_id != Some(self.channel_id) {
            return false;
        }
        let Some(user) = caller.user_id else {
            return false;
        };
        let base = self.users.contains(&user)
            || caller.role_ids.iter().any(|role| self.roles.contains(role));
        let elevated = self.admin_users.contains(&user)
            || caller
                .role_ids
                .iter()
                .any(|role| self.admin_roles.contains(role));
        elevated || (!admin && base)
    }
}

pub struct Caller<'a> {
    pub guild_id: Option<i64>,
    pub channel_id: Option<i64>,
    pub user_id: Option<i64>,
    pub role_ids: &'a [i64],
}

impl SevenDaysUsecase {
    pub fn from_env(operation_pool: Arc<PgPool>) -> Result<Option<Self>> {
        let Some(project) = optional_env("SEVEN_DAYS_GCP_PROJECT") else {
            return Ok(None);
        };
        let required = |name: &str| {
            env::var(name).with_context(|| format!("'{name}' is required when 7DTD is enabled"))
        };
        let parse = |name: &str| -> Result<i64> { Ok(required(name)?.parse()?) };
        let (duckdns_subdomain, domain) = duckdns_names(&required("SEVEN_DAYS_DUCKDNS_DOMAIN")?)?;
        Ok(Some(Self {
            compute: ComputeClient::new(
                project,
                required("SEVEN_DAYS_GCP_ZONE")?,
                required("SEVEN_DAYS_GCP_INSTANCE")?,
            )?,
            duckdns: DuckDnsClient::new(duckdns_subdomain, required("SEVEN_DAYS_DUCKDNS_TOKEN")?),
            domain,
            port: optional_env("SEVEN_DAYS_PORT")
                .unwrap_or_else(|| "26900".into())
                .parse()?,
            auth: Authorization {
                guild_id: parse("SEVEN_DAYS_DISCORD_GUILD_ID")?,
                channel_id: parse("SEVEN_DAYS_DISCORD_CHANNEL_ID")?,
                users: id_set("SEVEN_DAYS_DISCORD_USER_IDS")?,
                roles: id_set("SEVEN_DAYS_DISCORD_ROLE_IDS")?,
                admin_users: id_set("SEVEN_DAYS_DISCORD_ADMIN_USER_IDS")?,
                admin_roles: id_set("SEVEN_DAYS_DISCORD_ADMIN_ROLE_IDS")?,
            },
            operation_pool,
        }))
    }

    pub async fn try_operation_lock(&self) -> Result<Option<SevenDaysOperationLock>> {
        SevenDaysOperationLock::try_acquire(&self.operation_pool).await
    }

    pub fn authorize(&self, caller: &Caller<'_>, admin: bool) -> bool {
        self.auth.allowed(caller, admin)
    }

    pub async fn start(&self) -> Result<String> {
        let deadline = Instant::now() + START_TIMEOUT;
        let mut status = self.compute.status().await?;
        if status.state == "TERMINATED" {
            self.compute.start().await?;
        } else if status.state != "RUNNING" {
            return Ok(format!(
                "VM は現在 {} なのだ。起動処理の完了を待ってほしいのだ。",
                status.state
            ));
        }
        while Instant::now() < deadline {
            status = self.compute.status().await?;
            if status.state == "RUNNING" && status.external_ip.is_some() {
                break;
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
        let ip = status
            .external_ip
            .context("VM started but external IPv4 was unavailable")?;
        self.duckdns.update(&ip).await?;
        while Instant::now() < deadline {
            if tcp_ready(&ip, self.port).await {
                return Ok(format!(
                    "READY なのだ！ `{}` / `{}:{}`",
                    self.domain, ip, self.port
                ));
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
        anyhow::bail!("VM and DuckDNS are ready, but the game port did not become reachable")
    }

    pub async fn status(&self) -> Result<String> {
        let status = self.compute.status().await?;
        let uptime = uptime_minutes(&status, Utc::now())
            .map(|minutes| format!("{minutes}分"))
            .unwrap_or_else(|| "なし".into());
        let ip = status.external_ip.unwrap_or_else(|| "なし".into());
        let ready = status.state == "RUNNING" && tcp_ready(&ip, self.port).await;
        Ok(format!(
            "VM: {}\nゲーム: {}\n接続先: `{}:{}`\n外部 IPv4: `{}`\n稼働時間: {}",
            status.state,
            if ready { "READY" } else { "NOT READY" },
            self.domain,
            self.port,
            ip,
            uptime
        ))
    }

    pub async fn stop(&self) -> Result<String> {
        let InstanceStatus { state, .. } = self.compute.status().await?;
        if state == "TERMINATED" {
            return Ok("VM はすでに停止しているのだ。".into());
        }
        anyhow::ensure!(
            state == "RUNNING",
            "VM is {state}; refusing a duplicate stop request"
        );
        self.compute.stop().await?;
        Ok("安全停止を要求したのだ。systemd がワールド保存と正常終了を行ってから VM を停止するのだ。".into())
    }
}

async fn tcp_ready(ip: &str, port: u16) -> bool {
    tokio::time::timeout(Duration::from_secs(2), TcpStream::connect((ip, port)))
        .await
        .map(|r| r.is_ok())
        .unwrap_or(false)
}
fn optional_env(name: &str) -> Option<String> {
    env::var(name).ok().filter(|v| !v.trim().is_empty())
}
fn id_set(name: &str) -> Result<HashSet<i64>> {
    optional_env(name)
        .unwrap_or_default()
        .split(',')
        .filter(|v| !v.trim().is_empty())
        .map(|v| v.trim().parse().map_err(Into::into))
        .collect()
}

fn duckdns_names(value: &str) -> Result<(String, String)> {
    let value = value.trim().trim_end_matches('.').to_ascii_lowercase();
    let subdomain = value.strip_suffix(".duckdns.org").unwrap_or(&value);
    anyhow::ensure!(
        !subdomain.is_empty() && !subdomain.contains('.'),
        "SEVEN_DAYS_DUCKDNS_DOMAIN must be a DuckDNS subdomain or FQDN"
    );
    anyhow::ensure!(
        subdomain
            .chars()
            .all(|character| character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '-'),
        "SEVEN_DAYS_DUCKDNS_DOMAIN contains invalid characters"
    );
    Ok((subdomain.into(), format!("{subdomain}.duckdns.org")))
}

fn uptime_minutes(status: &InstanceStatus, now: DateTime<Utc>) -> Option<i64> {
    if status.state != "RUNNING" {
        return None;
    }
    status
        .last_start
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|started| (now - started.with_timezone(&Utc)).num_minutes().max(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn auth() -> Authorization {
        Authorization {
            guild_id: 1,
            channel_id: 2,
            users: HashSet::from([3]),
            roles: HashSet::from([4]),
            admin_users: HashSet::from([5]),
            admin_roles: HashSet::from([6]),
        }
    }
    #[test]
    fn authorization_requires_location_and_id() {
        let caller = Caller {
            guild_id: Some(1),
            channel_id: Some(2),
            user_id: Some(3),
            role_ids: &[],
        };
        assert!(auth().allowed(&caller, false));
        assert!(!auth().allowed(&caller, true));
        let wrong_channel = Caller {
            channel_id: Some(9),
            ..caller
        };
        assert!(!auth().allowed(&wrong_channel, false));
        let admin = Caller {
            guild_id: Some(1),
            channel_id: Some(2),
            user_id: Some(5),
            role_ids: &[],
        };
        assert!(auth().allowed(&admin, true));
        assert!(auth().allowed(&admin, false));
    }

    #[test]
    fn normalizes_duckdns_subdomain_and_fqdn() {
        assert_eq!(
            duckdns_names("Zunda-7DTD.duckdns.org.").expect("domain should be valid"),
            ("zunda-7dtd".into(), "zunda-7dtd.duckdns.org".into())
        );
        assert_eq!(
            duckdns_names("zunda-7dtd").expect("subdomain should be valid"),
            ("zunda-7dtd".into(), "zunda-7dtd.duckdns.org".into())
        );
        assert!(duckdns_names("example.com").is_err());
    }

    #[test]
    fn uptime_is_only_reported_for_running_instance() {
        let now = DateTime::parse_from_rfc3339("2026-08-12T03:00:00Z")
            .expect("timestamp should parse")
            .with_timezone(&Utc);
        let running = InstanceStatus {
            state: "RUNNING".into(),
            external_ip: None,
            last_start: Some("2026-08-12T01:30:00Z".into()),
        };
        assert_eq!(uptime_minutes(&running, now), Some(90));

        let stopped = InstanceStatus {
            state: "TERMINATED".into(),
            ..running
        };
        assert_eq!(uptime_minutes(&stopped, now), None);
    }
}
