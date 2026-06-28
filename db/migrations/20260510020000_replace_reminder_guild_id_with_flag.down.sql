ALTER TABLE guild_member
    ADD COLUMN reminder_guild_id BIGINT;

UPDATE guild_member
SET reminder_guild_id = guild_id
WHERE is_reminder_opted_in = TRUE;

ALTER TABLE guild_member
    ADD CONSTRAINT guild_member_reminder_guild_id_fkey
        FOREIGN KEY (reminder_guild_id) REFERENCES guild (guild_id);

ALTER TABLE guild_member
    DROP COLUMN is_reminder_opted_in;
