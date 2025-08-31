use crate::data::guild_repository::GuildRepository;
use crate::models::common::{Context, Error};
use poise::CreateReply;
use serenity::all::{CreateActionRow, CreateButton, CreateEmbed, Http};
use sqlx::PgPool;
use std::sync::Arc;

pub struct BirthResetUsecase {
    guild_repo: GuildRepository,
}

impl BirthResetUsecase {
    pub fn new(pool: Arc<PgPool>, http: Arc<Http>) -> anyhow::Result<Self> {
        let guild_repo = GuildRepository::new(pool, http.clone())?;
        Ok(BirthResetUsecase {
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
            poise_ctx
                .send(
                    CreateReply::default()
                        .embed(
                            CreateEmbed::new()
                                .title("⚠️ 誕生日が登録されていないのだ")
                                .color(0xff9900),
                        ) // オレンジ色
                        .ephemeral(true),
                )
                .await?;
        } else {
            let reset_button = CreateButton::new("reset")
                .label("解除")
                .style(serenity::all::ButtonStyle::Danger);

            let action_row = CreateActionRow::Buttons(vec![reset_button]);

            poise_ctx
                .send(
                    CreateReply::default()
                        .content("誕生日の通知登録を解除してもいいのだ？")
                        .embed(CreateEmbed::new().title("確認"))
                        .components(vec![action_row]),
                )
                .await?;
        }
        Ok(())
    }
}
