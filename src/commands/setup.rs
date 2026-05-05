use crate::models::common::{Context, Error};
use crate::reminder::ui;
use poise::CreateReply;
use std::time::Instant;

/// 管理者向けセットアップコマンド
#[poise::command(slash_command, subcommands("reminder_channel"))]
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

    if let Err(e) = ctx
        .data()
        .reminder_service
        .ensure_reminder_channel(guild_id)
        .await
    {
        report_command_error(ctx, "setup reminder-channel", &e).await;
        return Ok(());
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

async fn report_command_error<E>(ctx: Context<'_>, action: &str, e: &E)
where
    E: std::fmt::Display + ?Sized,
{
    tracing::error!(action, "setup command failed: {}", e);
    if let Err(send_err) = ctx
        .send(
            CreateReply::default()
                .content(
                    "コマンドの実行中にエラーが発生したのだ。時間をおいて再実行してほしいのだ。",
                )
                .ephemeral(true),
        )
        .await
    {
        let err_str = send_err.to_string();
        if err_str.contains("Interaction has already been acknowledged") {
            tracing::debug!("interaction already acknowledged, skipping error report reply");
        } else {
            tracing::warn!("failed to send fallback error response: {}", send_err);
        }
    }
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
