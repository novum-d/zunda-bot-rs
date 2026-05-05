use crate::models::common::Data;
use crate::reminder::service::parse_stop_button_custom_id;
use crate::reminder::ui::{self, ReminderUiAction};
use serenity::all::{
    ComponentInteraction, ComponentInteractionDataKind, CreateInteractionResponse,
    CreateInteractionResponseMessage, EditInteractionResponse,
};

pub async fn handle_component_interaction(
    data: &Data,
    component: &ComponentInteraction,
) -> anyhow::Result<bool> {
    if let Some(action) = ui::parse_reminder_ui_custom_id(&component.data.custom_id) {
        return handle_reminder_ui_interaction(data, component, action).await;
    }

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

    component
        .create_response(
            &data.discord_http,
            CreateInteractionResponse::Defer(
                CreateInteractionResponseMessage::new().ephemeral(true),
            ),
        )
        .await?;

    data.reminder_service
        .stop_reminder(guild_id, member_id)
        .await?;

    let delete_message_result = data
        .reminder_service
        .delete_saved_reminder_message(guild_id, member_id)
        .await;
    let response_content = match delete_message_result {
        Ok(true) => "通知を停止して、リマインドメッセージを削除したのだ！",
        Ok(false) => match component
            .channel_id
            .delete_message(&data.discord_http, component.message.id)
            .await
        {
            Ok(()) => "通知を停止して、リマインドメッセージを削除したのだ！",
            Err(e) => {
                tracing::warn!(
                    guild_id,
                    member_id,
                    message_id = i64::from(component.message.id),
                    "failed to delete current reminder message: {}",
                    e
                );
                "通知は停止したけど、リマインドメッセージの削除に失敗したのだ。Botのメッセージ管理権限を確認してほしいのだ。"
            }
        },
        Err(e) => {
            tracing::warn!(
                guild_id,
                member_id,
                "failed to delete stopped reminder message: {}",
                e
            );
            "通知は停止したけど、リマインドメッセージの削除に失敗したのだ。Botのメッセージ管理権限を確認してほしいのだ。"
        }
    };

    component
        .edit_response(
            &data.discord_http,
            EditInteractionResponse::new().content(response_content),
        )
        .await?;

    Ok(true)
}

async fn handle_reminder_ui_interaction(
    data: &Data,
    component: &ComponentInteraction,
    action: ReminderUiAction,
) -> anyhow::Result<bool> {
    let (owner_id, guild_id, session_id, page) = match action {
        ReminderUiAction::Open {
            owner_id,
            guild_id,
            session_id,
        } => (owner_id, guild_id, session_id, 0),
        ReminderUiAction::Page {
            owner_id,
            guild_id,
            session_id,
            page,
        } => (owner_id, guild_id, session_id, page),
        ReminderUiAction::Select {
            owner_id,
            guild_id,
            session_id,
            page,
        } => {
            if i64::from(component.user.id) != owner_id {
                respond_owner_only(data, component).await?;
                return Ok(true);
            }

            component
                .create_response(&data.discord_http, CreateInteractionResponse::Acknowledge)
                .await?;

            let candidates = data
                .reminder_service
                .list_due_reminder_candidates_for_guild(guild_id)
                .await?;
            let page_member_ids = ui::page_member_ids(&candidates, page);
            let selected_page_member_ids = match &component.data.kind {
                ComponentInteractionDataKind::StringSelect { values } => values
                    .iter()
                    .filter_map(|value| value.parse::<i64>().ok())
                    .collect::<Vec<_>>(),
                _ => Vec::new(),
            };
            data.reminder_service.update_selection_for_page(
                &session_id,
                owner_id,
                guild_id,
                &page_member_ids,
                &selected_page_member_ids,
            )?;
            edit_selection_message(data, component, owner_id, guild_id, &session_id, page).await?;
            return Ok(true);
        }
        ReminderUiAction::Run {
            owner_id,
            guild_id,
            session_id,
        } => {
            if i64::from(component.user.id) != owner_id {
                respond_owner_only(data, component).await?;
                return Ok(true);
            }

            let selected_member_ids =
                data.reminder_service
                    .take_selected_member_ids(&session_id, owner_id, guild_id)?;
            let selected_count = selected_member_ids.len();
            if selected_count > 0 {
                let reminder_service = data.reminder_service.clone();
                tokio::spawn(async move {
                    match reminder_service
                        .send_selected_reminders(guild_id, selected_member_ids)
                        .await
                    {
                        Ok(sent_count) => {
                            tracing::info!(guild_id, sent_count, "selected reminders sent");
                        }
                        Err(e) => {
                            tracing::error!(guild_id, "selected reminder send failed: {}", e);
                        }
                    }
                });
            }

            component
                .create_response(
                    &data.discord_http,
                    CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new()
                            .content(format!(
                                "選択した{selected_count}人にリマインドを送信するのだ！"
                            ))
                            .components(Vec::new()),
                    ),
                )
                .await?;
            return Ok(true);
        }
    };

    if i64::from(component.user.id) != owner_id {
        respond_owner_only(data, component).await?;
        return Ok(true);
    }

    component
        .create_response(&data.discord_http, CreateInteractionResponse::Acknowledge)
        .await?;

    edit_selection_message(data, component, owner_id, guild_id, &session_id, page).await?;

    Ok(true)
}

async fn edit_selection_message(
    data: &Data,
    component: &ComponentInteraction,
    owner_id: i64,
    guild_id: i64,
    session_id: &str,
    page: usize,
) -> anyhow::Result<()> {
    let candidates = data
        .reminder_service
        .list_due_reminder_candidates_for_guild(guild_id)
        .await?;
    let selected_member_ids = data
        .reminder_service
        .selected_member_ids(session_id, owner_id, guild_id)?;
    component
        .edit_response(
            &data.discord_http,
            EditInteractionResponse::new()
                .content(ui::build_selection_content(&candidates, page))
                .components(ui::build_selection_components(
                    owner_id,
                    guild_id,
                    session_id,
                    &candidates,
                    &selected_member_ids,
                    page,
                )),
        )
        .await?;

    Ok(())
}

async fn respond_owner_only(data: &Data, component: &ComponentInteraction) -> anyhow::Result<()> {
    component
        .create_response(
            &data.discord_http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content("この操作は自分のみ使えるのだ")
                    .ephemeral(true),
            ),
        )
        .await?;
    Ok(())
}
