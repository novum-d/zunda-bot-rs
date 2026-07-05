use crate::data::guild_repository::GuildRepository;
use crate::models::common::{Context, Error};
use crate::res::colors::{EMBED_COLOR_ERROR, EMBED_COLOR_SUCCESS, EMBED_COLOR_WARNING};
use chrono::NaiveDate;
use poise::{CreateReply, Modal};
use serenity::all::{CreateEmbed, Http};
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

    pub async fn invoke(&self, poise_ctx: Context<'_>) -> anyhow::Result<(), Error> {
        let input_birth = if let Context::Application(app_ctx) = poise_ctx {
            // 先にモーダルを開いて interaction のタイムアウトを避ける
            let data = BirthSignupModal::execute(app_ctx).await?;
            match data {
                Some(data) => data.birth_input,
                None => return Ok(()),
            }
        } else {
            return Ok(());
        };

        // モーダル送信後はインタラクションが一度終了するため、
        // 以降の応答は defer してから行う必要がある。
        poise_ctx.defer_ephemeral().await?;

        // コマンドが実行されたギルドのギルドIDを取得
        let guild_id = self
            .guild_repo
            .fetch_guild_id_from_command(poise_ctx)
            .await?;
        let guild_id = i64::from(guild_id);
        let guild_name = poise_ctx
            .guild()
            .map(|guild| guild.name.clone())
            .unwrap_or_else(|| format!("guild-{guild_id}"));

        // コマンドを実行したメンバーのメンバーIDを取得;
        let member_id = i64::from(poise_ctx.author().id);

        let result = self
            .register_birth(guild_id, Some(guild_name.as_str()), member_id, &input_birth)
            .await?;
        poise_ctx.send(result.into_reply()).await?;

        Ok(())
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
    pub fn into_reply(self) -> CreateReply {
        match self {
            Self::InvalidFormat => CreateReply::default()
                .embed(
                    CreateEmbed::new()
                        .title("🚨  誕生日が正しいフォーマットで入力されていないのだ。")
                        .color(EMBED_COLOR_ERROR),
                )
                .ephemeral(true),
            Self::Registered => CreateReply::default()
                .embed(
                    CreateEmbed::new()
                        .title("✅  誕生日の通知登録が完了したのだ。")
                        .color(EMBED_COLOR_SUCCESS),
                )
                .content("登録した日付の正午（12:00）に誕生日が通知されるのだ。")
                .ephemeral(true),
            Self::AlreadyRegistered => CreateReply::default()
                .embed(
                    CreateEmbed::new()
                        .title("⚠️ 誕生日はすでに登録済みなのだ")
                        .color(EMBED_COLOR_WARNING),
                )
                .ephemeral(true),
        }
    }

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

#[derive(Debug, Modal)]
#[name = "誕生日の通知登録"] // 最初のタイトル
struct BirthSignupModal {
    #[name = "自身の誕生日を入力するのだ"] // フィールドのタイトル
    #[placeholder = "02/01"]
    #[min_length = 5]
    #[max_length = 5]
    birth_input: String,
}
