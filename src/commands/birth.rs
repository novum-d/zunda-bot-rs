use crate::models::common::{Context, Error};
use poise::CreateReply;
use std::time::Instant;

/// 誕生日コマンド birth
#[poise::command(slash_command, subcommands("list", "signup", "reset", "remind"))]
pub async fn birth(_ctx: Context<'_>) -> anyhow::Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command, rename = "list")]
pub async fn list(ctx: Context<'_>) -> anyhow::Result<(), Error> {
    let start = Instant::now();
    tracing::info!(action = "list", "birth command received");

    ctx.defer_ephemeral().await?;
    let sync_start = Instant::now();
    if let Err(e) = ctx.data().guild_update_usecase.invoke().await {
        tracing::warn!("Guild sync failed before birth list: {}", e);
    }
    tracing::info!(
        elapsed_ms = sync_start.elapsed().as_millis(),
        "guild sync finished for birth list"
    );

    if let Err(e) = ctx.data().birth_list_usecase.invoke(ctx).await {
        report_command_error(ctx, "list", &e).await;
        return Ok(());
    }

    tracing::info!(
        action = "list",
        elapsed_ms = start.elapsed().as_millis(),
        "birth command finished"
    );
    Ok(())
}

#[poise::command(slash_command, rename = "signup")]
pub async fn signup(ctx: Context<'_>) -> anyhow::Result<(), Error> {
    let start = Instant::now();
    tracing::info!(action = "signup", "birth command received");

    if let Err(e) = ctx.data().birth_signup_usecase.invoke(ctx).await {
        report_command_error(ctx, "signup", &e).await;
        return Ok(());
    }

    tracing::info!(
        action = "signup",
        elapsed_ms = start.elapsed().as_millis(),
        "birth command finished"
    );
    Ok(())
}

#[poise::command(slash_command, rename = "reset")]
pub async fn reset(ctx: Context<'_>) -> anyhow::Result<(), Error> {
    let start = Instant::now();
    tracing::info!(action = "reset", "birth command received");

    ctx.defer_ephemeral().await?;
    if let Err(e) = ctx.data().birth_reset_usecase.invoke(ctx).await {
        report_command_error(ctx, "reset", &e).await;
        return Ok(());
    }

    tracing::info!(
        action = "reset",
        elapsed_ms = start.elapsed().as_millis(),
        "birth command finished"
    );
    Ok(())
}

#[poise::command(slash_command, subcommands("remind_resume"), rename = "remind")]
pub async fn remind(_ctx: Context<'_>) -> anyhow::Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command, rename = "resume")]
pub async fn remind_resume(ctx: Context<'_>) -> anyhow::Result<(), Error> {
    let start = Instant::now();
    tracing::info!(action = "remind resume", "birth command received");

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
    if let Err(e) = ctx
        .data()
        .reminder_service
        .resume_reminder(guild_id, member_id)
        .await
    {
        report_command_error(ctx, "remind resume", &e).await;
        return Ok(());
    }

    ctx.send(
        CreateReply::default()
            .content("リマインドを再開したのだ！")
            .ephemeral(true),
    )
    .await?;

    tracing::info!(
        action = "remind resume",
        elapsed_ms = start.elapsed().as_millis(),
        "birth command finished"
    );
    Ok(())
}

async fn report_command_error<E>(ctx: Context<'_>, action: &str, e: &E)
where
    E: std::fmt::Display + ?Sized,
{
    tracing::error!(action, "birth command failed: {}", e);
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
        tracing::warn!("failed to send fallback error response: {}", send_err);
    }
}
