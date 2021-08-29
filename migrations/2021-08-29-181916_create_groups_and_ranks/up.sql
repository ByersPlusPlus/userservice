-- Your SQL goes here
CREATE TABLE bpp_groups (
    group_id SERIAL PRIMARY KEY,
    group_name VARCHAR NOT NULL,
    bonus_payout INTEGER NOT NULL
);

CREATE TABLE bpp_groups_permissions (
    group_id INTEGER NOT NULL REFERENCES bpp_groups(group_id),
    permission VARCHAR NOT NULL,
    granted BOOLEAN NOT NULL,
    PRIMARY KEY(group_id, permission)
);

CREATE TABLE bpp_ranks (
    rank_id SERIAL PRIMARY KEY,
    rank_name VARCHAR NOT NULL,
    rank_sorting INTEGER NOT NULL,
    hour_requirement_seconds BIGINT NOT NULL,
    hour_requirement_nanos INTEGER NOT NULL
);

CREATE TABLE bpp_groups_users (
    group_id INTEGER NOT NULL REFERENCES bpp_groups(group_id),
    channel_id VARCHAR NOT NULL REFERENCES bpp_users(channel_id),
    PRIMARY KEY(group_id, channel_id)
);

CREATE TABLE bpp_users_permissions (
    channel_id VARCHAR NOT NULL REFERENCES bpp_users(channel_id),
    permission VARCHAR NOT NULL,
    granted BOOLEAN NOT NULL,
    PRIMARY KEY(channel_id, permission)
);