ALTER TABLE guild_member
    ADD COLUMN reminder_channel_id BIGINT,
    ADD COLUMN reminder_message_id BIGINT;
