table! {
    bpp_groups (group_id) {
        group_id -> Int4,
        group_name -> Varchar,
        bonus_payout -> Int4,
        group_sorting -> Int4,
    }
}

table! {
    bpp_groups_permissions (group_id, permission) {
        group_id -> Int4,
        permission -> Varchar,
        granted -> Bool,
    }
}

table! {
    bpp_groups_users (group_id, channel_id) {
        group_id -> Int4,
        channel_id -> Varchar,
    }
}

table! {
    bpp_ranks (rank_id) {
        rank_id -> Int4,
        rank_name -> Varchar,
        rank_sorting -> Int4,
        hour_requirement_seconds -> Int8,
        hour_requirement_nanos -> Int4,
    }
}

table! {
    bpp_users (channel_id) {
        channel_id -> Varchar,
        display_name -> Varchar,
        hours_seconds -> Int8,
        hours_nanos -> Int4,
        money -> Float8,
        first_seen_at -> Timestamp,
        last_seen_at -> Timestamp,
    }
}

table! {
    bpp_users_permissions (channel_id, permission) {
        channel_id -> Varchar,
        permission -> Varchar,
        granted -> Bool,
    }
}

joinable!(bpp_groups_permissions -> bpp_groups (group_id));
joinable!(bpp_groups_users -> bpp_groups (group_id));
joinable!(bpp_groups_users -> bpp_users (channel_id));
joinable!(bpp_users_permissions -> bpp_users (channel_id));

allow_tables_to_appear_in_same_query!(
    bpp_groups,
    bpp_groups_permissions,
    bpp_groups_users,
    bpp_ranks,
    bpp_users,
    bpp_users_permissions,
);
