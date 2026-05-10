ALTER TABLE guild_member
    ADD COLUMN reminder_channel_id BIGINT,
    ADD COLUMN reminder_message_id BIGINT;

UPDATE guild_member gm
SET reminder_channel_id = rsm.channel_id,
    reminder_message_id = rsm.message_id
FROM reminder_sent_message rsm
WHERE gm.guild_id = rsm.guild_id
  AND gm.member_id = rsm.member_id;

DROP TABLE reminder_sent_message;
