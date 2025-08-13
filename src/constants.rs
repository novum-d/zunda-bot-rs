use sqlx::types::chrono::{DateTime, FixedOffset, Utc};

pub const EPOCH: DateTime<Utc> = DateTime::<Utc>::from_timestamp(0, 0).unwrap();

pub const JST: FixedOffset = FixedOffset::east_opt(9 * 3600).unwrap(); // JST（UTC+9）