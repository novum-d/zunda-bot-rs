use crate::commands::report_command_error;
use crate::models::common::{Context, Error};
use crate::reminder::ui;
use poise::CreateReply;
use serenity::all::GuildChannel;
use std::time::Instant;

/// 管理者向けセットアップコマンド
#[poise::command(
    slash_command,
    subcommands(
        "reminder_channel",
        "add_notification_channel",
        "remove_notification_channel"
    )
)]
pub async fn setup(_ctx: Context<'_>) -> anyhow::Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command, rename = "reminder-channel")]
pub async fn reminder_channel(ctx: Context<'_>) -> anyhow::Result<(), Error> {
    let start = Instant::now();
    tracing::info!(action = "setup reminder-channel", "setup command received");

    let guild_id = match ctx.guild_id() {
        Some(guild_id) => i64::from(guild_id),
        None => {
            ctx.send(
                CreateReply::default()
                    .content("サーバー内で実行してほしいのだ。")
                    .ephemeral(true),
            )
            .await?;
            return Ok(());
        }
    };

    let member_id = i64::from(ctx.author().id);
    let is_admin = match ctx.data().reminder_service.is_admin_member(member_id).await {
        Ok(is_admin) => is_admin,
        Err(e) => {
            report_command_error(ctx, "setup reminder-channel", &e).await;
            return Ok(());
        }
    };

    if !can_run_admin_command(is_admin) {
        ctx.send(
            CreateReply::default()
                .content("このコマンドを実行する権限がないのだ。")
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    ctx.defer_ephemeral().await?;

    if let Err(e) = ctx.data().guild_update_usecase.invoke().await {
        tracing::warn!("Guild sync failed before reminder setup: {}", e);
    }

    let session_id = match ctx
        .data()
        .reminder_service
        .create_selection_session(member_id, guild_id)
    {
        Ok(session_id) => session_id,
        Err(e) => {
            report_command_error(ctx, "setup reminder-channel", &e).await;
            return Ok(());
        }
    };

    ctx.send(
        CreateReply::default()
            .content("リマインドを送るユーザーを選ぶのだ！")
            .components(ui::start_components(member_id, guild_id, &session_id))
            .ephemeral(true),
    )
    .await?;

    tracing::info!(
        action = "setup reminder-channel",
        elapsed_ms = start.elapsed().as_millis(),
        "setup command finished"
    );
    Ok(())
}

pub fn can_run_admin_command(is_admin: bool) -> bool {
    is_admin
}

#[poise::command(slash_command, rename = "add-notification-channel")]
pub async fn add_notification_channel(
    ctx: Context<'_>,
    #[description = "通知を送るチャンネル"] channel: GuildChannel,
) -> anyhow::Result<(), Error> {
    manage_notification_channel(ctx, channel, true, "add-notification-channel").await
}

#[poise::command(slash_command, rename = "remove-notification-channel")]
pub async fn remove_notification_channel(
    ctx: Context<'_>,
    #[description = "通知を送らなくするチャンネル"] channel: GuildChannel,
) -> anyhow::Result<(), Error> {
    manage_notification_channel(ctx, channel, false, "remove-notification-channel").await
}

async fn manage_notification_channel(
    ctx: Context<'_>,
    channel: GuildChannel,
    add: bool,
    action: &str,
) -> anyhow::Result<(), Error> {
    let guild_id = match ctx.guild_id() {
        Some(guild_id) => i64::from(guild_id),
        None => {
            ctx.send(
                CreateReply::default()
                    .content("サーバー内で実行してほしいのだ。")
                    .ephemeral(true),
            )
            .await?;
            return Ok(());
        }
    };

    let member_id = i64::from(ctx.author().id);
    let is_admin = match ctx.data().reminder_service.is_admin_member(member_id).await {
        Ok(is_admin) => is_admin,
        Err(e) => {
            report_command_error(ctx, action, &e).await;
            return Ok(());
        }
    };

    if !can_run_admin_command(is_admin) {
        ctx.send(
            CreateReply::default()
                .content("このコマンドを実行する権限がないのだ。")
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let channel_id = i64::from(channel.id);
    let result = if add {
        ctx.data()
            .reminder_service
            .add_notification_channel(guild_id, channel_id)
            .await
    } else {
        ctx.data()
            .reminder_service
            .remove_notification_channel(guild_id, channel_id)
            .await
    };

    if let Err(e) = result {
        report_command_error(ctx, action, &e).await;
        return Ok(());
    }

    let msg = if add {
        format!("<#{}> を通知チャンネルに追加したのだ！", channel.id)
    } else {
        format!("<#{}> を通知チャンネルから削除したのだ！", channel.id)
    };
    ctx.send(CreateReply::default().content(msg).ephemeral(true))
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_run_admin_command_when_admin_flag_is_enabled() {
        assert!(can_run_admin_command(true));
    }

    #[test]
    fn cannot_run_admin_command_when_admin_flag_is_disabled() {
        assert!(!can_run_admin_command(false));
    }
}
