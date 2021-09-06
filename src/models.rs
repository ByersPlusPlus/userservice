use std::ops::Deref;

use super::schema::*;
use super::userservice::{BppUser, BppGroup};
use crate::{bpp_foreign_model_impl, bpp_model_impl};
use chrono::NaiveDateTime;
use diesel::prelude::*;
use prost_types::Duration;

#[derive(Queryable, AsChangeset, Identifiable)]
#[primary_key(rank_id)]
#[table_name = "bpp_ranks"]
pub struct Rank {
    pub rank_id: i32,
    pub rank_name: String,
    pub rank_sorting: i32,
    pub hour_requirement_seconds: i64,
    pub hour_requirement_nanos: i32,
}

#[derive(Insertable)]
#[table_name = "bpp_ranks"]
pub struct InsertRank {
    pub rank_name: String,
    pub rank_sorting: i32,
    pub hour_requirement_seconds: i64,
    pub hour_requirement_nanos: i32,
}

#[derive(Queryable, AsChangeset, Identifiable, PartialEq, Eq)]
#[primary_key(group_id)]
#[table_name = "bpp_groups"]
pub struct Group {
    pub group_id: i32,
    pub group_name: String,
    pub bonus_payout: i32,
    pub group_sorting: i32
}

#[derive(Insertable)]
#[table_name = "bpp_groups"]
pub struct InsertGroup {
    pub group_name: String,
    pub bonus_payout: i32,
    pub group_sorting: i32
}

#[derive(Queryable, Insertable, AsChangeset, Identifiable)]
#[primary_key(channel_id)]
#[table_name = "bpp_users"]
pub struct User {
    pub channel_id: String,
    pub display_name: String,
    pub hours_seconds: i64,
    pub hours_nanos: i32,
    pub money: i64,
    pub first_seen_at: NaiveDateTime,
    pub last_seen_at: NaiveDateTime,
}

#[derive(Queryable, Insertable, AsChangeset, Identifiable, Associations)]
#[primary_key(group_id, permission)]
#[table_name = "bpp_groups_permissions"]
#[belongs_to(Group, foreign_key = "group_id")]
pub struct GroupPermission {
    pub group_id: i32,
    pub permission: String,
    pub granted: bool,
}

#[derive(Queryable, Insertable, Identifiable, Associations)]
#[primary_key(group_id, channel_id)]
#[table_name = "bpp_groups_users"]
#[belongs_to(Group, foreign_key = "group_id")]
#[belongs_to(User, foreign_key = "channel_id")]
pub struct GroupUser {
    pub group_id: i32,
    pub channel_id: String,
}

#[derive(Queryable, Insertable, AsChangeset, Identifiable, Associations)]
#[primary_key(channel_id, permission)]
#[table_name = "bpp_users_permissions"]
#[belongs_to(User, foreign_key = "channel_id")]
pub struct UserPermission {
    pub channel_id: String,
    pub permission: String,
    pub granted: bool,
}

bpp_foreign_model_impl!(
    get_permissions_for_user,
    UserPermission,
    channel_id,
    String,
    crate::schema::bpp_users_permissions::dsl,
    bpp_users_permissions
);
bpp_foreign_model_impl!(
    get_permissions_for_group,
    GroupPermission,
    group_id,
    i32,
    crate::schema::bpp_groups_permissions::dsl,
    bpp_groups_permissions
);
bpp_foreign_model_impl!(
    get_users_for_group,
    User,
    group_id,
    i32,
    crate::schema::bpp_groups_users::dsl,
    crate::schema::bpp_users::dsl,
    bpp_groups_users,
    bpp_users
);
bpp_foreign_model_impl!(
    get_groups_for_user,
    Group,
    channel_id,
    String,
    crate::schema::bpp_groups_users::dsl,
    crate::schema::bpp_groups::dsl,
    bpp_groups_users,
    bpp_groups
);

bpp_model_impl!(
    Group,
    InsertGroup,
    group_id,
    i32,
    crate::schema::bpp_groups::dsl,
    bpp_groups
);
bpp_model_impl!(
    User,
    channel_id,
    String,
    crate::schema::bpp_users::dsl,
    bpp_users
);
bpp_model_impl!(
    Rank,
    InsertRank,
    rank_id,
    i32,
    crate::schema::bpp_ranks::dsl,
    bpp_ranks
);

impl From<GroupPermission> for String {
    fn from(gp: GroupPermission) -> String {
        gp.permission
    }
}

pub struct PermissionStrings(Vec<String>);

impl Deref for PermissionStrings {
    type Target = Vec<String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Vec<GroupPermission>> for PermissionStrings {
    fn from(permissions: Vec<GroupPermission>) -> Self {
        let mut permissions: Vec<String> =
            permissions.into_iter().map(|gp| gp.permission).collect();
        PermissionStrings(permissions)
    }
}

impl From<UserPermission> for String {
    fn from(up: UserPermission) -> String {
        up.permission
    }
}

impl From<Vec<UserPermission>> for PermissionStrings {
    fn from(permissions: Vec<UserPermission>) -> Self {
        let mut permissions: Vec<String> =
            permissions.into_iter().map(|up| up.permission).collect();
        PermissionStrings(permissions)
    }
}

// impl PartialEq for Group {
//     fn eq(&self, other: &Self) -> bool {
//         self.group_id == other.group_id
//     }
// }
impl PartialOrd for Group {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.group_sorting.partial_cmp(&other.group_sorting)
    }
}

impl Ord for Group {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.group_sorting.cmp(&other.group_sorting)
    }
}

