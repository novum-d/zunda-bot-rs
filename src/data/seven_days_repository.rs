use crate::models::seven_days::{
    SevenDaysAuthorization, SevenDaysConfig, SevenDaysOperator, SevenDaysOperatorKind,
};
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct SevenDaysRepository {
    pool: Arc<PgPool>,
}

impl SevenDaysRepository {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    pub async fn get_authorization(
        &self,
        guild_id: i64,
    ) -> anyhow::Result<Option<SevenDaysAuthorization>> {
        let config = sqlx::query_as::<_, SevenDaysConfig>(
            r#"
            SELECT guild_id, channel_id, enabled
            FROM seven_days_config
            WHERE guild_id = $1
            "#,
        )
        .bind(guild_id)
        .fetch_optional(&*self.pool)
        .await?;
        let Some(config) = config else {
            return Ok(None);
        };
        let operators = sqlx::query_as::<_, SevenDaysOperator>(
            r#"
            SELECT guild_id, operator_kind, operator_id, is_admin
            FROM seven_days_operator
            WHERE guild_id = $1
            "#,
        )
        .bind(guild_id)
        .fetch_all(&*self.pool)
        .await?;
        Ok(Some(SevenDaysAuthorization { config, operators }))
    }

    pub async fn is_guild_admin(&self, guild_id: i64, member_id: i64) -> anyhow::Result<bool> {
        Ok(sqlx::query_scalar::<_, bool>(
            r#"
            SELECT COALESCE(BOOL_OR(is_admin), FALSE)
            FROM guild_member
            WHERE guild_id = $1 AND member_id = $2
            "#,
        )
        .bind(guild_id)
        .bind(member_id)
        .fetch_one(&*self.pool)
        .await?)
    }

    pub async fn upsert_config(
        &self,
        guild_id: i64,
        channel_id: i64,
        manager_user_id: i64,
    ) -> anyhow::Result<()> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            r#"
            INSERT INTO guild (guild_id, name)
            VALUES ($1, $2)
            ON CONFLICT (guild_id) DO NOTHING
            "#,
        )
        .bind(guild_id)
        .bind(format!("guild-{guild_id}"))
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO seven_days_config (guild_id, channel_id, enabled)
            VALUES ($1, $2, TRUE)
            ON CONFLICT (guild_id)
            DO UPDATE SET channel_id = EXCLUDED.channel_id, enabled = TRUE
            "#,
        )
        .bind(guild_id)
        .bind(channel_id)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO seven_days_operator (guild_id, operator_kind, operator_id, is_admin)
            VALUES ($1, 'user', $2, TRUE)
            ON CONFLICT (guild_id, operator_kind, operator_id)
            DO UPDATE SET is_admin = TRUE
            "#,
        )
        .bind(guild_id)
        .bind(manager_user_id)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn upsert_operator(
        &self,
        guild_id: i64,
        kind: SevenDaysOperatorKind,
        operator_id: i64,
        is_admin: bool,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO seven_days_operator (guild_id, operator_kind, operator_id, is_admin)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (guild_id, operator_kind, operator_id)
            DO UPDATE SET is_admin = EXCLUDED.is_admin
            "#,
        )
        .bind(guild_id)
        .bind(kind.as_str())
        .bind(operator_id)
        .bind(is_admin)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_operator(
        &self,
        guild_id: i64,
        kind: SevenDaysOperatorKind,
        operator_id: i64,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            DELETE FROM seven_days_operator
            WHERE guild_id = $1 AND operator_kind = $2 AND operator_id = $3
            "#,
        )
        .bind(guild_id)
        .bind(kind.as_str())
        .bind(operator_id)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }
}
