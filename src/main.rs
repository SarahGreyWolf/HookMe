use axum::{routing::post, Router, extract::{Path, Query, Extension}, Json, http::StatusCode};
use body_type::{EmbedData, Destination, Embed};
use serde::Deserialize;
use tower::ServiceBuilder;
use tokio::sync::{RwLock};
use tokio::sync::mpsc::{Sender, channel};
use std::{sync::Arc, error::Error};
use std::{net::SocketAddr};
use serenity::model::{user::User};
use serenity::prelude::*;
use serenity::Client as DS_Client;
use serenity::framework::standard::{StandardFramework};
use mongodb::{Client, bson::{Document, doc}, bson::oid::ObjectId, options::ClientOptions, bson::Bson};

use discord::{Handler, ADMIN_GROUP};


mod body_type;
mod discord;

type SendEmbed = Arc<RwLock<Sender<(Destination, EmbedData)>>>;


#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv::dotenv().expect("Failed to load .env file");
    let (sender, receiver) = channel::<(Destination, EmbedData)>(2048);

    let mongo_username = std::env::var("MONGO_USERNAME").expect("Could not get mongo username in environment");
    let mongo_password = url_encode(&std::env::var("MONGO_PASSWORD").expect("Could not get mongo password in environment")).await;
    let mongo_address = std::env::var("MONGO_ADDR").expect("Could not get mongo address in environment");
    let mongo_database = std::env::var("MONGO_DB").expect("Could not get mongo db in environment");
    let connection_string = format!("mongodb://{mongo_username}:{mongo_password}@{mongo_address}/{mongo_database}");
    let options = ClientOptions::parse(connection_string).await?;

    let client = Client::with_options(options)?;
    let client_clone = client.clone();

    // Run Discord Bot
    let discord_task = tokio::spawn(async move {
        let prefix = std::env::var("BOT_PREFIX").unwrap_or("`".into());
        let handler = Handler::new(prefix.chars().next().unwrap(), receiver, client_clone);
        let framework = StandardFramework::new()
            .configure(|c| c.prefix(prefix))
            .group(&ADMIN_GROUP);
        let token = std::env::var("DISCORD_TOKEN").expect("Could not find Discord Token in environment");
        let intents = GatewayIntents::GUILD_MESSAGES
            | GatewayIntents::DIRECT_MESSAGES
            | GatewayIntents::MESSAGE_CONTENT;
        let mut client = DS_Client::builder(token, intents)
            .event_handler(handler)
            .framework(framework)
            .await.expect("Error creating client");
        if let Err(e) = client.start().await {
            println!("An error occurred while running the client: {:?}", e);
        }
    });

    let app = Router::new()
        .route("/:app_id/discord", post(hook_discord))
        .layer(
            ServiceBuilder::new()
                .layer(Extension(Arc::new(RwLock::new(sender))))
                .layer(Extension(client))
                .into_inner(),
        );
    let addr = SocketAddr::from(([192,168,0,14], 80));
    println!("Listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

#[derive(Deserialize)]
struct HookQuery{
    pub(crate) token: String
}

#[derive(Deserialize, Debug)]
struct Collection {
    _id: ObjectId,
    app_id: u64,
    app_name: String,
    token: String,
    owner: Document,
    server_id: u64,
    channel_id: u64,
    approved: Bson
}

// Webhook handling routes
/// Discord webhook handling route
async fn hook_discord(
    Json(body): Json<body_type::DiscordWebhook>,
    Path(app_id): Path<i64>,
    Query(query): Query<HookQuery>,
    state: Extension<SendEmbed>,
    mongo: Extension<Client>
) -> StatusCode {
    // println!("{:?}", body);
    // println!("App ID: {}", app_id);
    // println!("Token: {}", query.token);
    let db = mongo.default_database().expect("Failed to get default database");
    let collection = db.collection::<Collection>("application");
    if let Ok(found) = collection.find_one(
        doc!{"token": query.token, "app_id": app_id, "accepted": Bson::Boolean(true)}, None).await {
        if let Some(coll) = found {
            let destination = Destination::new(
                &body.get_username(),
                &body.get_avatar_url(),
                coll.server_id,
                coll.channel_id,
                121691909688131587,
                coll.app_id
            );
            let lock = state.write().await;
            lock.send((destination, body.get_first_embed())).await.expect("Failed to send embed");
            drop(lock);
            return StatusCode::ACCEPTED;
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