use crate::{Context, Error};
use serenity::all::{ChannelType, GuildId};

#[poise::command(slash_command)]
pub async fn test(ctx: Context<'_>) -> Result<(), Error> {
    let http = ctx.http();

    let guild_id = GuildId::new(1393513606548553758);
    let channels = guild_id.channels(http).await?;
    if let Some((_, channel)) = channels.iter()
        .find(|(_, ch)| {
            ch.kind == ChannelType::Text &&
                // (ch.name == "一般" || ch.name == "general")
                ch.name == "bot"
        })
    {
        let member_id: i64 = 1393512983996661882;
    }
    Ok(())
}