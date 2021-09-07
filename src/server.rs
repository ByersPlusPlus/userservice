#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate serde;

use std::env;
use std::net::SocketAddr;

use ::log::{debug, error, info};
use chrono::NaiveDateTime;
use chrono::Utc;
use diesel::prelude::*;
use diesel::r2d2::ConnectionManager;
use diesel::PgConnection;
use diesel_migrations::embed_migrations;
use dotenv::dotenv;
use models::{Group, GroupPermission, InsertGroup, InsertRank, User, UserPermission, Rank};
use r2d2::Pool;
use tonic::transport::Channel;
use tonic::Request;

use userservice::user_service_server::{UserService, UserServiceServer};
use userservice::{BppGroup, BppUser};
use youtubeservice::you_tube_service_client::YouTubeServiceClient;

use crate::log::setup_log;
use crate::settings::Settings;

mod settings;
mod log;
mod macros;
mod models;
mod schema;

embed_migrations!();

pub mod youtubeservice {
    tonic::include_proto!("youtubeservice");
}

pub mod userservice {
    tonic::include_proto!("userservice");
}

type Void = Result<(), Box<dyn std::error::Error>>;
type DbPool = Pool<ConnectionManager<PgConnection>>;

pub fn connect_to_database() -> Pool<ConnectionManager<PgConnection>> {
    // Get the database URL from the environment
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let manager = ConnectionManager::new(database_url);
    // Create a connection pool of 10 connections
    let pool = Pool::builder().max_size(10).build(manager).unwrap();

    // Run migrations
    let _ = embedded_migrations::run_with_output(&pool.get().unwrap(), &mut std::io::stdout());

    return pool;
}

fn calculate_hours_and_money(user: &mut User, now: &NaiveDateTime, settings: Settings, conn: &PgConnection) {
    let new_hours_seconds;
    let hours_duration = chrono::Duration::seconds(user.hours_seconds);
    let new_duration = *now - user.last_seen_at;
    debug!("Between the last time the user was seen and now, {} seconds have passed", new_duration.num_seconds());
    let hours = hours_duration + new_duration;
    new_hours_seconds = hours.num_seconds();
    debug!(
        "Updating hours of {} ({}) from {}s to {}s",
        user.channel_id,
        user.display_name,
        user.hours_seconds,
        new_hours_seconds
    );

    user.hours_seconds = new_hours_seconds;

    // Grant x money per minute
    let mut money_per_minute: f64 = settings.default_payout as f64;
    let user_groups = Group::get_groups_for_user(user.channel_id.clone(), conn);
    for group in user_groups {
        money_per_minute += group.bonus_payout as f64;
    }
    let money_per_second: f64 = money_per_minute / 60.0;

    let new_money = user.money + money_per_second * new_duration.num_seconds() as f64;
    debug!(
        "Updating money of {} ({}) from {:.2} to {:.2}",
        user.channel_id, user.display_name, user.money, new_money
    );
    user.money = new_money;
}

async fn fetch_users_from_messages(
    youtube_client: &mut YouTubeServiceClient<Channel>,
    pool: &DbPool,
) -> Void {
    let mut stream = youtube_client
        .subscribe_messages(Request::new(()))
        .await?
        .into_inner();

    while let Some(message) = stream.message().await? {
        let conn = pool.get()?;
        let now = Utc::now().naive_utc();
        let mut user = if User::check_if_exists(&message.channel_id, &conn) {
            debug!("Updating existing user {}", &message.channel_id);
            // Update the user
            User::get_from_database(&message.channel_id, &conn).unwrap()
        } else {
            debug!("Creating new user {}", &message.channel_id);
            // Create the user
            User::new(
                message.channel_id.clone(),
                message.display_name.clone(),
                0,
                0,
                0 as f64,
                now,
                now,
            )
        };

        user.display_name = message.display_name.clone();

        let settings = Settings::new()?;

        // Determine if user was active before this message and if so, update the hours
        // if the user has been last seen less than the configured timeframe, update the hours
        if user.last_seen_at + chrono::Duration::seconds(settings.active_time as i64) > now {
            calculate_hours_and_money(&mut user, &now, settings, &conn);
        }
        user.last_seen_at = now;

        // Update the user
        user.save_to_database(&conn).unwrap();
    }

    return Ok(());
}

