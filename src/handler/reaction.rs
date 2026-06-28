use crate::models::common::Data;
use serenity::all::{Context, Reaction};

pub async fn handle_reaction_add(
    _ctx: &Context,
    data: &Data,
    reaction: &Reaction,
) -> anyhow::Result<()> {
    data.reminder_service
        .record_reaction_activity(reaction)
        .await
}
