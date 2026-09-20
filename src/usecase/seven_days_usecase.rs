use crate::services::seven_days_gcp::{
    ComputeClient, DuckDnsClient, GuestRuntimeState, InstanceStatus,
};
use crate::services::seven_days_operation_lock::SevenDaysOperationLock;
use crate::services::seven_days_secret::SecretManagerClient;
use anyhow::{Context as _, Result};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::{collections::HashSet, env, net::Ipv4Addr, sync::Arc, time::Duration};
use tokio::time::Instant;

const START_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const POLL_INTERVAL: Duration = Duration::from_secs(5);
const STOP_TIMEOUT: Duration = Duration::from_secs(2 * 60);

#[derive(Clone)]
pub struct SevenDaysUsecase {
    compute: ComputeClient,
    duckdns: DuckDnsClient,
    domain: String,
    port: u16,
    network: String,
    max_allowed_ips: usize,
    server_password: SecretManagerClient,
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
        let server_password = SecretManagerClient::new(
            project.clone(),
            optional_env("SEVEN_DAYS_SERVER_PASSWORD_SECRET_ID")
                .unwrap_or_else(|| "SEVEN_DAYS_SERVER_PASSWORD".into()),
        )?;
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
            network: optional_env("SEVEN_DAYS_GCP_NETWORK").unwrap_or_else(|| "default".into()),
            max_allowed_ips: optional_env("SEVEN_DAYS_MAX_ALLOWED_IPS")
                .unwrap_or_else(|| "16".into())
                .parse()?,
            server_password,
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
            .clone()
            .context("VM started but external IPv4 was unavailable")?;
        self.duckdns.update(&ip).await?;
        while Instant::now() < deadline {
            if self.runtime_ready(&status).await? {
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
        let ready = status.state == "RUNNING" && self.runtime_ready(&status).await?;
        let ip = status.external_ip.unwrap_or_else(|| "なし".into());
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
        let deadline = Instant::now() + STOP_TIMEOUT;
        while Instant::now() < deadline {
            if self.compute.status().await?.state == "TERMINATED" {
                return Ok("ワールドを保存して VM を安全に停止したのだ。".into());
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
        Ok("安全停止はまだ処理中なのだ。`/7dtd status` で停止完了を確認してほしいのだ。".into())
    }

    pub async fn list_allowed_ips(&self) -> Result<String> {
        let ips = self.compute.allowed_ips().await?;
        if ips.is_empty() {
            return Ok("Bot が管理する接続許可IPはないのだ。".into());
        }
        Ok(format!(
            "接続許可IPなのだ（{}件）:\n{}",
            ips.len(),
            ips.iter()
                .map(|ip| format!("- `{ip}`"))
                .collect::<Vec<String>>()
                .join("\n")
        ))
    }

    pub async fn add_allowed_ip(&self, value: &str) -> Result<String> {
        let ip = allowed_ipv4(value)?;
        let current = self.compute.allowed_ips().await?;
        let already_allowed = current.contains(&ip);
        if !already_allowed {
            anyhow::ensure!(
                current.len() < self.max_allowed_ips,
                "allowed IP limit ({}) was reached",
                self.max_allowed_ips
            );
        }
        // Firewallを変更する前に取得し、パスワードを表示できない中途半端な成功を避ける。
        let password = self.server_password.latest().await?;
        let added = if already_allowed {
            false
        } else {
            self.compute.add_allowed_ip(ip, &self.network).await?
        };
        Ok(format_allowed_ip_add(
            ip,
            &self.domain,
            self.port,
            &password,
            added,
        ))
    }

    pub async fn remove_allowed_ip(&self, value: &str) -> Result<String> {
        let ip = allowed_ipv4(value)?;
        if self.compute.remove_allowed_ip(ip).await? {
            Ok(format!("`{ip}` を接続許可IPから削除したのだ。"))
        } else {
            Ok("そのIPはすでに許可されていないのだ。".into())
        }
    }

    async fn runtime_ready(&self, status: &InstanceStatus) -> Result<bool> {
        let Some(runtime) = self.compute.guest_runtime_state().await? else {
            return Ok(false);
        };
        Ok(guest_runtime_is_current_and_ready(status, &runtime))
    }
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

fn guest_runtime_is_current_and_ready(
    status: &InstanceStatus,
    runtime: &GuestRuntimeState,
) -> bool {
    if status.state != "RUNNING" || runtime.state != "READY" {
        return false;
    }
    let Some(instance_started) = status
        .last_start
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
    else {
        return false;
    };
    let Some(runtime_started) = DateTime::parse_from_rfc3339(&runtime.started_at).ok() else {
        return false;
    };
    runtime_started >= instance_started
}

fn allowed_ipv4(value: &str) -> Result<Ipv4Addr> {
    let value = value.trim();
    let ip = value
        .strip_suffix("/32")
        .unwrap_or(value)
        .parse::<Ipv4Addr>()
        .context("address must be an IPv4 address")?;
    anyhow::ensure!(is_global_ipv4(ip), "address must be a global IPv4 address");
    Ok(ip)
}

fn format_allowed_ip_add(
    ip: Ipv4Addr,
    domain: &str,
    port: u16,
    password: &str,
    added: bool,
) -> String {
    let result = if added {
        format!("`{ip}` を接続許可IPへ追加したのだ。")
    } else {
        "そのIPはすでに許可されているのだ。".into()
    };
    format!(
        "{result}\n接続先: `{domain}:{port}`\nサーバーパスワード: `{password}`\n※管理者だけが受け取るエフェメラル応答なのだ。"
    )
}

fn is_global_ipv4(ip: Ipv4Addr) -> bool {
    let [first, second, third, _] = ip.octets();
    !matches!(
        (first, second, third),
        (0, _, _)
            | (10, _, _)
            | (100, 64..=127, _)
            | (127, _, _)
            | (169, 254, _)
            | (172, 16..=31, _)
            | (192, 0, 0)
            | (192, 0, 2)
            | (192, 88, 99)
            | (192, 168, _)
            | (198, 18..=19, _)
            | (198, 51, 100)
            | (203, 0, 113)
            | (224..=255, _, _)
    )
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

    #[test]
    fn ready_state_must_belong_to_current_start() {
        let status = InstanceStatus {
            state: "RUNNING".into(),
            external_ip: None,
            last_start: Some("2026-08-12T03:00:00Z".into()),
        };
        let current = GuestRuntimeState {
            state: "READY".into(),
            boot_id: "new-boot".into(),
            started_at: "2026-08-12T03:00:01Z".into(),
        };
        assert!(guest_runtime_is_current_and_ready(&status, &current));

        let stale = GuestRuntimeState {
            started_at: "2026-08-11T03:00:00Z".into(),
            ..current
        };
        assert!(!guest_runtime_is_current_and_ready(&status, &stale));
    }

    #[test]
    fn accepts_only_global_ipv4_addresses() {
        assert_eq!(
            allowed_ipv4("8.8.8.8/32").expect("public IPv4 should be accepted"),
            Ipv4Addr::new(8, 8, 8, 8)
        );
        for address in [
            "10.0.0.1",
            "127.0.0.1",
            "169.254.1.1",
            "192.0.2.1",
            "224.0.0.1",
            "255.255.255.255",
        ] {
            assert!(allowed_ipv4(address).is_err(), "{address} must be rejected");
        }
    }

    #[test]
    fn formats_password_after_allowed_ip_add() {
        assert_eq!(
            format_allowed_ip_add(
                Ipv4Addr::new(8, 8, 8, 8),
                "zunda-7dtd.duckdns.org",
                26900,
                "0123456789abcdef",
                true,
            ),
            "`8.8.8.8` を接続許可IPへ追加したのだ。\n接続先: `zunda-7dtd.duckdns.org:26900`\nサーバーパスワード: `0123456789abcdef`\n※管理者だけが受け取るエフェメラル応答なのだ。"
        );
    }
}
