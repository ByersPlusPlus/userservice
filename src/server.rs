#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

use std::env;
use std::net::SocketAddr;

use chrono::Utc;
use diesel::prelude::*;
use diesel::PgConnection;
use diesel::r2d2::ConnectionManager;
use models::User;
use r2d2::Pool;
use tonic::transport::Channel;
use tonic::{transport::Server, Request, Response, Status};
use ::log::{debug, error, info};
use dotenv::dotenv;
use diesel_migrations::embed_migrations;

use userservice::{BppUser, BppUserById, BppUserFilter, BppUserFilters, BppUsers};
use userservice::user_service_server::{UserService, UserServiceServer};
use youtubeservice::{YouTubeChatMessage, YouTubeChatMessages, GetMessageRequest};
use youtubeservice::you_tube_service_client::YouTubeServiceClient;

use crate::log::setup_log;

mod log;
mod schema;
mod macros;
mod models;

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

async fn fetch_users_from_messages(youtube_client: &mut YouTubeServiceClient<Channel>, pool: &DbPool) -> Void {
    let mut stream = youtube_client.subscribe_messages(Request::new(())).await?.into_inner();

    while let Some(message) = stream.message().await? {
        let conn = pool.get()?;
        let user = if User::check_if_exists(&message.channel_id, &conn) {
            // Update the user
            User::get_from_database(&message.channel_id, &conn).unwrap()
        } else {
            let now = Utc::now().naive_utc();
            // Create the user
            User::new(
                message.channel_id.clone(),
                message.display_name.clone(),
                0,
                0,
                0,
                now,
                now
            )
        };

        // Determine if user was active before this message and if so, update the hours
        
    }
    
    return Ok(());
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    setup_log(env::var_os("DEBUG").is_some());
    debug!("Debug mode activated!");

    let pool = connect_to_database();

    let youtube_address: SocketAddr = env::var("YTS_GRPC_ADDRESS").expect("YTS_GRPC_ADDRESS must be set").parse()?;
    let userservice_address = env::var("US_GRPC_ADDRESS");
    let userservice_address: SocketAddr = if userservice_address.is_err() {
        "0.0.0.0:50051".parse()?
    } else {
        userservice_address.unwrap().parse()?
    };

    let mut youtube_client = YouTubeServiceClient::connect("http://localhost:50051").await?;

    let _ = tokio::join!(
        fetch_users_from_messages(&mut youtube_client, &pool),
    );

    return Ok(());
}
