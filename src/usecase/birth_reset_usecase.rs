use crate::data::guild_repository::GuildRepository;
use crate::models::common::{Context, Error};
use poise::CreateReply;
use serenity::all::{
    CreateActionRow, CreateButton, CreateEmbed, CreateInteractionResponse,
    CreateInteractionResponseMessage, Http,
};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;

pub struct BirthResetUsecase {
    guild_repo: GuildRepository,
}

impl BirthResetUsecase {
    pub fn new(pool: Arc<PgPool>, http: Arc<Http>) -> anyhow::Result<Self> {
        let guild_repo = GuildRepository::new(pool, http.clone())?;
        Ok(BirthResetUsecase { guild_repo })
    }

    pub async fn invoke(&self, poise_ctx: Context<'_>) -> anyhow::Result<(), Error> {
        let guild_id = match poise_ctx.guild_id() {
            Some(id) => i64::from(id),
            None => {
                let err_msg = "Could not retrieve the Guild ID.";
                tracing::error!(err_msg);
                return Err(Error::from(anyhow::anyhow!(err_msg)));
            }
        };

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
                                .color(0xffd700), // 警告系の色
                        )
                        .ephemeral(true),
                )
                .await?;
        } else {
            let reset_button = CreateButton::new("reset")
                .label("解除")
                .style(serenity::all::ButtonStyle::Danger);

            let action_row = CreateActionRow::Buttons(vec![reset_button]);

            let msg = poise_ctx
                .send(
                    CreateReply::default()
                        .content("誕生日の通知登録を解除してもいいのだ👀？")
                        .components(vec![action_row])
                        .ephemeral(true),
                )
                .await?;

            if let Some(interaction) = msg
                .message()
                .await?
                .await_component_interaction(&poise_ctx.serenity_context().shard)
                .timeout(Duration::from_secs(60))
                .await
            {
                if interaction.data.custom_id == "reset" {
                    self.guild_repo
                        .reset_member_birth(guild_id, member_id)
                        .await?;
                    msg.delete(poise_ctx).await?;

                    let response = CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .embed(
                                CreateEmbed::new()
                                    .title("🗑️ 誕生日の通知登録を解除したのだ。")
                                    .description("登録した日付はリセットされたのだ。")
                                    .color(0x00ff00), // 正常系の色
                            )
                            .ephemeral(true),
                    );
                    interaction
                        .create_response(poise_ctx.http(), response)
                        .await
                        .unwrap_or_default();
                }
            }
        }
        Ok(())
    }
}
