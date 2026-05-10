ALTER TABLE guild_member
    ADD COLUMN last_active_at TIMESTAMPTZ,
    ADD COLUMN last_reminded_at TIMESTAMPTZ,
    ADD COLUMN next_remind_at TIMESTAMPTZ,
    ADD COLUMN remind_count INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN is_remind_opt_out BOOLEAN NOT NULL DEFAULT FALSE;
