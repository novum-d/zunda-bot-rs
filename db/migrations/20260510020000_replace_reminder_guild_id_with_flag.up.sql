ALTER TABLE guild_member
    ADD COLUMN is_reminder_opted_in BOOLEAN NOT NULL DEFAULT FALSE;

UPDATE guild_member
SET is_reminder_opted_in = TRUE
WHERE reminder_guild_id IS NOT NULL
  AND reminder_guild_id = guild_id;

ALTER TABLE guild_member
    DROP CONSTRAINT guild_member_reminder_guild_id_fkey;

ALTER TABLE guild_member
    DROP COLUMN reminder_guild_id;
