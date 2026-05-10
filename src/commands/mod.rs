pub mod birth;
pub mod hello;
pub mod setup;

use crate::models::common::Context;
use poise::CreateReply;

pub async fn report_command_error<E>(ctx: Context<'_>, action: &str, e: &E)
where
    E: std::fmt::Display + ?Sized,
{
    tracing::error!(action, "command failed: {}", e);
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
