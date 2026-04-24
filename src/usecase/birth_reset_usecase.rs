use crate::data::guild_repository::GuildRepository;
use crate::models::common::{Context, Error};
use crate::res::colors::{EMBED_COLOR_SUCCESS, EMBED_COLOR_WARNING};
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
        // コマンドが実行されたギルドのギルドIDを取得
        let guild_id = self
            .guild_repo
            .fetch_guild_id_from_command(poise_ctx)
            .await?;
        let guild_id = i64::from(guild_id);

        // コマンドを実行したメンバーのメンバーIDを取得
        let member_id = i64::from(poise_ctx.author().id);

        // ギルドIDとメンバーIDに一致するメンバーの誕生日をguild_memberテーブルから取得
        let member_birth = self
            .guild_repo
            .get_member_birth(guild_id, member_id)
            .await?;

        if member_birth.is_none() {
            // 「誕生日が登録されていないこと」をメッセージで通知
            poise_ctx
                .send(
                    CreateReply::default()
                        .embed(
                            CreateEmbed::new()
                                .title("⚠️ 誕生日が登録されていないのだ")
                                .color(EMBED_COLOR_WARNING), // 警告系の色
                        )
                        .ephemeral(true),
                )
                .await?;
        } else {
            // 誕生日解除の確認メッセージと「解除」ボタンを表示
            let reset_button = CreateButton::new("reset")
                .label("解除")
                .style(serenity::all::ButtonStyle::Danger);
            let action_row = CreateActionRow::Buttons(vec![reset_button]);
            let reply_handle = poise_ctx
                .send(
                    CreateReply::default()
                        .content("誕生日の通知登録を解除するのだ⚠️")
                        .components(vec![action_row])
                        .ephemeral(true),
                )
                .await?;
            let msg = reply_handle.message().await?;

            let msg_interaction = msg
                .await_component_interaction(&poise_ctx.serenity_context().shard)
                .timeout(Duration::from_secs(60))
                .await;
            if let Some(interaction) = msg_interaction {
                if interaction.data.custom_id == "reset" {
                    // ユーザーが「解除」ボタンを押下
                    // guild_memberテーブルの誕生日と最終通知日をNULLに更新
                    self.guild_repo
                        .reset_member_birth(guild_id, member_id)
                        .await?;

                    // 誕生日解除の確認メッセージと「解除」ボタンを削除
                    reply_handle
                        .delete(poise_ctx)
                        .await
                        .unwrap_or_else(|e| tracing::warn!("Failed to delete message: {}", e));

                    // 「誕生日通知が解除されたこと」をメッセージで通知
                    let response = CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .embed(
                                CreateEmbed::new()
                                    .title("🗑️ 誕生日の通知登録を解除したのだ。")
                                    .description("登録した日付はリセットされたのだ。")
                                    .color(EMBED_COLOR_SUCCESS), // 正常系の色
                            )
                            .ephemeral(true),
                    );
                    interaction
                        .create_response(poise_ctx.http(), response)
                        .await
                        .unwrap_or_else(|e| {
                            tracing::warn!("Failed to respond to interaction: {}", e)
                        });
                }
            }
        }
        Ok(())
    }
}
