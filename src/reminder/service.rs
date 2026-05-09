use crate::data::guild_repository::GuildRepository;
use crate::models::data::GuildMember;
use anyhow::Context as _;
use chrono::{DateTime, Duration, Utc};
use serenity::all::{
    ButtonStyle, ChannelId, ChannelType, CreateActionRow, CreateButton, CreateChannel,
    CreateMessage, GuildId, Http, Message, MessageId, Reaction, UserId,
};
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::sleep;

const MAX_REMIND_COUNT: i32 = 5;
const ACTIVE_WINDOW_DAYS: i64 = 7;
const MIN_REMIND_INTERVAL_HOURS: i64 = 24;
const FIRST_REMIND_DELAY_DAYS: i64 = 1;
const BOT_CHANNEL_NAME: &str = "ずんだぼっと";
pub const STOP_BUTTON_PREFIX: &str = "birth_reminder_stop";

pub type User = GuildMember;

#[derive(Clone)]
pub struct ReminderService {
    guild_repo: GuildRepository,
    http: Arc<Http>,
    selection_sessions: Arc<Mutex<HashMap<String, ReminderSelectionSession>>>,
}

#[derive(Debug, Clone)]
struct ReminderSelectionSession {
    owner_id: i64,
    guild_id: i64,
    selected_member_ids: Vec<i64>,
}

#[derive(Debug, Clone)]
pub struct ReminderUiCandidate {
    pub member_id: i64,
    pub display_name: String,
}