impl User {
    pub fn new(
        channel_id: String,
        display_name: String,
        hours_seconds: i64,
        hours_nanos: i32,
        money: i64,
        first_seen_at: NaiveDateTime,
        last_seen_at: NaiveDateTime,
    ) -> User {
        User {
            channel_id,
            display_name,
            hours_seconds,
            hours_nanos,
            money,
            first_seen_at,
            last_seen_at,
        }
    }

    pub fn check_if_exists(check_channel_id: &String, conn: &diesel::PgConnection) -> bool {
        use super::schema::bpp_users::dsl::*;
        use diesel::dsl::exists;
        use diesel::select;
        let exists: bool = select(exists(bpp_users.filter(channel_id.eq(check_channel_id))))
            .get_result(conn)
            .unwrap();
        return exists;
    }

    pub fn get_active_rank(&self, conn: &diesel::PgConnection) -> Option<Rank> {
        use super::schema::bpp_ranks::dsl::*;

        // Get all ranks which match the hour requirements and sort by the sorting field
        let rank: Option<Rank> = bpp_ranks
            .filter(hour_requirement_seconds.ge(self.hours_seconds))
            .filter(hour_requirement_nanos.ge(self.hours_nanos))
            .order(rank_sorting.desc())
            .first::<Rank>(conn).ok();
        return rank;
    }

    pub fn to_userservice_user(self, conn: &diesel::PgConnection) -> BppUser {
        let mut prost_duration = prost_types::Duration::default();
        prost_duration.seconds = self.hours_seconds;
        prost_duration.nanos = self.hours_nanos;

        let first_seen_at_ts = prost_types::Timestamp {
            seconds: self.first_seen_at.timestamp() as i64,
            nanos: self.first_seen_at.timestamp_subsec_nanos() as i32,
        };
        let last_seen_at_ts = prost_types::Timestamp {
            seconds: self.last_seen_at.timestamp() as i64,
            nanos: self.last_seen_at.timestamp_subsec_nanos() as i32,
        };

        let groups = Group::get_groups_for_user(self.channel_id.clone(), &conn);
        let permissions =
            UserPermission::get_permissions_for_user(self.channel_id.clone(), &conn);
        let permissions: Vec<super::userservice::Permission> = permissions.into_iter()
            .map(|p|super::userservice::Permission {
                permission: p.permission,
                granted: p.granted,
            })
            .collect();
        let groups = groups
            .iter()
            .map(|group| {
                let permissions =
                    GroupPermission::get_permissions_for_group(group.group_id, &conn);
                let permissions = permissions.into_iter()
                    .map(|p|super::userservice::Permission {
                        permission: p.permission,
                        granted: p.granted,
                    })
                    .collect();

                super::userservice::BppGroup {
                    group_id: group.group_id,
                    group_name: group.group_name.clone(),
                    permissions,
                    bonus_payout: group.bonus_payout,
                    group_sorting: group.group_sorting,
                }
            })
            .collect::<Vec<super::userservice::BppGroup>>();

        let rank = if let Some(rank) = self.get_active_rank(&conn) {
            rank.rank_name
        } else {
            "default".to_string()
        };

        let bpp_user = BppUser {
            channel_id: self.channel_id,
            display_name: self.display_name,
            hours: Some(prost_duration),
            money: self.money,
            first_seen_at: Some(first_seen_at_ts),
            last_seen_at: Some(last_seen_at_ts),
            groups,
            permissions,
            rank
        };
        return bpp_user;
    }
}

impl From<BppGroup> for Group {
    fn from(bpp_group: BppGroup) -> Self {
        Group {
            group_id: bpp_group.group_id,
            group_name: bpp_group.group_name,
            bonus_payout: bpp_group.bonus_payout,
            group_sorting: bpp_group.group_sorting,
        }
    }
}

impl From<&BppGroup> for Group {
    fn from(bpp_group: &BppGroup) -> Self {
        Group {
            group_id: bpp_group.group_id,
            group_name: bpp_group.group_name.clone(),
            bonus_payout: bpp_group.bonus_payout,
            group_sorting: bpp_group.group_sorting,
        }
    }
}

impl From<BppUser> for User {
    fn from(user: BppUser) -> User {
        let hours = user.hours.unwrap_or(Duration {
            seconds: 0,
            nanos: 0,
        });
        let first_seen_at = user.first_seen_at.unwrap();
        let last_seen_at = user.last_seen_at.unwrap();

        let first_seen_at_naive =
            NaiveDateTime::from_timestamp(first_seen_at.seconds, first_seen_at.nanos as u32);
        let last_seen_at_naive =
            NaiveDateTime::from_timestamp(last_seen_at.seconds, last_seen_at.nanos as u32);
        User {
            channel_id: user.channel_id,
            display_name: user.display_name,
            hours_seconds: hours.seconds,
            hours_nanos: hours.nanos,
            money: user.money,
            first_seen_at: first_seen_at_naive,
            last_seen_at: last_seen_at_naive,
        }
    }
}

impl From<&BppUser> for User {
    fn from(user: &BppUser) -> User {
        let hours = user.hours.clone().unwrap_or(Duration {
            seconds: 0,
            nanos: 0,
        });
        let first_seen_at = user.first_seen_at.clone().unwrap();
        let last_seen_at = user.last_seen_at.clone().unwrap();

        let first_seen_at_naive =
            NaiveDateTime::from_timestamp(first_seen_at.seconds, first_seen_at.nanos as u32);
        let last_seen_at_naive =
            NaiveDateTime::from_timestamp(last_seen_at.seconds, last_seen_at.nanos as u32);
        User {
            channel_id: user.channel_id.clone(),
            display_name: user.display_name.clone(),
            hours_seconds: hours.seconds,
            hours_nanos: hours.nanos,
            money: user.money,
            first_seen_at: first_seen_at_naive,
            last_seen_at: last_seen_at_naive,
        }
    }
}
