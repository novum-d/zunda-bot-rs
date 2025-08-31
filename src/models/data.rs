use chrono::NaiveDate;

#[derive(Debug, sqlx::FromRow)]
pub struct GuildMember {
    pub guild_id: i64,
    pub member_id: i64,
    pub birth: Option<NaiveDate>,
    pub last_notified: Option<NaiveDate>,
}
