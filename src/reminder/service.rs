use crate::data::guild_repository::GuildRepository;
use crate::models::data::GuildMember;
use anyhow::Context as _;
use chrono::{DateTime, Duration, Utc};
use serenity::all::{
    ButtonStyle, ChannelId, CreateActionRow, CreateButton, CreateMessage, Http, Message,
};
use sqlx::PgPool;
use std::env;
use std::sync::Arc;
use tokio::time::sleep;

const MAX_REMIND_COUNT: i32 = 5;
const ACTIVE_WINDOW_DAYS: i64 = 7;
const MIN_REMIND_INTERVAL_HOURS: i64 = 24;
const FIRST_REMIND_DELAY_DAYS: i64 = 1;
pub const STOP_BUTTON_PREFIX: &str = "birth_reminder_stop";

pub type User = GuildMember;

pub struct ReminderService {
    guild_repo: GuildRepository,
    http: Arc<Http>,
    bot_channel_id: ChannelId,
}

impl ReminderService {
    pub fn new(pool: Arc<PgPool>, http: Arc<Http>) -> anyhow::Result<Self> {
        let bot_channel_id = env::var("BOT_CHANNEL_ID")
            .context("'BOT_CHANNEL_ID' was not found")?
            .parse::<u64>()
            .context("'BOT_CHANNEL_ID' must be a Discord channel id")?;
        let guild_repo = GuildRepository::new(pool, http.clone())?;

        Ok(Self {
            guild_repo,
            http,
            bot_channel_id: ChannelId::new(bot_channel_id),
        })
    }

    pub async fn record_message_activity(&self, message: &Message) -> anyhow::Result<()> {
        if message.author.bot {
            return Ok(());
        }

        let Some(guild_id) = message.guild_id else {
            return Ok(());
        };

        let guild_id = i64::from(guild_id);
        let member_id = i64::from(message.author.id);
        let now = Utc::now();
        let first_remind_at = now + Duration::days(FIRST_REMIND_DELAY_DAYS);

        self.guild_repo
            .add_guild(guild_id, Some(&format!("guild-{guild_id}")))
            .await?;
        self.guild_repo.add_member(guild_id, member_id, None).await?;
        self.guild_repo
            .update_last_active(guild_id, member_id, now, first_remind_at)
            .await?;

        if let Some(user) = self.guild_repo.get_member(guild_id, member_id).await? {
            self.send_due_reminder(&user, now).await?;
        }

        Ok(())
    }

    pub async fn send_due_reminder(&self, user: &User, now: DateTime<Utc>) -> anyhow::Result<bool> {
        if !should_send_reminder(user, now) {
            return Ok(false);
        }

        self.send_reminder(user).await?;
        self.mark_reminder_sent(user, now).await?;
        Ok(true)
    }

    pub async fn send_reminder(&self, user: &User) -> anyhow::Result<()> {
        let custom_id = stop_button_custom_id(user.guild_id, user.member_id);
        let stop_button = CreateButton::new(custom_id)
            .label("リマインド停止")
            .style(ButtonStyle::Danger);

        self.bot_channel_id
            .send_message(
                &self.http,
                CreateMessage::new()
                    .content(format!(
                        "<@{}>\nまだ誕生日が登録されていないのだ！\nよければ `/birth signup` から登録してほしいのだ！",
                        user.member_id
                    ))
                    .components(vec![CreateActionRow::Buttons(vec![stop_button])]),
            )
            .await?;

        Ok(())
    }

    pub async fn mark_reminder_sent(&self, user: &User, now: DateTime<Utc>) -> anyhow::Result<()> {
        let next_count = user.remind_count + 1;
        let next_remind_at = calculate_next_remind_at(now, next_count);
        self.guild_repo
            .update_reminder_sent(user.guild_id, user.member_id, now, next_remind_at)
            .await?;
        Ok(())
    }

    pub async fn stop_reminder(&self, guild_id: i64, member_id: i64) -> anyhow::Result<()> {
        self.guild_repo
            .update_reminder_opt_out(guild_id, member_id, true, None)
            .await?;
        Ok(())
    }

    pub async fn resume_reminder(&self, guild_id: i64, member_id: i64) -> anyhow::Result<()> {
        let next_remind_at = Utc::now() + Duration::days(FIRST_REMIND_DELAY_DAYS);
        self.guild_repo
            .add_guild(guild_id, Some(&format!("guild-{guild_id}")))
            .await?;
        self.guild_repo.add_member(guild_id, member_id, None).await?;
        self.guild_repo
            .update_reminder_opt_out(guild_id, member_id, false, Some(next_remind_at))
            .await?;
        Ok(())
    }

