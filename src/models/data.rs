use chrono::{DateTime, NaiveDate, Utc};

#[derive(Debug, sqlx::FromRow)]
pub struct GuildMember {
    pub guild_id: i64,
    pub member_id: i64,
    pub birth: Option<NaiveDate>,
    pub last_notified: Option<NaiveDate>,
    pub last_active_at: Option<DateTime<Utc>>,
    pub last_reminded_at: Option<DateTime<Utc>>,
    pub next_remind_at: Option<DateTime<Utc>>,
    pub remind_count: i32,
    pub is_remind_opt_out: bool,
    pub reminder_guild_id: Option<i64>,
}
