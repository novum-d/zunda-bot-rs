use poise::async_trait;
use serenity::all::{
    CreateEmbed, CreateInteractionResponse, CreateInteractionResponseMessage, EventHandler,
    Interaction,
};

pub struct CommandHandler;

#[async_trait]
impl EventHandler for CommandHandler {
    async fn interaction_create(&self, ctx: serenity::all::Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(_command) => {}
            Interaction::Component(interaction) => {
                if interaction.data.custom_id == "reset" {
                    let response = CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .embed(
                                CreateEmbed::new()
                                    .title("🗑️ 誕生日の通知登録を解除したのだ。")
                                    .description("登録した日付はリセットされたのだ。")
                                    .color(0x00ff00),
                            ) // オレンジ色
                            .ephemeral(true),
                    );
                    interaction
                        .create_response(&ctx.http, response)
                        .await
                        .unwrap_or_default();
                }
            }
            _ => {}
        }
    }
}
