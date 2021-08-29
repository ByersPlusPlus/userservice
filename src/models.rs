use chrono::NaiveDateTime;
use prost_types::Duration;
use super::schema::*;
use super::userservice::BppUser;
use diesel::prelude::*;

#[derive(Queryable, AsChangeset, Identifiable)]
#[primary_key(rank_id)]
#[table_name = "bpp_ranks"]
pub struct Rank {
    pub rank_id: i32,
    pub rank_name: String,
    pub rank_sorting: i32,
    pub hour_requirement_seconds: i64,
    pub hour_requirement_nanos: i32
}

#[derive(Insertable)]
#[table_name = "bpp_ranks"]
pub struct InsertRank {
    pub rank_name: String,
    pub rank_sorting: i32,
    pub hour_requirement_seconds: i64,
    pub hour_requirement_nanos: i32
}

#[derive(Queryable, AsChangeset, Identifiable)]
#[primary_key(group_id)]
#[table_name = "bpp_groups"]
pub struct Group {
    pub group_id: i32,
    pub group_name: String,
    pub bonus_payout: i32
}

#[derive(Insertable)]
#[table_name = "bpp_groups"]
pub struct InsertGroup {
    pub group_name: String,
    pub bonus_payout: i32
}

#[derive(Queryable, Insertable, AsChangeset, Identifiable)]
#[primary_key(channel_id)]
#[table_name="bpp_users"]
pub struct User {
    pub channel_id: String,
    pub display_name: String,
    pub hours_seconds: i64,
    pub hours_nanos: i32,
    pub money: i64,
    pub first_seen_at: NaiveDateTime,
    pub last_seen_at: NaiveDateTime
}

#[derive(Queryable, Insertable, AsChangeset, Identifiable, Associations)]
#[primary_key(group_id, permission)]
#[table_name="bpp_groups_permissions"]
#[belongs_to(Group, foreign_key = "group_id")]
pub struct GroupPermission {
    pub group_id: i32,
    pub permission: String,
    pub granted: bool
}

#[derive(Queryable, Insertable, Identifiable, Associations)]
#[primary_key(group_id, channel_id)]
#[table_name="bpp_groups_users"]
#[belongs_to(Group, foreign_key = "group_id")]
#[belongs_to(User, foreign_key = "channel_id")]
pub struct GroupUser {
    pub group_id: i32,
    pub channel_id: String
}

#[derive(Queryable, Insertable, AsChangeset, Identifiable, Associations)]
#[primary_key(channel_id, permission)]
#[table_name="bpp_users_permissions"]
#[belongs_to(User, foreign_key = "channel_id")]
pub struct UserPermission {
    pub channel_id: String,
    pub permission: String,
    pub granted: bool
}

impl User {
    pub fn new(channel_id: String, display_name: String, hours_seconds: i64, hours_nanos: i32, money: i64, first_seen_at: NaiveDateTime, last_seen_at: NaiveDateTime) -> User {
        User {
            channel_id,
            display_name,
            hours_seconds,
            hours_nanos,
            money,
            first_seen_at,
            last_seen_at
        }
    }

    pub fn get_from_database<S: AsRef<str>>(check_channel_id: S, conn: &PgConnection) -> Option<User> {
        use super::schema::bpp_users::dsl::*;
        bpp_users.filter(channel_id.eq(check_channel_id.as_ref())).first::<User>(conn).ok()
    }

    /// Creates or updates a user in the database.
    pub fn save_to_database(&self, conn: &diesel::PgConnection) -> QueryResult<usize> {
        use super::schema::bpp_users::dsl::*;
        diesel::insert_into(bpp_users)
            .values(self)
            .on_conflict(channel_id)
            .do_update()
            .set(self)
            .execute(conn)
    }

    pub fn check_if_exists(check_channel_id: &String, conn: &diesel::PgConnection) -> bool {
        use super::schema::bpp_users::dsl::*;
        use diesel::dsl::exists;
        use diesel::select;
        let exists: bool = select(exists(bpp_users.filter(channel_id.eq(check_channel_id)))).get_result(conn).unwrap();
        return exists;
    }
}

impl From<BppUser> for User {
    fn from(user: BppUser) -> User {
        let hours = user.hours.unwrap_or(Duration { seconds: 0, nanos: 0 });
        let first_seen_at = user.first_seen_at.unwrap();
        let last_seen_at = user.last_seen_at.unwrap();

        let first_seen_at_naive = NaiveDateTime::from_timestamp(first_seen_at.seconds, first_seen_at.nanos as u32);
        let last_seen_at_naive = NaiveDateTime::from_timestamp(last_seen_at.seconds, last_seen_at.nanos as u32);
        User {
            channel_id: user.channel_id,
            display_name: user.display_name,
            hours_seconds: hours.seconds,
            hours_nanos: hours.nanos,
            money: user.money,
            first_seen_at: first_seen_at_naive,
            last_seen_at: last_seen_at_naive
        }
    }
}

impl From<&BppUser> for User {
    fn from(user: &BppUser) -> User {
        let hours = user.hours.clone().unwrap_or(Duration { seconds: 0, nanos: 0 });
        let first_seen_at = user.first_seen_at.clone().unwrap();
        let last_seen_at = user.last_seen_at.clone().unwrap();

        let first_seen_at_naive = NaiveDateTime::from_timestamp(first_seen_at.seconds, first_seen_at.nanos as u32);
        let last_seen_at_naive = NaiveDateTime::from_timestamp(last_seen_at.seconds, last_seen_at.nanos as u32);
        User {
            channel_id: user.channel_id.clone(),
            display_name: user.display_name.clone(),
            hours_seconds: hours.seconds,
            hours_nanos: hours.nanos,
            money: user.money,
            first_seen_at: first_seen_at_naive,
            last_seen_at: last_seen_at_naive
        }
    }
}