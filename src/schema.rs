table! {
    bpp_users (channel_id) {
        channel_id -> Varchar,
        display_name -> Varchar,
        hours_seconds -> Int8,
        hours_nanos -> Int4,
        money -> Int8,
        first_seen_at -> Timestamp,
        last_seen_at -> Timestamp,
    }
}
