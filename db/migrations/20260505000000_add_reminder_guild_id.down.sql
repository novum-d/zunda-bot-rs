ALTER TABLE guild_member
    DROP CONSTRAINT guild_member_reminder_guild_id_fkey;

ALTER TABLE guild_member
    DROP COLUMN reminder_guild_id;
