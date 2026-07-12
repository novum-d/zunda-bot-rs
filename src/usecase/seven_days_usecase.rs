use crate::services::seven_days_gcp::{ComputeClient, DuckDnsClient, InstanceStatus};
use anyhow::{Context as _, Result};
use chrono::{DateTime, Utc};
use std::{collections::HashSet, env, time::Duration};
use tokio::net::TcpStream;

#[derive(Clone)]
pub struct SevenDaysUsecase {
    compute: ComputeClient,
    duckdns: DuckDnsClient,
    domain: String,
    port: u16,
    auth: Authorization,
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
    pub fn from_env() -> Result<Option<Self>> {
        let Some(project) = optional_env("SEVEN_DAYS_GCP_PROJECT") else {
            return Ok(None);
        };
        let required = |name: &str| {
            env::var(name).with_context(|| format!("'{name}' is required when 7DTD is enabled"))
        };
        let parse = |name: &str| -> Result<i64> { Ok(required(name)?.parse()?) };
        let domain = required("SEVEN_DAYS_DUCKDNS_DOMAIN")?;
        Ok(Some(Self {
            compute: ComputeClient::new(
                project,
                required("SEVEN_DAYS_GCP_ZONE")?,
                required("SEVEN_DAYS_GCP_INSTANCE")?,
            )?,
            duckdns: DuckDnsClient::new(domain.clone(), required("SEVEN_DAYS_DUCKDNS_TOKEN")?),
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
        }))
    }

    pub fn authorize(&self, caller: &Caller<'_>, admin: bool) -> bool {
        self.auth.allowed(caller, admin)
    }

    pub async fn start(&self) -> Result<String> {
        let mut status = self.compute.status().await?;
        if status.state == "TERMINATED" {
            self.compute.start().await?;
        } else if status.state != "RUNNING" {
            return Ok(format!(
                "VM は現在 {} なのだ。起動処理の完了を待ってほしいのだ。",
                status.state
            ));
        }
        for _ in 0..30 {
            status = self.compute.status().await?;
            if status.state == "RUNNING" && status.external_ip.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        let ip = status
            .external_ip
            .context("VM started but external IPv4 was unavailable")?;
        self.duckdns.update(&ip).await?;
        for _ in 0..30 {
            if tcp_ready(&ip, self.port).await {
                return Ok(format!(
                    "READY なのだ！ `{}` / `{}:{}`",
                    self.domain, ip, self.port
                ));
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        anyhow::bail!("VM and DuckDNS are ready, but the game port did not become reachable")
    }

    pub async fn status(&self) -> Result<String> {
        let status = self.compute.status().await?;
        let ip = status.external_ip.unwrap_or_else(|| "なし".into());
        let ready = status.state == "RUNNING" && tcp_ready(&ip, self.port).await;
        let uptime = status
            .last_start
            .as_deref()
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|started| (Utc::now() - started.with_timezone(&Utc)).num_minutes())
            .unwrap_or(0);
        Ok(format!(
            "VM: {}\nゲーム: {}\n接続先: `{}:{}`\n外部 IPv4: `{}`\n稼働時間: {}分",
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
}
