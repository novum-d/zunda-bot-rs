use crate::data::zunda_bot_database::ZundaBotDatabase;
use crate::models::common::Context;
use crate::models::data::GuildMember;
use crate::models::domain::{MyGuild, MyGuildMember};
use chrono::{DateTime, NaiveDate, Utc};
use poise::serenity_prelude::{GuildId, Http};
use sqlx::PgPool;
use std::sync::Arc;

pub struct GuildRepository {
    db: ZundaBotDatabase,
    http: Arc<Http>,
}

impl GuildRepository {
    pub fn new(pool: Arc<PgPool>, http: Arc<Http>) -> anyhow::Result<Self> {
        let db = ZundaBotDatabase::new(pool)?;
        Ok(GuildRepository { db, http })
    }

    pub async fn get_all_members(&self) -> anyhow::Result<Vec<GuildMember>> {
        let all_members = self.db.select_members().await?;
        Ok(all_members)
    }

    pub async fn get_members_by_guild_id(&self, guild_id: i64) -> anyhow::Result<Vec<GuildMember>> {
        let members_by_guid_id = self.db.select_members_by_guild_id(guild_id).await?;
        Ok(members_by_guid_id)
    }

    pub async fn add_guild(&self, guild_id: i64, guild_name: Option<&str>) -> anyhow::Result<()> {
        self.db.insert_guild(guild_id, guild_name).await?;
        Ok(())
    }

    pub async fn add_member(
        &self,
        guild_id: i64,
        member_id: i64,
        birth: Option<NaiveDate>,
    ) -> anyhow::Result<()> {
        self.db
            .insert_guild_member(guild_id, member_id, birth)
            .await?;
        Ok(())
    }

    pub async fn get_member(
        &self,
        guild_id: i64,
        member_id: i64,
    ) -> anyhow::Result<Option<GuildMember>> {
        self.db.select_member_by_id(guild_id, member_id).await
    }

    pub async fn delete_guild(&self, guild_id: i64) -> anyhow::Result<()> {
        self.db.delete_guild(guild_id).await?;
        Ok(())
    }

    pub async fn delete_member(&self, guild_id: i64, member_id: i64) -> anyhow::Result<()> {
        self.db.delete_guild_member(guild_id, member_id).await?;
        Ok(())
    }

    pub async fn get_guild_ids(&self) -> anyhow::Result<Vec<i64>> {
        let guild_ids = self.db.select_guild_ids().await?;
        Ok(guild_ids)
    }

    pub async fn get_member_birth(
        &self,
        guild_id: i64,
        member_id: i64,
    ) -> anyhow::Result<Option<NaiveDate>, anyhow::Error> {
        let member = self.db.select_member_by_id(guild_id, member_id).await?;
        Ok(member.and_then(|m| m.birth))
    }

    pub async fn update_member_birth(
        &self,
        guild_id: i64,
        member_id: i64,
        birth: NaiveDate,
    ) -> anyhow::Result<()> {
        self.db
            .update_member_birth(guild_id, member_id, birth)
            .await?;

        Ok(())
    }

    pub async fn reset_member_birth(&self, guild_id: i64, member_id: i64) -> anyhow::Result<()> {
        self.db
            .update_member_birth_none(guild_id, member_id)
            .await?;

        Ok(())
    }

    pub async fn update_guild(&self, guild_id: i64, guild_name: &str) -> anyhow::Result<()> {
        self.db.update_guild(guild_id, guild_name).await?;
        Ok(())
    }

    pub async fn update_last_notified(
        &self,
        guild_id: i64,
        member_id: i64,
        last_notified: NaiveDate,
    ) -> anyhow::Result<()> {
        self.db
            .update_guild_member_last_notified(guild_id, member_id, last_notified)
            .await?;
        Ok(())
    }

    pub async fn update_last_active(
        &self,
        guild_id: i64,
        member_id: i64,
        now: DateTime<Utc>,
        first_remind_at: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        self.db
            .update_member_last_active(guild_id, member_id, now, first_remind_at)
            .await?;
        Ok(())
    }

    pub async fn get_active_reminder_candidates(
        &self,
        active_since: DateTime<Utc>,
    ) -> anyhow::Result<Vec<GuildMember>> {
        self.db.select_active_reminder_candidates(active_since).await
    }

    pub async fn update_reminder_sent(
        &self,
        guild_id: i64,
        member_id: i64,
        now: DateTime<Utc>,
        next_remind_at: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        self.db
            .update_member_reminder_sent(guild_id, member_id, now, next_remind_at)
            .await?;
        Ok(())
    }

    pub async fn update_reminder_opt_out(
        &self,
        guild_id: i64,
        member_id: i64,
        is_remind_opt_out: bool,
        next_remind_at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<()> {
        self.db
            .update_member_reminder_opt_out(
                guild_id,
                member_id,
                is_remind_opt_out,
                next_remind_at,
            )
            .await?;
        Ok(())
    }

    pub async fn fetch_my_guild(&self, guild_id: &GuildId) -> anyhow::Result<MyGuild> {
        let partial_guild = self.http.get_guild(*guild_id).await?;
        let members = partial_guild
            .members(&*self.http, None, None)
            .await?
            .into_iter()
            .map(|member| MyGuildMember {
                guild_id: i64::from(member.guild_id),
                member_id: i64::from(member.user.id),
                birth: None,
            })
            .collect::<Vec<MyGuildMember>>();

        Ok(MyGuild {
            id: i64::from(partial_guild.id),
            name: partial_guild.name,
            members,
        })
    }

    pub async fn fetch_guild_id_from_command(
        &self,
        poise_ctx: Context<'_>,
    ) -> anyhow::Result<GuildId> {
        match poise_ctx.guild_id() {
            Some(id) => Ok(id),
            None => {
                let err_msg = "Could not retrieve the Guild ID.";
                tracing::error!(err_msg);
                Err(anyhow::anyhow!(err_msg))
            }
        }
    }

    pub async fn fetch_my_guild_ids(&self) -> anyhow::Result<Vec<GuildId>> {
        let guilds = self.http.get_guilds(None, None).await?;
        Ok(guilds.into_iter().map(|g| g.id).collect())
    }
}
