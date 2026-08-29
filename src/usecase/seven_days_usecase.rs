use crate::data::seven_days_repository::SevenDaysRepository;
use crate::models::seven_days::{SevenDaysAuthorization, SevenDaysOperatorKind};
use crate::services::seven_days_gcp::{ComputeClient, DuckDnsClient, InstanceStatus};
use crate::services::seven_days_operation_lock::SevenDaysOperationLock;
use anyhow::{Context as _, Result};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::{env, sync::Arc, time::Duration};
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
    repository: SevenDaysRepository,
    operation_pool: Arc<PgPool>,
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
        let (duckdns_subdomain, domain) = duckdns_names(&required("SEVEN_DAYS_DUCKDNS_DOMAIN")?)?;
        let repository = SevenDaysRepository::new(operation_pool.clone());
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
            repository,
            operation_pool,
        }))
    }

    pub async fn try_operation_lock(&self) -> Result<Option<SevenDaysOperationLock>> {
        SevenDaysOperationLock::try_acquire(&self.operation_pool).await
    }

    pub async fn authorize(&self, caller: &Caller<'_>, admin: bool) -> Result<bool> {
        let Some(guild_id) = caller.guild_id else {
            return Ok(false);
        };
        let Some(authorization) = self.repository.get_authorization(guild_id).await? else {
            return Ok(false);
        };
        Ok(authorization_allows(&authorization, caller, admin))
    }

    pub async fn can_manage(&self, caller: &Caller<'_>) -> Result<bool> {
        let (Some(guild_id), Some(user_id)) = (caller.guild_id, caller.user_id) else {
            return Ok(false);
        };
        if self.repository.is_guild_admin(guild_id, user_id).await? {
            return Ok(true);
        }
        let Some(authorization) = self.repository.get_authorization(guild_id).await? else {
            return Ok(false);
        };
        Ok(admin_operator_allows(&authorization, caller))
    }

    pub async fn configure(
        &self,
        guild_id: i64,
        channel_id: i64,
        manager_user_id: i64,
    ) -> Result<()> {
        ensure_discord_id(guild_id)?;
        ensure_discord_id(channel_id)?;
        ensure_discord_id(manager_user_id)?;
        self.repository
            .upsert_config(guild_id, channel_id, manager_user_id)
            .await
    }

    pub async fn set_operator(
        &self,
        guild_id: i64,
        kind: SevenDaysOperatorKind,
        operator_id: i64,
        is_admin: bool,
    ) -> Result<()> {
        ensure_discord_id(guild_id)?;
        ensure_discord_id(operator_id)?;
        anyhow::ensure!(
            self.repository.get_authorization(guild_id).await?.is_some(),
            "7DTD configuration is missing"
        );
        self.repository
            .upsert_operator(guild_id, kind, operator_id, is_admin)
            .await
    }

    pub async fn remove_operator(
        &self,
        guild_id: i64,
        kind: SevenDaysOperatorKind,
        operator_id: i64,
    ) -> Result<()> {
        ensure_discord_id(guild_id)?;
        ensure_discord_id(operator_id)?;
        self.repository
            .delete_operator(guild_id, kind, operator_id)
            .await
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

fn ensure_discord_id(value: i64) -> Result<()> {
    anyhow::ensure!(value > 0, "Discord ID must be positive");
    Ok(())
}

fn authorization_allows(
    authorization: &SevenDaysAuthorization,
    caller: &Caller<'_>,
    admin: bool,
) -> bool {
    if !authorization.config.enabled
        || caller.guild_id != Some(authorization.config.guild_id)
        || caller.channel_id != Some(authorization.config.channel_id)
    {
        return false;
    }
    let Some(user_id) = caller.user_id else {
        return false;
    };
    authorization.operators.iter().any(|operator| {
        (!admin || operator.is_admin)
            && operator_matches(
                operator.operator_kind.as_str(),
                operator.operator_id,
                user_id,
                caller.role_ids,
            )
    })
}

fn admin_operator_allows(authorization: &SevenDaysAuthorization, caller: &Caller<'_>) -> bool {
    let Some(user_id) = caller.user_id else {
        return false;
    };
    authorization.operators.iter().any(|operator| {
        operator.is_admin
            && operator_matches(
                operator.operator_kind.as_str(),
                operator.operator_id,
                user_id,
                caller.role_ids,
            )
    })
}

fn operator_matches(kind: &str, operator_id: i64, user_id: i64, role_ids: &[i64]) -> bool {
    match kind {
        "user" => operator_id == user_id,
        "role" => role_ids.contains(&operator_id),
        _ => false,
    }
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
    use crate::models::seven_days::{SevenDaysConfig, SevenDaysOperator};

    fn authorization() -> SevenDaysAuthorization {
        SevenDaysAuthorization {
            config: SevenDaysConfig {
                guild_id: 1,
                channel_id: 2,
                enabled: true,
            },
            operators: vec![
                SevenDaysOperator {
                    guild_id: 1,
                    operator_kind: "user".into(),
                    operator_id: 3,
                    is_admin: false,
                },
                SevenDaysOperator {
                    guild_id: 1,
                    operator_kind: "role".into(),
                    operator_id: 4,
                    is_admin: false,
                },
                SevenDaysOperator {
                    guild_id: 1,
                    operator_kind: "user".into(),
                    operator_id: 5,
                    is_admin: true,
                },
                SevenDaysOperator {
                    guild_id: 1,
                    operator_kind: "role".into(),
                    operator_id: 6,
                    is_admin: true,
                },
            ],
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
        assert!(authorization_allows(&authorization(), &caller, false));
        assert!(!authorization_allows(&authorization(), &caller, true));
        let wrong_channel = Caller {
            channel_id: Some(9),
            ..caller
        };
        assert!(!authorization_allows(
            &authorization(),
            &wrong_channel,
            false
        ));
        let admin = Caller {
            guild_id: Some(1),
            channel_id: Some(2),
            user_id: Some(5),
            role_ids: &[],
        };
        assert!(authorization_allows(&authorization(), &admin, true));
        assert!(authorization_allows(&authorization(), &admin, false));
    }

    #[test]
    fn authorization_accepts_configured_roles_and_rejects_other_guilds() {
        let role_operator = Caller {
            guild_id: Some(1),
            channel_id: Some(2),
            user_id: Some(9),
            role_ids: &[4],
        };
        assert!(authorization_allows(
            &authorization(),
            &role_operator,
            false
        ));
        assert!(!authorization_allows(
            &authorization(),
            &role_operator,
            true
        ));
        let other_guild = Caller {
            guild_id: Some(7),
            ..role_operator
        };
        assert!(!authorization_allows(&authorization(), &other_guild, false));
    }

    #[test]
    fn disabled_configuration_rejects_operators() {
        let mut authorization = authorization();
        authorization.config.enabled = false;
        let admin = Caller {
            guild_id: Some(1),
            channel_id: Some(2),
            user_id: Some(5),
            role_ids: &[],
        };
        assert!(!authorization_allows(&authorization, &admin, true));
        assert!(admin_operator_allows(&authorization, &admin));
    }

    #[test]
    fn admin_role_can_manage_configuration() {
        let caller = Caller {
            guild_id: Some(1),
            channel_id: Some(9),
            user_id: Some(10),
            role_ids: &[6],
        };

        assert!(admin_operator_allows(&authorization(), &caller));
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
