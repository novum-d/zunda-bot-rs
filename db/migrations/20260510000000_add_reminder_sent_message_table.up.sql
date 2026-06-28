CREATE TABLE reminder_sent_message
(
    guild_id   BIGINT NOT NULL,
    member_id  BIGINT NOT NULL,
    channel_id BIGINT NOT NULL,
    message_id BIGINT NOT NULL,
    PRIMARY KEY (guild_id, member_id, channel_id)
);

INSERT INTO reminder_sent_message (guild_id, member_id, channel_id, message_id)
SELECT guild_id, member_id, reminder_channel_id, reminder_message_id
FROM guild_member
WHERE reminder_channel_id IS NOT NULL
  AND reminder_message_id IS NOT NULL;

ALTER TABLE guild_member
    DROP COLUMN reminder_channel_id,
    DROP COLUMN reminder_message_id;
