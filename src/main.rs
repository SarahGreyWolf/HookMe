use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use bcrypt::{hash, verify, DEFAULT_COST};
use body_type::{Destination, Embed, EmbedData};
use mongodb::{
    bson::doc, bson::oid::ObjectId, bson::Bson, options::ClientOptions, Client, Database,
};
use serde::{Deserialize, Serialize};
use serenity::framework::standard::StandardFramework;
use serenity::model::user::User;
use serenity::prelude::*;
use serenity::Client as DS_Client;
use std::{error::Error, sync::Arc};
use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
};
use tokio::sync::mpsc::{channel, Sender};
use tokio::sync::RwLock;
use tower::ServiceBuilder;

use discord::Handler;

mod body_type;
mod discord;

type SendEmbed = Arc<RwLock<Sender<(Destination, EmbedData)>>>;

#[derive(Serialize, Deserialize, Debug)]
pub struct UserCollection {
    _id: ObjectId,
    id: u64,
    username: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserRef {
    // #[serde(rename(deserialize="$ref"))]
    reference: String,
    // #[serde(rename(deserialize="$id"))]
    id: ObjectId,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppCollection {
    _id: ObjectId,
    app_id: u64,
    app_name: String,
    token: String,
    owner: UserRef,
    server_id: u64,
    channel_id: u64,
    approved: Bson,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv::dotenv().expect("Failed to load .env file");
    let (sender, receiver) = channel::<(Destination, EmbedData)>(2048);
    let mongo_username =
        std::env::var("MONGO_USERNAME").expect("Could not get mongo username in environment");
    let mongo_password = url_encode(
        &std::env::var("MONGO_PASSWORD").expect("Could not get mongo password in environment"),
    )
    .await;
    let mongo_address =
        std::env::var("MONGO_ADDR").expect("Could not get mongo address in environment");
    let mongo_database = std::env::var("MONGO_DB").expect("Could not get mongo db in environment");
    let connection_string =
        format!("mongodb://{mongo_username}:{mongo_password}@{mongo_address}/{mongo_database}");
    let options = ClientOptions::parse(connection_string).await?;

    let client = Client::with_options(options)?;
    let db = client.default_database().expect("Failed to get default database");
    let db_clone = db.clone();

    // Run Discord Bot
    let discord_task = tokio::spawn(async move {
        let prefix = std::env::var("BOT_PREFIX").unwrap_or("`".into());
        let handler = Handler::new(prefix.chars().next().unwrap(), receiver, db_clone);
        let framework = StandardFramework::new().configure(|c| c.prefix(prefix));
        let token =
            std::env::var("DISCORD_TOKEN").expect("Could not find Discord Token in environment");
        let intents = GatewayIntents::GUILD_MESSAGES |
            GatewayIntents::DIRECT_MESSAGES |
            GatewayIntents::MESSAGE_CONTENT;
        let mut client = DS_Client::builder(token, intents)
            .event_handler(handler)
            .framework(framework)
            .await
            .expect("Error creating client");

        if let Err(e) = client.start().await {
            println!("An error occurred while running the client: {:?}", e);
        }
    });

    let app = Router::new().route("/:app_id/discord", post(hook_discord)).layer(
        ServiceBuilder::new()
            .layer(Extension(Arc::new(RwLock::new(sender))))
            .layer(Extension(db))
            .into_inner(),
    );
    let address = std::env::var("LOCAL_IP").expect("Could not find local ip in environment");
    let port = std::env::var("PORT").expect("Could not find port in environment");
    let addr = SocketAddr::from((IpAddr::from_str(&address).unwrap(), port.parse().unwrap()));
    println!("Listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
    Ok(())
}

#[derive(Deserialize)]
struct HookQuery {
    pub(crate) token: String,
}

// Webhook handling routes
/// Discord webhook handling route
async fn hook_discord(
    Json(body): Json<body_type::DiscordWebhook>,
    Path(app_id): Path<i64>,
    // The token has to come in through the URI in one way or another sadly
    // This is INCREDIBLY unsafe and unsecure, but I can't enforce services
    // to provide it through more secure means since the intention of the bot
    // is to work with already existing webhook services just with a custom URI
    Query(query): Query<HookQuery>,
    state: Extension<SendEmbed>,
    db: Extension<Database>,
) -> StatusCode {
    // println!("{:?}", body);
    // println!("App ID: {}", app_id);
    // println!("Token: {}", &query.token);
    let collection = db.collection::<AppCollection>("application");
    if let Ok(found) = collection
        .find_one(
            doc! {"app_id": app_id, "approved": Bson::Boolean(true)},
            None,
        )
        .await
    {
        if let Some(coll) = found {
            if verify(&query.token, &coll.token).is_ok() {
                let destination = Destination::new(
                    &body.get_username(),
                    &body.get_avatar_url(),
                    coll.server_id,
                    coll.channel_id,
                    121691909688131587,
                    coll.app_id,
                );
                let lock = state.write().await;
                lock.send((destination, body.get_first_embed()))
                    .await
                    .expect("Failed to send embed");
                drop(lock);
                return StatusCode::ACCEPTED;
            } else {
                return StatusCode::UNAUTHORIZED;
            }
        }
    }
    return StatusCode::UNAUTHORIZED;
}

async fn url_encode(input: &str) -> String {
    input
        .replace("%", "%25")
        .replace(" ", "%20")
        .replace("\"", "%22")
        .replace("#", "%23")
        .replace("$", "%24")
        .replace("+", "%2b")
        .replace(",", "%2c")
        .replace("/", "%2f")
        .replace(":", "%3a")
        .replace(";", "%3b")
        .replace("<", "%3c")
        .replace("=", "%3d")
        .replace(">", "%3e")
        .replace("?", "%3f")
        .replace("@", "%40")
        .replace("[", "%5b")
        .replace("\\", "%5c")
        .replace("]", "%5d")
        .replace("^", "%5e")
        .replace("`", "%60")
        .replace("{", "%7b")
        .replace("|", "%7c")
        .replace("}", "%7d")
        .replace("~", "%7e")
}
