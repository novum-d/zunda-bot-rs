use crate::data::guild_repository::GuildRepository;
use crate::res::colors::{EMBED_COLOR_ERROR, EMBED_COLOR_SUCCESS, EMBED_COLOR_WARNING};
use chrono::NaiveDate;
use serenity::all::Http;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct BirthSignupUsecase {
    guild_repo: GuildRepository,
    reminder_service: crate::reminder::service::ReminderService,
}

impl BirthSignupUsecase {
    pub fn new(pool: Arc<PgPool>, http: Arc<Http>) -> anyhow::Result<Self> {
        let guild_repo = GuildRepository::new(pool.clone(), http.clone())?;
        let reminder_service = crate::reminder::service::ReminderService::new(pool, http)?;
        Ok(BirthSignupUsecase {
            guild_repo,
            reminder_service,
        })
    }

    pub async fn register_birth(
        &self,
        guild_id: i64,
        guild_name: Option<&str>,
        member_id: i64,
        input_birth: &str,
    ) -> anyhow::Result<BirthSignupResult> {
        let Ok(birth) = NaiveDate::parse_from_str(&format!("2000/{input_birth}"), "%Y/%m/%d")
        else {
            return Ok(BirthSignupResult::InvalidFormat);
        };

        // 初回参加メンバーでも登録できるよう、対象レコードを事前に作成しておく
        self.guild_repo.add_guild(guild_id, guild_name).await?;
        self.guild_repo
            .add_member(guild_id, member_id, None)
            .await?;

        // ギルドIDとメンバーIDに一致するメンバー情報をguild_memberテーブルから取得
        let member_birth = self
            .guild_repo
            .get_member_birth(guild_id, member_id)
            .await?;

        if member_birth.is_none() {
            // メンバー情報に誕生日が存在しない
            // guild_memberテーブルのメンバーIDに一致するにメンバーの誕生日を更新
            self.guild_repo
                .update_member_birth(guild_id, member_id, birth)
                .await?;

            if let Err(e) = self
                .reminder_service
                .delete_saved_reminder_message(guild_id, member_id)
                .await
            {
                tracing::warn!(
                    guild_id,
                    member_id,
                    "failed to delete reminder message after birthday signup: {}",
                    e
                );
            }

            Ok(BirthSignupResult::Registered)
        } else {
            Ok(BirthSignupResult::AlreadyRegistered)
        }
    }
}

pub enum BirthSignupResult {
    InvalidFormat,
    Registered,
    AlreadyRegistered,
}

impl BirthSignupResult {
    pub fn title(&self) -> &'static str {
        match self {
            Self::InvalidFormat => "🚨  誕生日が正しいフォーマットで入力されていないのだ。",
            Self::Registered => "✅  誕生日の通知登録が完了したのだ。",
            Self::AlreadyRegistered => "⚠️ 誕生日はすでに登録済みなのだ",
        }
    }

    pub fn color(&self) -> u32 {
        match self {
            Self::InvalidFormat => EMBED_COLOR_ERROR,
            Self::Registered => EMBED_COLOR_SUCCESS,
            Self::AlreadyRegistered => EMBED_COLOR_WARNING,
        }
    }

    pub fn content(&self) -> Option<&'static str> {
        match self {
            Self::Registered => Some("登録した日付の正午（12:00）に誕生日が通知されるのだ。"),
            Self::InvalidFormat | Self::AlreadyRegistered => None,
        }
    }
}
