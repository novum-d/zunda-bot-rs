ALTER TABLE guild_member
    DROP COLUMN is_remind_opt_out,
    DROP COLUMN remind_count,
    DROP COLUMN next_remind_at,
    DROP COLUMN last_reminded_at,
    DROP COLUMN last_active_at;
