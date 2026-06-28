use crate::models::common::Data;
use serenity::all::{Context, Message};

pub async fn handle_message(_ctx: &Context, data: &Data, message: &Message) -> anyhow::Result<()> {
    data.reminder_service.record_message_activity(message).await
}
