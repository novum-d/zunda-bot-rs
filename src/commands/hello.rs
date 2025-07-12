use crate::{Context, Error};

/// あいさつのメッセージを返すコマンド
#[poise::command(slash_command)]
pub async fn hello(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("こんにちは、なのだ!").await?;
    Ok(())
}