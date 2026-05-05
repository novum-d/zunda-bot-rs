ALTER TABLE guild_member
    ADD COLUMN reminder_guild_id BIGINT;

ALTER TABLE guild_member
    ADD CONSTRAINT guild_member_reminder_guild_id_fkey
    FOREIGN KEY (reminder_guild_id) REFERENCES guild (guild_id);
