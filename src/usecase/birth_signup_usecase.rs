use crate::data::guild_repository::GuildRepository;
use crate::models::common::{Context, Error};
use chrono::NaiveDate;
use poise::{CreateReply, Modal};
use serenity::all::{CreateEmbed, Http};
use sqlx::PgPool;
use std::sync::Arc;

pub struct BirthSignupUsecase {
    guild_repo: GuildRepository,
}

impl BirthSignupUsecase {
    pub fn new(pool: Arc<PgPool>, http: Arc<Http>) -> anyhow::Result<Self> {
        let guild_repo = GuildRepository::new(pool, http.clone())?;
        Ok(BirthSignupUsecase {
            guild_repo,
        })
    }

    pub async fn invoke(&self, poise_ctx: Context<'_>) -> anyhow::Result<(), Error> {
        let guild_id = poise_ctx.guild_id().map(i64::from).ok_or_else(|| {
            let err_msg = "Could not retrieve the Guild ID.";
            tracing::error!(err_msg);
            anyhow::anyhow!(err_msg)
        })?;
        let member_id = i64::from(poise_ctx.author().id);

        let member_birth = self
            .guild_repo
            .get_member_birth(guild_id, member_id)
            .await?;

        if let None = member_birth {
            if let Context::Application(app_ctx) = poise_ctx {
                let data = BirthSignupModal::execute(app_ctx).await?;
                if let Some(data) = data {
                    let birth = NaiveDate::parse_from_str(&data.birth_input, "%Y-%m-%d")?;
                    self.guild_repo
                        .update_member_birth(guild_id, member_id, birth)
                        .await?;

                    poise_ctx
                        .send(
                            CreateReply::default()
                                .embed(
                                    CreateEmbed::new()
                                        .title("✅  誕生日の通知登録が完了したのだ。")
                                        .color(0x00ff00),
                                ) // オレンジ色
                                .content("登録したた日付の12時に誕生日が通知されるのだ。")
                                .ephemeral(true),
                        )
                        .await?;
                }
            }
        } else {
            poise_ctx
                .send(
                    CreateReply::default()
                        .embed(
                            CreateEmbed::new()
                                .title("⚠️ 誕生日はすでに登録済みなのだ")
                                .color(0xff9900),
                        ) // オレンジ色
                        .ephemeral(true),
                )
                .await?;
        }

        Ok(())
    }
}

#[derive(Debug, Modal)]
#[name = "誕生日の通知登録"] // 最初のタイトル
struct BirthSignupModal {
    #[name = "自身の誕生日を入力するのだ"] // フィールドのタイトル
    #[placeholder = "1999-12-10"]
    #[min_length = 10]
    #[max_length = 10]
    birth_input: String,
}
