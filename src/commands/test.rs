use crate::{Context, Error};

/// テスト用のコマンド
#[poise::command(slash_command)]
pub async fn test(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("こんにちは、なのだ!").await?;
    Ok(())
}