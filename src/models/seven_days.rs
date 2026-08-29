#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SevenDaysConfig {
    pub guild_id: i64,
    pub channel_id: i64,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SevenDaysOperator {
    pub guild_id: i64,
    pub operator_kind: String,
    pub operator_id: i64,
    pub is_admin: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SevenDaysAuthorization {
    pub config: SevenDaysConfig,
    pub operators: Vec<SevenDaysOperator>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SevenDaysOperatorKind {
    User,
    Role,
}

impl SevenDaysOperatorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Role => "role",
        }
    }
}
