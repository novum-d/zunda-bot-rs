// DB接続や初期化など、DB全体の管理を担当

use crate::models::data::GuildMember;
use chrono::NaiveDate;
use sqlx::PgPool;
use std::sync::Arc;

pub struct ZundaBotDatabase {
    pool: Arc<PgPool>,
}

impl ZundaBotDatabase {
    pub fn new(pool: Arc<PgPool>) -> anyhow::Result<Self, sqlx::Error> {
        Ok(ZundaBotDatabase { pool })
    }

    pub async fn select_guild_ids(&self) -> anyhow::Result<Vec<i64>> {
        let guild_ids = sqlx::query_scalar!(
            r#"
        SELECT guild_id::BIGINT FROM guild
        "#
        )
        .fetch_all(&*self.pool)
        .await?
        .into_iter()
        .filter_map(|guild_id: i64| Some(guild_id))
        .collect::<Vec<i64>>();
        Ok(guild_ids)
    }

    pub async fn select_members(&self) -> anyhow::Result<Vec<GuildMember>> {
        let rows = sqlx::query_as!(GuildMember, r#"SELECT * FROM guild_member"#)
            .fetch_all(&*self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn select_members_by_guild_id(
        &self,
        guild_id: i64,
    ) -> anyhow::Result<Vec<GuildMember>> {
        let rows = sqlx::query_as!(
            GuildMember,
            r#"SELECT * FROM guild_member WHERE guild_id = $1"#,
            guild_id
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn select_member_by_id(
        &self,
        guild_id: i64,
        member_id: i64,
    ) -> anyhow::Result<Option<GuildMember>> {
        let row = sqlx::query_as!(
            GuildMember,
            "SELECT * FROM guild_member WHERE guild_id = $1 AND member_id = $2",
            guild_id,
            member_id
        )
        .fetch_optional(&*self.pool)
        .await?;
        Ok(row)
    }

    pub async fn update_guild(&self, guild_id: i64, guild_name: &str) -> anyhow::Result<()> {
        let guild_id: i64 = guild_id.try_into()?;

        sqlx::query!(
            r#"
        UPDATE guild
        SET name = $1
        WHERE guild_id = $2
        "#,
            guild_name,
            guild_id,
        )
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
        sqlx::query!(
            r#"
        UPDATE guild_member
        SET birth = $1
        WHERE guild_id = $2 AND member_id = $3
        "#,
            birth,
            guild_id,
            member_id,
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_member_birth_none(
        &self,
        guild_id: i64,
        member_id: i64,
    ) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
        UPDATE guild_member
        SET birth = NULL, last_notified = NULL
        WHERE guild_id = $1 AND member_id = $2
        "#,
            guild_id,
            member_id,
        )
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
        sqlx::query!(
            r#"
        UPDATE guild_member
        SET last_notified = $1
        WHERE guild_id = $2 AND member_id = $3
        "#,
            last_notified,
            guild_id,
            member_id,
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_guild(&self, guild_id: i64) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
        DELETE FROM guild_member
        WHERE guild_id = $1
        "#,
            guild_id,
        )
        .execute(&*self.pool)
        .await?;

        sqlx::query!(
            r#"
        DELETE FROM guild
        WHERE guild_id = $1
        "#,
            guild_id,
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_guild_member(&self, guild_id: i64, member_id: i64) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
        DELETE FROM guild_member
        WHERE guild_id = $1 AND member_id = $2
        "#,
            guild_id,
            member_id,
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn insert_guild(
        &self,
        guild_id: i64,
        guild_name: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
        INSERT INTO guild (guild_id, name)
        VALUES ($1, $2)
        ON CONFLICT (guild_id) DO NOTHING
        "#,
            guild_id,
            guild_name,
        )
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
        sqlx::query!(
            r#"
        INSERT INTO guild_member (guild_id, member_id, birth)
        VALUES ($1, $2, $3)
        ON CONFLICT (guild_id, member_id) DO NOTHING
        "#,
            guild_id,
            member_id,
            birth,
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }
}