impl ReminderService {
    pub fn new(pool: Arc<PgPool>, http: Arc<Http>) -> anyhow::Result<Self> {
        let guild_repo = GuildRepository::new(pool, http.clone())?;

        Ok(Self {
            guild_repo,
            http,
            selection_sessions: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn record_message_activity(&self, message: &Message) -> anyhow::Result<()> {
        if !is_user_activity_recordable(message.author.bot, message.guild_id.is_some()) {
            return Ok(());
        }

        let Some(guild_id) = message.guild_id else {
            return Ok(());
        };
        let guild_id = i64::from(guild_id);
        let member_id = i64::from(message.author.id);
        self.record_member_activity(guild_id, member_id).await
    }

    pub async fn record_reaction_activity(&self, reaction: &Reaction) -> anyhow::Result<()> {
        let user_is_bot = match &reaction.member {
            Some(member) => member.user.bot,
            None => reaction.user(&self.http).await?.bot,
        };

        if !is_user_activity_recordable(user_is_bot, reaction.guild_id.is_some()) {
            return Ok(());
        }

        let Some(guild_id) = reaction.guild_id else {
            return Ok(());
        };
        let Some(user_id) = reaction.user_id else {
            return Ok(());
        };

        self.record_member_activity(i64::from(guild_id), i64::from(user_id))
            .await
    }

    async fn record_member_activity(&self, guild_id: i64, member_id: i64) -> anyhow::Result<()> {
        let now = Utc::now();
        let first_remind_at = now + Duration::days(FIRST_REMIND_DELAY_DAYS);

        self.guild_repo
            .add_guild(guild_id, Some(&format!("guild-{guild_id}")))
            .await?;
        self.guild_repo
            .add_member(guild_id, member_id, None)
            .await?;
        self.guild_repo
            .update_last_active(guild_id, member_id, now, first_remind_at)
            .await?;

        let active_since = now - Duration::days(ACTIVE_WINDOW_DAYS);
        if let Some(user) = self
            .guild_repo
            .get_active_reminder_candidate(member_id, active_since)
            .await?
        {
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
        let channel_id = self.ensure_bot_channel(user.guild_id).await?;

        let message = channel_id
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
        self.guild_repo
            .update_reminder_message(
                user.guild_id,
                user.member_id,
                i64::from(message.channel_id),
                i64::from(message.id),
            )
            .await?;

        Ok(())
    }

    async fn ensure_bot_channel(&self, guild_id: i64) -> anyhow::Result<ChannelId> {
        let guild_id = GuildId::new(u64::try_from(guild_id).context("guild id must be positive")?);
        let channels = guild_id.channels(&self.http).await?;

        if let Some(channel) = channels
            .values()
            .find(|channel| channel.kind == ChannelType::Text && channel.name == BOT_CHANNEL_NAME)
        {
            return Ok(channel.id);
        }

        let channel = guild_id
            .create_channel(
                &self.http,
                CreateChannel::new(BOT_CHANNEL_NAME).kind(ChannelType::Text),
            )
            .await?;
        Ok(channel.id)
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

    pub async fn delete_saved_reminder_message(
        &self,
        guild_id: i64,
        member_id: i64,
    ) -> anyhow::Result<bool> {
        let Some((channel_id, message_id)) = self
            .guild_repo
            .get_reminder_message(guild_id, member_id)
            .await?
        else {
            return Ok(false);
        };

        let channel_id =
            ChannelId::new(u64::try_from(channel_id).context("channel id must be positive")?);
        let message_id =
            MessageId::new(u64::try_from(message_id).context("message id must be positive")?);
        channel_id.delete_message(&self.http, message_id).await?;
        self.guild_repo
            .clear_reminder_message(guild_id, member_id)
            .await?;
        Ok(true)
    }

    pub async fn resume_reminder(&self, guild_id: i64, member_id: i64) -> anyhow::Result<()> {
        let next_remind_at = Utc::now() + Duration::days(FIRST_REMIND_DELAY_DAYS);
        self.guild_repo
            .add_guild(guild_id, Some(&format!("guild-{guild_id}")))
            .await?;
        self.guild_repo
            .add_member(guild_id, member_id, None)
            .await?;
        self.guild_repo
            .update_reminder_opt_out(guild_id, member_id, false, Some(next_remind_at))
            .await?;
        Ok(())
    }

    pub async fn ensure_reminder_channel(&self, guild_id: i64) -> anyhow::Result<()> {
        self.ensure_bot_channel(guild_id).await?;
        Ok(())
    }

    pub async fn is_admin_member(&self, member_id: i64) -> anyhow::Result<bool> {
        self.guild_repo.is_admin_member(member_id).await
    }

    pub fn create_selection_session(&self, owner_id: i64, guild_id: i64) -> anyhow::Result<String> {
        let session_id = new_session_id(owner_id, guild_id);
        let session = ReminderSelectionSession {
            owner_id,
            guild_id,
            selected_member_ids: Vec::new(),
        };
        self.selection_sessions
            .lock()
            .map_err(|_| anyhow::anyhow!("reminder selection session lock poisoned"))?
            .insert(session_id.clone(), session);
        Ok(session_id)
    }

    pub fn selected_member_ids(
        &self,
        session_id: &str,
        owner_id: i64,
        guild_id: i64,
    ) -> anyhow::Result<Vec<i64>> {
        let sessions = self
            .selection_sessions
            .lock()
            .map_err(|_| anyhow::anyhow!("reminder selection session lock poisoned"))?;
        let Some(session) = sessions.get(session_id) else {
            return Ok(Vec::new());
        };
        if session.owner_id != owner_id || session.guild_id != guild_id {
            return Ok(Vec::new());
        }
        Ok(session.selected_member_ids.clone())
    }

    pub fn update_selection_for_page(
        &self,
        session_id: &str,
        owner_id: i64,
        guild_id: i64,
        page_member_ids: &[i64],
        selected_page_member_ids: &[i64],
    ) -> anyhow::Result<Vec<i64>> {
        let page_member_ids = page_member_ids.iter().copied().collect::<HashSet<_>>();
        let selected_page_member_ids = selected_page_member_ids
            .iter()
            .copied()
            .filter(|member_id| page_member_ids.contains(member_id))
            .collect::<HashSet<_>>();

        let mut sessions = self
            .selection_sessions
            .lock()
            .map_err(|_| anyhow::anyhow!("reminder selection session lock poisoned"))?;
        let Some(session) = sessions.get_mut(session_id) else {
            return Ok(Vec::new());
        };
        if session.owner_id != owner_id || session.guild_id != guild_id {
            return Ok(Vec::new());
        }

        session
            .selected_member_ids
            .retain(|member_id| !page_member_ids.contains(member_id));
        session.selected_member_ids.extend(selected_page_member_ids);
        session.selected_member_ids.sort_unstable();
        session.selected_member_ids.dedup();
        Ok(session.selected_member_ids.clone())
    }

    pub fn take_selected_member_ids(
        &self,
        session_id: &str,
        owner_id: i64,
        guild_id: i64,
    ) -> anyhow::Result<Vec<i64>> {
        let mut sessions = self
            .selection_sessions
            .lock()
            .map_err(|_| anyhow::anyhow!("reminder selection session lock poisoned"))?;
        let Some(session) = sessions.remove(session_id) else {
            return Ok(Vec::new());
        };
        if session.owner_id != owner_id || session.guild_id != guild_id {
            return Ok(Vec::new());
        }
        Ok(session.selected_member_ids)
    }

    pub async fn list_due_reminder_candidates_for_guild(
        &self,
        guild_id: i64,
    ) -> anyhow::Result<Vec<ReminderUiCandidate>> {
        let now = Utc::now();
        let users = self.guild_repo.get_members_by_guild_id(guild_id).await?;
        let guild = GuildId::new(u64::try_from(guild_id).context("guild id must be positive")?);
        let mut candidates = Vec::new();

        for user in users
            .into_iter()
            .filter(|user| should_show_manual_reminder_candidate(user, now))
        {
            let Some(display_name) =
                fetch_non_bot_display_name(&self.http, guild, user.member_id).await
            else {
                continue;
            };
            candidates.push(ReminderUiCandidate {
                member_id: user.member_id,
                display_name,
            });
        }

        Ok(candidates)
    }

    pub async fn set_selected_reminders(
        &self,
        guild_id: i64,
        selected_member_ids: Vec<i64>,
    ) -> anyhow::Result<usize> {
        let selected_member_ids = selected_member_ids.into_iter().collect::<HashSet<_>>();
        for member_id in &selected_member_ids {
            self.guild_repo
                .update_manual_reminder_target(guild_id, *member_id)
                .await?;
        }

        Ok(selected_member_ids.len())
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
            match self.send_due_reminder(&user, now).await {
                Ok(true) => {
                    sent_count += 1;
                    sleep(stagger_delay(&user)).await;
                }
                Ok(false) => {}
                Err(e) => {
                    tracing::error!(
                        guild_id = user.guild_id,
                        member_id = user.member_id,
                        "Failed to send reminder: {}",
                        e
                    );
                }
            }
        }

        Ok(sent_count)
    }
}

pub fn should_send_reminder(user: &User, now: DateTime<Utc>) -> bool {
    if user.birth.is_some() || user.is_remind_opt_out || user.remind_count >= MAX_REMIND_COUNT {
        return false;
    }

    if user.reminder_guild_id != Some(user.guild_id) {
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

    if user.last_reminded_at.is_some_and(|last_reminded_at| {
        now.signed_duration_since(last_reminded_at) < Duration::hours(MIN_REMIND_INTERVAL_HOURS)
    }) {
        return false;
    }

    true
}

pub fn should_show_manual_reminder_candidate(user: &User, now: DateTime<Utc>) -> bool {
    if user.birth.is_some() || user.is_remind_opt_out || user.remind_count >= MAX_REMIND_COUNT {
        return false;
    }

    if user.last_reminded_at.is_some_and(|last_reminded_at| {
        now.signed_duration_since(last_reminded_at) < Duration::hours(MIN_REMIND_INTERVAL_HOURS)
    }) {
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

fn is_user_activity_recordable(user_is_bot: bool, has_guild: bool) -> bool {
    !user_is_bot && has_guild
}

async fn fetch_non_bot_display_name(
    http: &Http,
    guild_id: GuildId,
    member_id: i64,
) -> Option<String> {
    let Ok(user_id) = u64::try_from(member_id) else {
        return Some(format!("user-{member_id}"));
    };

    match guild_id.member(http, UserId::new(user_id)).await {
        Ok(member) if member.user.bot => None,
        Ok(member) if member.display_name().eq_ignore_ascii_case("zunda-bot-rs") => None,
        Ok(member) => Some(member.display_name().to_string()),
        Err(e) => {
            tracing::warn!(
                guild_id = i64::from(guild_id),
                member_id,
                "Failed to fetch reminder candidate display name: {}",
                e
            );
            Some(format!("user-{member_id}"))
        }
    }
}

fn new_session_id(owner_id: i64, guild_id: i64) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{owner_id:x}{guild_id:x}{:x}", nanos % 0xffff_ffff)
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
            reminder_guild_id: Some(1),
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
    fn should_not_send_when_reminder_guild_is_not_configured() {
        let mut user = user();
        user.reminder_guild_id = None;

        assert!(!should_send_reminder(&user, now()));
    }

    #[test]
    fn should_not_send_when_reminder_guild_is_different() {
        let mut user = user();
        user.reminder_guild_id = Some(99);

        assert!(!should_send_reminder(&user, now()));
    }

    #[test]
    fn manual_candidate_can_be_inactive() {
        let mut user = user();
        user.last_active_at = None;
        user.next_remind_at = None;

        assert!(should_show_manual_reminder_candidate(&user, now()));
    }

    #[test]
    fn user_activity_is_recordable_for_non_bot_guild_events() {
        assert!(is_user_activity_recordable(false, true));
    }

    #[test]
    fn user_activity_is_not_recordable_for_bots_or_non_guild_events() {
        assert!(!is_user_activity_recordable(true, true));
        assert!(!is_user_activity_recordable(false, false));
    }

    #[test]
    fn calculate_next_remind_at_uses_required_backoff_days() {
        assert_eq!(
            calculate_next_remind_at(now(), 1),
            now() + Duration::days(1)
        );
        assert_eq!(
            calculate_next_remind_at(now(), 2),
            now() + Duration::days(3)
        );
        assert_eq!(
            calculate_next_remind_at(now(), 3),
            now() + Duration::days(7)
        );
        assert_eq!(
            calculate_next_remind_at(now(), 4),
            now() + Duration::days(14)
        );
        assert_eq!(
            calculate_next_remind_at(now(), 5),
            now() + Duration::days(30)
        );
    }

    #[test]
    fn stop_button_custom_id_round_trips() {
        let custom_id = stop_button_custom_id(123, 456);

        assert_eq!(parse_stop_button_custom_id(&custom_id), Some((123, 456)));
    }
}