pub struct UserServer {
    database_pool: DbPool
}

#[tonic::async_trait]
impl UserService for UserServer {
    async fn get_user_by_id(
        &self,
        request: tonic::Request<userservice::BppUserById>,
    ) -> Result<tonic::Response<userservice::BppUser>, tonic::Status> {
        let user_id = request.into_inner().channel_id;
        let conn = self.database_pool.get().unwrap();
        let potential_user = User::get_from_database(&user_id, &conn);

        match potential_user {
            Some(user) => {
                let bpp_user = user.to_userservice_user(&conn);
                return Ok(tonic::Response::new(bpp_user));
            }
            None => Err(tonic::Status::not_found("User not found")),
        }
    }

    async fn filter_users(
        &self,
        request: tonic::Request<userservice::BppUserFilters>,
    ) -> Result<tonic::Response<userservice::BppUsers>, tonic::Status> {
        let filter_request = request.into_inner();
        let filters = &filter_request.filters;
        let conn = self.database_pool.get().unwrap();

        use schema::bpp_users::dsl::*;
        let mut query = bpp_users.into_boxed();
        for filter in filters {
            let inner_filter = filter.filter.as_ref().unwrap();
            match inner_filter {
                userservice::bpp_user_filter::Filter::ChannelId(filter_channel_id) => {
                    query = query.filter(channel_id.eq(filter_channel_id));
                }
                userservice::bpp_user_filter::Filter::Name(filter_name) => {
                    query = query.filter(display_name.eq(filter_name));
                }
                userservice::bpp_user_filter::Filter::Hours(filter_hours) => {
                    query = query.filter(hours_seconds.eq(filter_hours));
                }
                userservice::bpp_user_filter::Filter::Money(filter_money) => {
                    query = query.filter(money.eq(filter_money));
                }
            }
        }

        match filter_request.sorting() {
            userservice::bpp_user_filters::SortingFields::HoursAsc => {
                query = query.order_by(hours_seconds.asc());
            }
            userservice::bpp_user_filters::SortingFields::HoursDesc => {
                query = query.order_by(hours_seconds.desc());
            }
            userservice::bpp_user_filters::SortingFields::MoneyAsc => {
                query = query.order_by(money.asc());
            }
            userservice::bpp_user_filters::SortingFields::MoneyDesc => {
                query = query.order_by(money.desc());
            }
            userservice::bpp_user_filters::SortingFields::Default => {}
        }
        let users = match query.load::<User>(&conn) {
            Ok(users) => users,
            Err(e) => {
                error!("{}", e);
                return Err(tonic::Status::internal("Failed to load users"));
            }
        };
        let users: Vec<BppUser> = users
            .into_iter()
            .map(|user| user.to_userservice_user(&conn))
            .collect();
        let count = users.len() as i32;

        return Ok(tonic::Response::new(userservice::BppUsers { users, count }));
    }

    async fn update_user(
        &self,
        request: tonic::Request<userservice::BppUser>,
    ) -> Result<tonic::Response<userservice::BppUser>, tonic::Status> {
        let user = request.into_inner();
        let conn = self.database_pool.get().unwrap();
        let db_user: User = (&user).into();
        db_user.save_to_database(&conn).unwrap();
        return Ok(tonic::Response::new(user));
    }

    async fn update_users(
        &self,
        request: tonic::Request<userservice::BppUsers>,
    ) -> Result<tonic::Response<userservice::BppUsers>, tonic::Status> {
        let users = request.into_inner();
        let conn = self.database_pool.get().unwrap();
        for user in &users.users {
            let db_user: User = user.into();
            db_user.save_to_database(&conn).unwrap();
        }
        return Ok(tonic::Response::new(users));
    }

    async fn delete_user(
        &self,
        request: tonic::Request<userservice::BppUserId>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let user_id = request.into_inner().channel_id;
        let conn = self.database_pool.get().unwrap();
        use schema::bpp_users::dsl::*;
        diesel::delete(bpp_users.filter(channel_id.eq(user_id)))
            .execute(&conn)
            .unwrap();
        return Ok(tonic::Response::new(()));
    }

