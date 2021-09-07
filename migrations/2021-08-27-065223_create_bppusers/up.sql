-- Your SQL goes here
CREATE TABLE bpp_users (
    channel_id VARCHAR PRIMARY KEY,
    display_name VARCHAR NOT NULL,
    hours_seconds BIGINT NOT NULL,
    money DOUBLE PRECISION NOT NULL,
    first_seen_at TIMESTAMP NOT NULL,
    last_seen_at TIMESTAMP NOT NULL
)