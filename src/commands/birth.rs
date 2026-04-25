use crate::models::common::{Context, Error};
use poise::ChoiceParameter;
use poise::CreateReply;
use std::time::Instant;

#[derive(Debug, ChoiceParameter)]
pub enum BirthAction {
    #[name = "List: サーバー内メンバーの誕生日リスト表示"]
    List,
    #[name = "Signup: 自身の誕生日の通知登録"]
    Signup,
    #[name = "Reset: 自身の誕生日の通知解除"]
    Reset,
}

/// 誕生日コマンド birth
#[poise::command(slash_command)]
pub async fn birth(
    ctx: Context<'_>,
    #[description = "操作"] action: BirthAction,
) -> anyhow::Result<(), Error> {
    async fn report_command_error(ctx: Context<'_>, action: &BirthAction, e: &Error) {
        tracing::error!(action = ?action, "birth command failed: {}", e);
        if let Err(send_err) = ctx
            .send(
                CreateReply::default()
                    .content("コマンドの実行中にエラーが発生したのだ。時間をおいて再実行してほしいのだ。")
                    .ephemeral(true),
            )
            .await
        {
            tracing::warn!("failed to send fallback error response: {}", send_err);
        }
    }

    let start = Instant::now();
    tracing::info!(action = ?action, "birth command received");

    match action {
        BirthAction::List => {
            // List はギルド同期が重くなることがあるため、先に interaction を確定させる
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
                report_command_error(ctx, &action, &e).await;
                return Ok(());
            }
        }
        BirthAction::Signup => {
            if let Err(e) = ctx.data().birth_signup_usecase.invoke(ctx).await {
                report_command_error(ctx, &action, &e).await;
                return Ok(());
            }
        }
        BirthAction::Reset => {
            // Reset は後続でボタン操作が発生するため、先に defer してタイムアウトを避ける
            ctx.defer_ephemeral().await?;
            if let Err(e) = ctx.data().birth_reset_usecase.invoke(ctx).await {
                report_command_error(ctx, &action, &e).await;
                return Ok(());
            }
        }
    }
    tracing::info!(
        action = ?action,
        elapsed_ms = start.elapsed().as_millis(),
        "birth command finished"
    );
    Ok(())
}
