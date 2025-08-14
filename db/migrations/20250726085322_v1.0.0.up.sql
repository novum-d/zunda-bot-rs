-- Add up migration script here

CREATE TABLE guild
(
    guild_id BIGINT PRIMARY KEY,
    name     VARCHAR(255) NOT NULL
);

CREATE TABLE guild_member
(
    member_id BIGINT,
    guild_id  BIGINT,
    nickname  VARCHAR(255),
    birth     TIMESTAMP,
    PRIMARY KEY (member_id, guild_id),
    FOREIGN KEY (guild_id) REFERENCES guild (guild_id)
);