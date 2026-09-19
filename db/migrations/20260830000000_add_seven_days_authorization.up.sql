CREATE TABLE seven_days_config
(
    guild_id   BIGINT PRIMARY KEY REFERENCES guild (guild_id) ON DELETE CASCADE,
    channel_id BIGINT  NOT NULL,
    enabled    BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TABLE seven_days_operator
(
    guild_id      BIGINT      NOT NULL REFERENCES seven_days_config (guild_id) ON DELETE CASCADE,
    operator_kind VARCHAR(4)  NOT NULL CHECK (operator_kind IN ('user', 'role')),
    operator_id   BIGINT      NOT NULL,
    is_admin      BOOLEAN     NOT NULL DEFAULT FALSE,
    PRIMARY KEY (guild_id, operator_kind, operator_id)
);
