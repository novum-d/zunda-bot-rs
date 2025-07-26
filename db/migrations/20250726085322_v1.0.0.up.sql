-- Add up migration script here

CREATE TABLE guild
(
    guild_id VARCHAR(255) PRIMARY KEY,
    name     VARCHAR(255) NOT NULL
);

CREATE TABLE guild_member
(
    member_id VARCHAR(255),
    guild_id  VARCHAR(255),
    nickname  VARCHAR(255),
    birth     TIMESTAMP NOT NULL,
    PRIMARY KEY (member_id, guild_id),
    FOREIGN KEY (guild_id) REFERENCES guild (guild_id)
);