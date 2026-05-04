use crate::models::common::Data;
use crate::reminder::service::parse_stop_button_custom_id;
use serenity::all::{
    ComponentInteraction, CreateInteractionResponse, CreateInteractionResponseMessage,
};

pub async fn handle_component_interaction(
    data: &Data,
    component: &ComponentInteraction,
) -> anyhow::Result<bool> {
    let Some((guild_id, member_id)) = parse_stop_button_custom_id(&component.data.custom_id) else {
        return Ok(false);
    };

    if i64::from(component.user.id) != member_id {
        component
            .create_response(
                &data.discord_http,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content("この通知は対象ユーザーだけが停止できるのだ。")
                        .ephemeral(true),
                ),
            )
            .await?;
        return Ok(true);
    }

    data.reminder_service
        .stop_reminder(guild_id, member_id)
        .await?;

    component
        .create_response(
            &data.discord_http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content("通知を停止したのだ！")
                    .ephemeral(true),
            ),
        )
        .await?;

    Ok(true)
}
