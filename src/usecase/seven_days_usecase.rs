use crate::services::seven_days_billing::{BillingClient, CostSummary};
use crate::services::seven_days_gcp::{
    ComputeClient, DuckDnsClient, GuestRuntimeState, InstanceStatus,
};
use crate::services::seven_days_operation_lock::SevenDaysOperationLock;
use anyhow::{Context as _, Result};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::{collections::HashSet, env, sync::Arc, time::Duration};
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
    auth: Authorization,
    billing: Option<BillingClient>,
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
        let billing = optional_env("SEVEN_DAYS_BILLING_DATASET")
            .map(|dataset| {
                BillingClient::new(
                    optional_env("SEVEN_DAYS_BILLING_PROJECT").unwrap_or_else(|| project.clone()),
                    dataset,
                    required("SEVEN_DAYS_BILLING_TABLE")?,
                    project.clone(),
                    optional_env("SEVEN_DAYS_BILLING_MAX_BYTES")
                        .unwrap_or_else(|| "100000000".into())
                        .parse()?,
                )
            })
            .transpose()?;
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
            billing,
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
            return Ok(self.with_costs("VM はすでに停止しているのだ。").await);
        }
        anyhow::ensure!(
            state == "RUNNING",
            "VM is {state}; refusing a duplicate stop request"
        );
        self.compute.stop().await?;
        let deadline = Instant::now() + STOP_TIMEOUT;
        while Instant::now() < deadline {
            if self.compute.status().await?.state == "TERMINATED" {
                return Ok(self
                    .with_costs("ワールドを保存して VM を安全に停止したのだ。")
                    .await);
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
        Ok("安全停止はまだ処理中なのだ。`/7dtd status` で停止完了を確認してほしいのだ。".into())
    }

    async fn runtime_ready(&self, status: &InstanceStatus) -> Result<bool> {
        let Some(runtime) = self.compute.guest_runtime_state().await? else {
            return Ok(false);
        };
        Ok(guest_runtime_is_current_and_ready(status, &runtime))
    }

    async fn with_costs(&self, message: &str) -> String {
        let Some(billing) = &self.billing else {
            return format!("{message}\n料金情報は設定されていないのだ。");
        };
        match billing.costs().await {
            Ok(costs) => format!("{message}\n{}", format_costs(&costs)),
            Err(error) => {
                tracing::warn!(error = %error, "7DTD billing query failed after stop");
                format!(
                    "{message}\n料金情報は取得できなかったのだ。Billing export を確認してほしいのだ。"
                )
            }
        }
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

fn format_costs(costs: &CostSummary) -> String {
    let exported_at = costs
        .exported_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| {
            value
                .with_timezone(&chrono_tz::Asia::Tokyo)
                .format("%Y-%m-%d %H:%M JST")
                .to_string()
        })
        .unwrap_or_else(|| "不明".into());
    format!(
        "料金（反映済み概算）: 今月 {} {:.0} / 今年 {} {:.0}\n集計反映時点: {}\n※直近の利用分はまだ反映されていない場合があるのだ。",
        costs.currency, costs.monthly, costs.currency, costs.yearly, exported_at
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
    fn formats_billing_costs_with_export_time() {
        assert_eq!(
            format_costs(&CostSummary {
                monthly: 1234.4,
                yearly: 5678.6,
                currency: "JPY".into(),
                exported_at: Some("2026-08-12T03:00:00Z".into()),
            }),
            "料金（反映済み概算）: 今月 JPY 1234 / 今年 JPY 5679\n集計反映時点: 2026-08-12 12:00 JST\n※直近の利用分はまだ反映されていない場合があるのだ。"
        );
    }
}