    async fn delete_users(
        &self,
        request: tonic::Request<userservice::BppUserIds>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let user_ids = request.into_inner().users;
        let conn = self.database_pool.get().unwrap();
        use schema::bpp_users::dsl::*;
        diesel::delete(bpp_users.filter(channel_id.eq_any(user_ids)))
            .execute(&conn)
            .unwrap();
        return Ok(tonic::Response::new(()));
    }

    async fn create_user(
        &self,
        request: tonic::Request<userservice::BppUser>,
    ) -> Result<tonic::Response<userservice::BppUser>, tonic::Status> {
        let user = request.into_inner();
        let conn = self.database_pool.get().unwrap();
        let db_user: User = (&user).into();
        db_user.save_to_database(&conn).unwrap();
        return Ok(tonic::Response::new(user));
    }

    async fn user_has_permission(
        &self,
        request: tonic::Request<userservice::UserPermissionCheck>,
    ) -> Result<tonic::Response<bool>, tonic::Status> {
        let check = request.into_inner();
        let conn = self.database_pool.get().unwrap();

        let user_id = check.channel_id;
        let permission = check.permission;
        let mut has_permission = check.granted_default;

        let mut user_groups = Group::get_groups_for_user(user_id.clone(), &conn);
        user_groups.sort_by(|a, b| a.cmp(b));
        for group in user_groups {
            let group_permissions =
                GroupPermission::get_permissions_for_group(group.group_id, &conn);
            let searched_permission = group_permissions
                .iter()
                .find(|group_permission| group_permission.permission == permission);
            if searched_permission.is_some() {
                has_permission = searched_permission.unwrap().granted;
            }
        }

        let user_permissions = UserPermission::get_permissions_for_user(user_id.clone(), &conn);
        let searched_permission = user_permissions
            .iter()
            .find(|perm| perm.permission == permission);
        if searched_permission.is_some() {
            has_permission = searched_permission.unwrap().granted;
        }

        return Ok(tonic::Response::new(has_permission));
    }

    async fn get_groups(
        &self,
        _: tonic::Request<()>,
    ) -> Result<tonic::Response<userservice::BppGroups>, tonic::Status> {
        let conn = self.database_pool.get().unwrap();
        use schema::bpp_groups::dsl::*;
        let groups = bpp_groups
            .order(group_sorting.desc())
            .load::<Group>(&conn)
            .unwrap();
        let groups: Vec<BppGroup> = groups
            .into_iter()
            .map(|group| {
                let group_permissions =
                    GroupPermission::get_permissions_for_group(group.group_id, &conn);
                let group_permissions: Vec<userservice::Permission> = group_permissions
                    .into_iter()
                    .map(|p| userservice::Permission {
                        permission: p.permission,
                        granted: p.granted,
                    })
                    .collect();

                BppGroup {
                    group_id: group.group_id,
                    group_name: group.group_name,
                    permissions: group_permissions,
                    bonus_payout: group.bonus_payout,
                    group_sorting: group.group_sorting,
                }
            })
            .collect();
        let count = groups.len() as i32;
        return Ok(tonic::Response::new(userservice::BppGroups {
            groups,
            count,
        }));
    }

    async fn update_group(
        &self,
        request: tonic::Request<userservice::BppGroup>,
    ) -> Result<tonic::Response<userservice::BppGroup>, tonic::Status> {
        let group = request.into_inner();
        let conn = self.database_pool.get().unwrap();
        let db_group: Group = (&group).into();
        db_group.save_to_database(&conn).unwrap();
        return Ok(tonic::Response::new(group));
    }

    async fn update_groups(
        &self,
        request: tonic::Request<userservice::BppGroups>,
    ) -> Result<tonic::Response<userservice::BppGroups>, tonic::Status> {
        let groups = request.into_inner();
        let conn = self.database_pool.get().unwrap();
        for group in &groups.groups {
            let db_group: Group = group.into();
            db_group.save_to_database(&conn).unwrap();
        }
        return Ok(tonic::Response::new(groups));
    }

    async fn delete_group(
        &self,
        request: tonic::Request<userservice::BppGroupId>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let id = request.into_inner().group_id;
        let conn = self.database_pool.get().unwrap();
        use schema::bpp_groups::dsl::*;
        diesel::delete(bpp_groups.filter(group_id.eq(id)))
            .execute(&conn)
            .unwrap();
        return Ok(tonic::Response::new(()));
    }

