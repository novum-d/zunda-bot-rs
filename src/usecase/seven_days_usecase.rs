use crate::data::seven_days_repository::SevenDaysRepository;
use crate::models::seven_days::{SevenDaysAuthorization, SevenDaysOperatorKind};
use crate::services::seven_days_billing::{BillingClient, CostSummary};
use crate::services::seven_days_gcp::{
    ComputeClient, DuckDnsClient, GuestRuntimeState, InstanceStatus,
};
use crate::services::seven_days_operation_lock::SevenDaysOperationLock;
use anyhow::{Context as _, Result};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::{env, sync::Arc};

#[derive(Clone)]
pub struct SevenDaysUsecase {
    compute: ComputeClient,
    duckdns: DuckDnsClient,
    domain: String,
    port: u16,
    billing: Option<BillingClient>,
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
            billing,
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
        let status = self.compute.status().await?;
        if status.state == "TERMINATED" {
            self.compute.start().await?;
            return Ok(
                "VM の起動を要求したのだ。`/7dtd status` で READY と接続先を確認してほしいのだ。"
                    .into(),
            );
        } else if status.state != "RUNNING" {
            return Ok(format!(
                "VM は現在 {} なのだ。起動処理の完了を待ってほしいのだ。",
                status.state
            ));
        }
        Ok(
            "VM はすでに起動しているのだ。`/7dtd status` で READY と接続先を確認してほしいのだ。"
                .into(),
        )
    }

    pub async fn status(&self) -> Result<String> {
        let status = self.compute.status().await?;
        let uptime = uptime_minutes(&status, Utc::now())
            .map(|minutes| format!("{minutes}分"))
            .unwrap_or_else(|| "なし".into());
        let ready = status.state == "RUNNING" && self.runtime_ready(&status).await?;
        if ready {
            self.duckdns
                .update(
                    status
                        .external_ip
                        .as_deref()
                        .context("READY VM did not have an external IPv4")?,
                )
                .await?;
        }
        let ip = status.external_ip.clone().unwrap_or_else(|| "なし".into());
        let message = format!(
            "VM: {}\nゲーム: {}\n接続先: `{}:{}`\n外部 IPv4: `{}`\n稼働時間: {}",
            status.state,
            if ready { "READY" } else { "NOT READY" },
            self.domain,
            self.port,
            ip,
            uptime
        );
        if status.state == "TERMINATED" {
            Ok(self.with_costs(&message).await)
        } else {
            Ok(message)
        }
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
        Ok("VM の安全停止を要求したのだ。`/7dtd status` で停止完了を確認してほしいのだ。".into())
    }

    async fn runtime_ready(&self, status: &InstanceStatus) -> Result<bool> {
        let Some(runtime) = self.compute.guest_runtime_state().await? else {
            return Ok(false);
        };
        Ok(instance_is_ready(status, &runtime))
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

fn instance_is_ready(status: &InstanceStatus, runtime: &GuestRuntimeState) -> bool {
    if status.state != "RUNNING" || status.external_ip.is_none() || runtime.state != "READY" {
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

    #[test]
    fn ready_state_must_belong_to_current_start() {
        let status = InstanceStatus {
            state: "RUNNING".into(),
            external_ip: Some("203.0.113.10".into()),
            last_start: Some("2026-08-12T03:00:00Z".into()),
        };
        let current = GuestRuntimeState {
            state: "READY".into(),
            boot_id: "new-boot".into(),
            started_at: "2026-08-12T03:00:01Z".into(),
        };
        assert!(instance_is_ready(&status, &current));

        let missing_ip = InstanceStatus {
            external_ip: None,
            ..status.clone()
        };
        assert!(!instance_is_ready(&missing_ip, &current));

        let stale = GuestRuntimeState {
            started_at: "2026-08-11T03:00:00Z".into(),
            ..current
        };
        assert!(!instance_is_ready(&status, &stale));
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
