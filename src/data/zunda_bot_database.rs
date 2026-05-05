// DB接続や初期化など、DB全体の管理を担当

use crate::models::data::GuildMember;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct ZundaBotDatabase {
    pool: Arc<PgPool>,
}

impl ZundaBotDatabase {
    pub fn new(pool: Arc<PgPool>) -> anyhow::Result<Self, sqlx::Error> {
        Ok(ZundaBotDatabase { pool })
    }

    pub async fn select_guild_ids(&self) -> anyhow::Result<Vec<i64>> {
        let guild_ids = sqlx::query_scalar::<_, i64>(
            r#"
        SELECT guild_id::BIGINT FROM guild
        "#,
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(guild_ids)
    }

    pub async fn select_members(&self) -> anyhow::Result<Vec<GuildMember>> {
        let rows = sqlx::query_as::<_, GuildMember>(r#"SELECT * FROM guild_member"#)
            .fetch_all(&*self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn select_members_by_guild_id(
        &self,
        guild_id: i64,
    ) -> anyhow::Result<Vec<GuildMember>> {
        let rows =
            sqlx::query_as::<_, GuildMember>(r#"SELECT * FROM guild_member WHERE guild_id = $1"#)
                .bind(guild_id)
                .fetch_all(&*self.pool)
                .await?;
        Ok(rows)
    }

    pub async fn select_member_by_id(
        &self,
        guild_id: i64,
        member_id: i64,
    ) -> anyhow::Result<Option<GuildMember>> {
        let row = sqlx::query_as::<_, GuildMember>(
            "SELECT * FROM guild_member WHERE guild_id = $1 AND member_id = $2",
        )
        .bind(guild_id)
        .bind(member_id)
        .fetch_optional(&*self.pool)
        .await?;
        Ok(row)
    }

    pub async fn select_member_is_admin(&self, member_id: i64) -> anyhow::Result<bool> {
        let is_admin = sqlx::query_scalar::<_, bool>(
            r#"
        SELECT COALESCE(BOOL_OR(is_admin), FALSE)
        FROM guild_member
        WHERE member_id = $1
        "#,
        )
        .bind(member_id)
        .fetch_one(&*self.pool)
        .await?;
        Ok(is_admin)
    }

    pub async fn update_guild(&self, guild_id: i64, guild_name: &str) -> anyhow::Result<()> {
        sqlx::query(
            r#"
        UPDATE guild
        SET name = $1
        WHERE guild_id = $2
        "#,
        )
        .bind(guild_name)
        .bind(guild_id)
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_member_birth(
        &self,
        guild_id: i64,
        member_id: i64,
        birth: NaiveDate,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
        UPDATE guild_member
        SET birth = $1
        WHERE guild_id = $2 AND member_id = $3
        "#,
        )
        .bind(birth)
        .bind(guild_id)
        .bind(member_id)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_member_birth_none(
        &self,
        guild_id: i64,
        member_id: i64,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
        UPDATE guild_member
        SET birth = NULL, last_notified = NULL
        WHERE guild_id = $1 AND member_id = $2
        "#,
        )
        .bind(guild_id)
        .bind(member_id)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_guild_member_last_notified(
        &self,
        guild_id: i64,
        member_id: i64,
        last_notified: NaiveDate,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
        UPDATE guild_member
        SET last_notified = $1
        WHERE guild_id = $2 AND member_id = $3
        "#,
        )
        .bind(last_notified)
        .bind(guild_id)
        .bind(member_id)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_guild(&self, guild_id: i64) -> anyhow::Result<()> {
        sqlx::query(
            r#"
        UPDATE guild_member
        SET reminder_guild_id = NULL
        WHERE reminder_guild_id = $1
        "#,
        )
        .bind(guild_id)
        .execute(&*self.pool)
        .await?;

        sqlx::query(
            r#"
        DELETE FROM guild_member
        WHERE guild_id = $1
        "#,
        )
        .bind(guild_id)
        .execute(&*self.pool)
        .await?;

        sqlx::query(
            r#"
        DELETE FROM guild
        WHERE guild_id = $1
        "#,
        )
        .bind(guild_id)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_guild_member(&self, guild_id: i64, member_id: i64) -> anyhow::Result<()> {
        sqlx::query(
            r#"
        DELETE FROM guild_member
        WHERE guild_id = $1 AND member_id = $2
        "#,
        )
        .bind(guild_id)
        .bind(member_id)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn insert_guild(
        &self,
        guild_id: i64,
        guild_name: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
        INSERT INTO guild (guild_id, name)
        VALUES ($1, $2)
        ON CONFLICT (guild_id) DO NOTHING
        "#,
        )
        .bind(guild_id)
        .bind(guild_name)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn insert_guild_member(
        &self,
        guild_id: i64,
        member_id: i64,
        birth: Option<NaiveDate>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
        INSERT INTO guild_member (guild_id, member_id, birth)
        VALUES ($1, $2, $3)
        ON CONFLICT (guild_id, member_id) DO NOTHING
        "#,
        )
        .bind(guild_id)
        .bind(member_id)
        .bind(birth)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_member_last_active(
        &self,
        guild_id: i64,
        member_id: i64,
        now: DateTime<Utc>,
        first_remind_at: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
        UPDATE guild_member
        SET last_active_at = $1,
            next_remind_at = COALESCE(next_remind_at, $2)
        WHERE guild_id = $3 AND member_id = $4
        "#,
        )
        .bind(now)
        .bind(first_remind_at)
        .bind(guild_id)
        .bind(member_id)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_member_manual_reminder_target(
        &self,
        guild_id: i64,
        member_id: i64,
        now: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
        UPDATE guild_member
        SET reminder_guild_id = $1,
            last_active_at = $3,
            next_remind_at = $3
        WHERE guild_id = $1 AND member_id = $2
        "#,
        )
        .bind(guild_id)
        .bind(member_id)
        .bind(now)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn select_active_reminder_candidates(
        &self,
        active_since: DateTime<Utc>,
    ) -> anyhow::Result<Vec<GuildMember>> {
        let rows = sqlx::query_as::<_, GuildMember>(
            r#"
        SELECT
            destination.guild_id,
            destination.member_id,
            destination.birth,
            destination.last_notified,
            active_user.last_active_at,
            destination.last_reminded_at,
            destination.next_remind_at,
            destination.remind_count,
            destination.is_remind_opt_out,
            destination.reminder_guild_id
        FROM guild_member destination
        JOIN (
            SELECT
                member_id,
                reminder_guild_id,
                MAX(last_active_at) AS last_active_at
            FROM guild_member
            WHERE reminder_guild_id IS NOT NULL
            GROUP BY member_id, reminder_guild_id
        ) active_user
            ON destination.member_id = active_user.member_id
            AND destination.guild_id = active_user.reminder_guild_id
        WHERE active_user.last_active_at >= $1
        ORDER BY guild_id, member_id
        "#,
        )
        .bind(active_since)
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn select_active_reminder_candidate_by_member_id(
        &self,
        member_id: i64,
        active_since: DateTime<Utc>,
    ) -> anyhow::Result<Option<GuildMember>> {
        let row = sqlx::query_as::<_, GuildMember>(
            r#"
        SELECT
            destination.guild_id,
            destination.member_id,
            destination.birth,
            destination.last_notified,
            active_user.last_active_at,
            destination.last_reminded_at,
            destination.next_remind_at,
            destination.remind_count,
            destination.is_remind_opt_out,
            destination.reminder_guild_id
        FROM guild_member destination
        JOIN (
            SELECT
                member_id,
                reminder_guild_id,
                MAX(last_active_at) AS last_active_at
            FROM guild_member
            WHERE member_id = $1
              AND reminder_guild_id IS NOT NULL
            GROUP BY member_id, reminder_guild_id
        ) active_user
            ON destination.member_id = active_user.member_id
            AND destination.guild_id = active_user.reminder_guild_id
        WHERE active_user.last_active_at >= $2
        "#,
        )
        .bind(member_id)
        .bind(active_since)
        .fetch_optional(&*self.pool)
        .await?;
        Ok(row)
    }

    pub async fn update_member_reminder_sent(
        &self,
        guild_id: i64,
        member_id: i64,
        now: DateTime<Utc>,
        next_remind_at: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
        UPDATE guild_member
        SET last_reminded_at = $1,
            remind_count = remind_count + 1,
            next_remind_at = $2
        WHERE guild_id = $3 AND member_id = $4
        "#,
        )
        .bind(now)
        .bind(next_remind_at)
        .bind(guild_id)
        .bind(member_id)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_member_reminder_message(
        &self,
        guild_id: i64,
        member_id: i64,
        channel_id: i64,
        message_id: i64,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
        UPDATE guild_member
        SET reminder_channel_id = $1,
            reminder_message_id = $2
        WHERE guild_id = $3 AND member_id = $4
        "#,
        )
        .bind(channel_id)
        .bind(message_id)
        .bind(guild_id)
        .bind(member_id)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn select_member_reminder_message(
        &self,
        guild_id: i64,
        member_id: i64,
    ) -> anyhow::Result<Option<(i64, i64)>> {
        let row = sqlx::query_as::<_, (i64, i64)>(
            r#"
        SELECT reminder_channel_id, reminder_message_id
        FROM guild_member
        WHERE guild_id = $1
          AND member_id = $2
          AND reminder_channel_id IS NOT NULL
          AND reminder_message_id IS NOT NULL
        "#,
        )
        .bind(guild_id)
        .bind(member_id)
        .fetch_optional(&*self.pool)
        .await?;
        Ok(row)
    }

    pub async fn clear_member_reminder_message(
        &self,
        guild_id: i64,
        member_id: i64,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
        UPDATE guild_member
        SET reminder_channel_id = NULL,
            reminder_message_id = NULL
        WHERE guild_id = $1 AND member_id = $2
        "#,
        )
        .bind(guild_id)
        .bind(member_id)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_member_reminder_opt_out(
        &self,
        guild_id: i64,
        member_id: i64,
        is_remind_opt_out: bool,
        next_remind_at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
        UPDATE guild_member
        SET is_remind_opt_out = $1,
            next_remind_at = $2
        WHERE guild_id = $3 AND member_id = $4
        "#,
        )
        .bind(is_remind_opt_out)
        .bind(next_remind_at)
        .bind(guild_id)
        .bind(member_id)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }
}