    async fn delete_groups(
        &self,
        request: tonic::Request<userservice::BppGroupIds>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let group_ids = request.into_inner().groups;
        let conn = self.database_pool.get().unwrap();
        use schema::bpp_groups::dsl::*;
        diesel::delete(bpp_groups.filter(group_id.eq_any(group_ids)))
            .execute(&conn)
            .unwrap();
        return Ok(tonic::Response::new(()));
    }

    async fn create_group(
        &self,
        request: tonic::Request<userservice::CreateBppGroup>,
    ) -> Result<tonic::Response<userservice::BppGroup>, tonic::Status> {
        let create_group = request.into_inner();
        let conn = self.database_pool.get().unwrap();
        let db_group: InsertGroup = create_group.into();
        let created_group = db_group.save_to_database(&conn).unwrap();

        let group_permissions =
            GroupPermission::get_permissions_for_group(created_group.group_id, &conn);
        let group_permissions: Vec<userservice::Permission> = group_permissions
            .into_iter()
            .map(|p| userservice::Permission {
                permission: p.permission,
                granted: p.granted,
            })
            .collect();

        let group = BppGroup {
            group_id: created_group.group_id,
            group_name: created_group.group_name,
            permissions: group_permissions,
            bonus_payout: created_group.bonus_payout,
            group_sorting: created_group.group_sorting,
        };
        return Ok(tonic::Response::new(group));
    }

    async fn get_ranks(
        &self,
        _: tonic::Request<()>,
    ) -> Result<tonic::Response<userservice::BppRanks>, tonic::Status> {
        let conn = self.database_pool.get().unwrap();
        use schema::bpp_ranks::dsl::*;
        let ranks = bpp_ranks
            .order(rank_sorting.desc())
            .load::<Rank>(&conn)
            .unwrap();
        let ranks: Vec<userservice::BppRank> = ranks
            .into_iter()
            .map(|rank| {
                let hour_requirement = prost_types::Duration {
                    seconds: rank.hour_requirement_seconds,
                    nanos: rank.hour_requirement_nanos,
                };

                userservice::BppRank {
                    rank_id: rank.rank_id,
                    rank_name: rank.rank_name,
                    rank_sorting: rank.rank_sorting,
                    hour_requirement: Some(hour_requirement),
                }
            })
            .collect();
        let count = ranks.len() as i32;
        return Ok(tonic::Response::new(userservice::BppRanks { ranks, count }));
    }

    async fn update_rank(
        &self,
        request: tonic::Request<userservice::BppRank>,
    ) -> Result<tonic::Response<userservice::BppRank>, tonic::Status> {
        let rank = request.into_inner();
        let conn = self.database_pool.get().unwrap();
        let db_rank: Rank = (&rank).into();
        db_rank.save_to_database(&conn).unwrap();
        return Ok(tonic::Response::new(rank));
    }

    async fn update_ranks(
        &self,
        request: tonic::Request<userservice::BppRanks>,
    ) -> Result<tonic::Response<userservice::BppRanks>, tonic::Status> {
        let ranks = request.into_inner();
        let conn = self.database_pool.get().unwrap();
        for rank in &ranks.ranks {
            let db_rank: Rank = rank.into();
            db_rank.save_to_database(&conn).unwrap();
        }
        return Ok(tonic::Response::new(ranks));
    }

    async fn delete_rank(
        &self,
        request: tonic::Request<userservice::BppRankId>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let id = request.into_inner().rank_id;
        let conn = self.database_pool.get().unwrap();
        use schema::bpp_ranks::dsl::*;
        diesel::delete(bpp_ranks.filter(rank_id.eq(id)))
            .execute(&conn)
            .unwrap();
        return Ok(tonic::Response::new(()));
    }

    async fn delete_ranks(
        &self,
        request: tonic::Request<userservice::BppRankIds>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let rank_ids = request.into_inner().ranks;
        let conn = self.database_pool.get().unwrap();
        use schema::bpp_ranks::dsl::*;
        diesel::delete(bpp_ranks.filter(rank_id.eq_any(rank_ids)))
            .execute(&conn)
            .unwrap();
        return Ok(tonic::Response::new(()));
    }

