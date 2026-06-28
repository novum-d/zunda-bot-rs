CREATE TABLE guild_notification_channel
(
    guild_id   BIGINT NOT NULL REFERENCES guild (guild_id) ON DELETE CASCADE,
    channel_id BIGINT NOT NULL,
    PRIMARY KEY (guild_id, channel_id)
);