    pub async fn scan_and_send(&self) -> anyhow::Result<usize> {
        let now = Utc::now();
        let active_since = now - Duration::days(ACTIVE_WINDOW_DAYS);
        let users = self
            .guild_repo
            .get_active_reminder_candidates(active_since)
            .await?;

        let mut sent_count = 0;
        for user in users {
            if self.send_due_reminder(&user, now).await? {
                sent_count += 1;
                sleep(stagger_delay(&user)).await;
            }
        }

        Ok(sent_count)
    }
}

pub fn should_send_reminder(user: &User, now: DateTime<Utc>) -> bool {
    if user.birth.is_some() || user.is_remind_opt_out || user.remind_count >= MAX_REMIND_COUNT {
        return false;
    }

    let Some(last_active_at) = user.last_active_at else {
        return false;
    };
    if now.signed_duration_since(last_active_at) > Duration::days(ACTIVE_WINDOW_DAYS) {
        return false;
    }

    let Some(next_remind_at) = user.next_remind_at else {
        return false;
    };
    if now < next_remind_at {
        return false;
    }

    if user
        .last_reminded_at
        .is_some_and(|last_reminded_at| {
            now.signed_duration_since(last_reminded_at)
                < Duration::hours(MIN_REMIND_INTERVAL_HOURS)
        })
    {
        return false;
    }

    true
}

pub fn calculate_next_remind_at(now: DateTime<Utc>, remind_count: i32) -> DateTime<Utc> {
    now + Duration::days(backoff_days(remind_count))
}

pub fn stop_button_custom_id(guild_id: i64, member_id: i64) -> String {
    format!("{STOP_BUTTON_PREFIX}:{guild_id}:{member_id}")
}

pub fn parse_stop_button_custom_id(custom_id: &str) -> Option<(i64, i64)> {
    let mut parts = custom_id.split(':');
    let prefix = parts.next()?;
    let guild_id = parts.next()?.parse::<i64>().ok()?;
    let member_id = parts.next()?.parse::<i64>().ok()?;

    if prefix == STOP_BUTTON_PREFIX && parts.next().is_none() {
        Some((guild_id, member_id))
    } else {
        None
    }
}

fn backoff_days(remind_count: i32) -> i64 {
    match remind_count {
        1 => 1,
        2 => 3,
        3 => 7,
        4 => 14,
        _ => 30,
    }
}

fn stagger_delay(user: &User) -> std::time::Duration {
    let seconds = 1 + (user.guild_id + user.member_id).rem_euclid(3) as u64;
    std::time::Duration::from_secs(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn now() -> DateTime<Utc> {
        match DateTime::parse_from_rfc3339("2026-05-04T12:00:00Z") {
            Ok(value) => value.with_timezone(&Utc),
            Err(e) => panic!("invalid test datetime: {e}"),
        }
    }

    fn user() -> User {
        User {
            guild_id: 1,
            member_id: 2,
            birth: None,
            last_notified: None,
            last_active_at: Some(now() - Duration::days(1)),
            last_reminded_at: Some(now() - Duration::hours(25)),
            next_remind_at: Some(now()),
            remind_count: 0,
            is_remind_opt_out: false,
        }
    }

    #[test]
    fn should_send_reminder_when_all_conditions_match() {
        assert!(should_send_reminder(&user(), now()));
    }

    #[test]
    fn should_not_send_when_birth_is_registered() {
        let mut user = user();
        user.birth = NaiveDate::from_ymd_opt(1970, 5, 4);

        assert!(!should_send_reminder(&user, now()));
    }

    #[test]
    fn should_not_send_when_recently_reminded() {
        let mut user = user();
        user.last_reminded_at = Some(now() - Duration::hours(23));

        assert!(!should_send_reminder(&user, now()));
    }

    #[test]
    fn should_not_send_before_next_remind_at() {
        let mut user = user();
        user.next_remind_at = Some(now() + Duration::minutes(1));

        assert!(!should_send_reminder(&user, now()));
    }

    #[test]
    fn should_not_send_after_five_reminders() {
        let mut user = user();
        user.remind_count = 5;

        assert!(!should_send_reminder(&user, now()));
    }

    #[test]
    fn calculate_next_remind_at_uses_required_backoff_days() {
        assert_eq!(calculate_next_remind_at(now(), 1), now() + Duration::days(1));
        assert_eq!(calculate_next_remind_at(now(), 2), now() + Duration::days(3));
        assert_eq!(calculate_next_remind_at(now(), 3), now() + Duration::days(7));
        assert_eq!(calculate_next_remind_at(now(), 4), now() + Duration::days(14));
        assert_eq!(calculate_next_remind_at(now(), 5), now() + Duration::days(30));
    }

    #[test]
    fn stop_button_custom_id_round_trips() {
        let custom_id = stop_button_custom_id(123, 456);

        assert_eq!(parse_stop_button_custom_id(&custom_id), Some((123, 456)));
    }
}