    async fn create_rank(
        &self,
        request: tonic::Request<userservice::CreateBppRank>,
    ) -> Result<tonic::Response<userservice::BppRank>, tonic::Status> {
        let create_rank = request.into_inner();
        let conn = self.database_pool.get().unwrap();
        let db_rank: InsertRank = create_rank.into();
        let created_rank = db_rank.save_to_database(&conn).unwrap();
        let hour_requirement = prost_types::Duration {
            seconds: created_rank.hour_requirement_seconds,
            nanos: created_rank.hour_requirement_nanos,
        };
        let rank = userservice::BppRank {
            rank_id: created_rank.rank_id,
            rank_name: created_rank.rank_name,
            rank_sorting: created_rank.rank_sorting,
            hour_requirement: Some(hour_requirement),
        };
        return Ok(tonic::Response::new(rank));
    }

    async fn user_grant_permission(
        &self,
        request: tonic::Request<userservice::UserPermission>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let granted_permission = request.into_inner();
        let conn = self.database_pool.get().unwrap();
        use schema::bpp_users_permissions::dsl::*;
        let db_permission = models::UserPermission {
            channel_id: granted_permission.channel_id,
            permission: granted_permission.permission,
            granted: true
        };
        diesel::insert_into(bpp_users_permissions)
            .values(&db_permission)
            .execute(&conn)
            .unwrap();
        return Ok(tonic::Response::new(()));
    }

    async fn user_revoke_permisison(
        &self,
        request: tonic::Request<userservice::UserPermission>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let revoked_permission = request.into_inner();
        let conn = self.database_pool.get().unwrap();
        use schema::bpp_users_permissions::dsl::*;
        let db_permission = models::UserPermission {
            channel_id: revoked_permission.channel_id,
            permission: revoked_permission.permission,
            granted: false
        };
        diesel::insert_into(bpp_users_permissions)
            .values(&db_permission)
            .execute(&conn)
            .unwrap();
        return Ok(tonic::Response::new(()));
    }

    async fn group_grant_permission(
        &self,
        request: tonic::Request<userservice::GroupPermission>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let granted_permission = request.into_inner();
        let conn = self.database_pool.get().unwrap();
        use schema::bpp_groups_permissions::dsl::*;
        let db_permission = models::GroupPermission {
            group_id: granted_permission.group_id,
            permission: granted_permission.permission,
            granted: true
        };
        diesel::insert_into(bpp_groups_permissions)
            .values(&db_permission)
            .execute(&conn)
            .unwrap();
        return Ok(tonic::Response::new(()));
    }

    async fn group_revoke_permission(
        &self,
        request: tonic::Request<userservice::GroupPermission>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let revoked_permission = request.into_inner();
        let conn = self.database_pool.get().unwrap();
        use schema::bpp_groups_permissions::dsl::*;
        let db_permission = models::GroupPermission {
            group_id: revoked_permission.group_id,
            permission: revoked_permission.permission,
            granted: false
        };
        diesel::insert_into(bpp_groups_permissions)
            .values(&db_permission)
            .execute(&conn)
            .unwrap();
        return Ok(tonic::Response::new(()));
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    setup_log(env::var_os("DEBUG").is_some());
    debug!("Debug mode activated!");

    info!("Loading settings...");
    let _ = Settings::new()?;

    let pool = connect_to_database();

    let youtube_address = env::var("YTS_GRPC_ADDRESS").expect("YTS_GRPC_ADDRESS must be set");
    let userservice_address = env::var("US_GRPC_ADDRESS");
    let userservice_address: SocketAddr = if userservice_address.is_err() {
        "0.0.0.0:50051".parse()?
    } else {
        userservice_address.unwrap().parse()?
    };

    let mut youtube_client = YouTubeServiceClient::connect(youtube_address).await?;
    info!("Connected to youtubeservice! Time to go on a hunt!");

    let service = UserServer {
        database_pool: pool.clone()
    };

    info!("Starting message fetching and userservice");
    let (_, _) = tokio::join!(
        fetch_users_from_messages(&mut youtube_client, &pool),
        tonic::transport::Server::builder()
            .add_service(UserServiceServer::new(service))
            .serve(userservice_address)
    );

    return Ok(());
}
