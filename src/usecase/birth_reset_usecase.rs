use crate::data::guild_repository::GuildRepository;
use serenity::all::Http;
use sqlx::PgPool;
use std::sync::Arc;

pub const WEBHOOK_RESET_BUTTON_PREFIX: &str = "birth_reset_confirm";

#[derive(Clone)]
pub struct BirthResetUsecase {
    guild_repo: GuildRepository,
}

impl BirthResetUsecase {
    pub fn new(pool: Arc<PgPool>, http: Arc<Http>) -> anyhow::Result<Self> {
        let guild_repo = GuildRepository::new(pool, http.clone())?;
        Ok(BirthResetUsecase { guild_repo })
    }

    pub async fn build_confirmation_view(
        &self,
        guild_id: i64,
        guild_name: Option<&str>,
        member_id: i64,
    ) -> anyhow::Result<BirthResetConfirmationView> {
        // 初回参加メンバーでも参照できるよう、対象レコードを事前に作成しておく
        self.guild_repo.add_guild(guild_id, guild_name).await?;
        self.guild_repo
            .add_member(guild_id, member_id, None)
            .await?;

        // ギルドIDとメンバーIDに一致するメンバーの誕生日をguild_memberテーブルから取得
        let member_birth = self
            .guild_repo
            .get_member_birth(guild_id, member_id)
            .await?;

        Ok(BirthResetConfirmationView {
            has_birth: member_birth.is_some(),
        })
    }

    pub async fn reset_member_birth(&self, guild_id: i64, member_id: i64) -> anyhow::Result<()> {
        self.guild_repo
            .reset_member_birth(guild_id, member_id)
            .await?;
        Ok(())
    }
}

pub struct BirthResetConfirmationView {
    pub has_birth: bool,
}

pub fn webhook_reset_button_custom_id(guild_id: i64, member_id: i64) -> String {
    format!("{WEBHOOK_RESET_BUTTON_PREFIX}:{guild_id}:{member_id}")
}

pub fn parse_webhook_reset_button_custom_id(custom_id: &str) -> Option<(i64, i64)> {
    let mut parts = custom_id.split(':');
    let prefix = parts.next()?;
    let guild_id = parts.next()?.parse::<i64>().ok()?;
    let member_id = parts.next()?.parse::<i64>().ok()?;

    if prefix == WEBHOOK_RESET_BUTTON_PREFIX && parts.next().is_none() {
        Some((guild_id, member_id))
    } else {
        None
    }
}
