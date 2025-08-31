use chrono::NaiveDate;

#[derive(Debug)]
pub struct MyGuild {
    pub id: i64,
    pub name: String,
    pub members: Vec<MyGuildMember>,
}

#[derive(Debug, Hash, Eq, PartialEq)]
pub struct MyGuildMember {
    pub guild_id: i64,
    pub member_id: i64,
    pub birth: Option<NaiveDate>,
}
