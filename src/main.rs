use axum::{response::Html, routing::{get, post}, Router, extract::{Path, Query, Extension, RawBody}, body::StreamBody, Json};
use body_type::{Embed, EmbedData, Destination};
use serde::Deserialize;
use tower::ServiceBuilder;
use tokio::sync::{RwLock};
use tokio::sync::mpsc::{Receiver, Sender, channel};
use std::sync::Arc;
use std::{net::SocketAddr};
use serenity::model::{user::User};
use serenity::async_trait;

use discord::{Handler, ADMIN_GROUP};


mod body_type;
mod discord;

type SendEmbed = Arc<RwLock<Sender<(Destination, EmbedData)>>>;

#[tokio::main]
async fn main() {
    dotenv::dotenv().expect("Failed to load .env file");
    let (sender, receiver) = channel::<(Destination, EmbedData)>(2048);

    // Run Discord Bot
    let discord_task = tokio::spawn(async move {
        use serenity::prelude::*;
        use serenity::framework::standard::{StandardFramework};
        let prefix = std::env::var("BOT_PREFIX").unwrap_or("`".into());
        let handler = Handler::new(prefix.chars().next().unwrap(), receiver);
        let framework = StandardFramework::new()
            .configure(|c| c.prefix("`"))
            .group(&ADMIN_GROUP);
        let token = std::env::var("DISCORD_TOKEN").expect("Could not find Discord Token in environment");
        let intents = GatewayIntents::GUILD_MESSAGES
            | GatewayIntents::DIRECT_MESSAGES
            | GatewayIntents::MESSAGE_CONTENT;
        let mut client = Client::builder(token, intents)
            .event_handler(handler)
            .framework(framework)
            .await
            .expect("Error creating client");
        if let Err(e) = client.start().await {
            println!("An error occurred while running the client: {:?}", e);
        }
    });

    let app = Router::new()
        .route("/:server_id/:channel_id/:user_id/:app_id/discord", post(hook_discord))
        .layer(
            ServiceBuilder::new()
                .layer(Extension(Arc::new(RwLock::new(sender))))
                .into_inner(),
        );
    let addr = SocketAddr::from(([192,168,0,14], 80));
    println!("Listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[derive(Deserialize)]
struct HookQuery{
    pub(crate) token: String
}

// Webhook handling routes
/// Discord webhook handling route
async fn hook_discord(
    Json(body): Json<body_type::DiscordWebhook>,
    Path((server_id, channel_id, user_id, app_id)): Path<(u64, u64, u64, u64)>,
    Query(query): Query<HookQuery>,
    state: Extension<SendEmbed>,
) {
    // println!("{:?}", body);
    // println!("Server ID: {}", server_id);
    // println!("Channel ID: {}", channel_id);
    // println!("User ID: {}", user_id);
    // println!("App ID: {}", app_id);
    // println!("Token: {}", query.token);

    let destination = Destination::new(&body.get_username(), &body.get_avatar_url(), server_id, channel_id, user_id, app_id);
    let lock = state.write().await;
    lock.send((destination, body.get_first_embed())).await.expect("Failed to send embed");
    drop(lock);
}

// Server/Guild dashboard for managing webhook settings
// Only provides access to the server that the JWT was generated for
async fn dashboard() {

}

// Post for form
// Takes the users id e.g SarahGreyWolf#8257 and the JWT from the discord command
async fn login() {
}

// Probably not from the web, should be called via bot command
// Takes the users id and the server they are calling from
// Generates a single use password/JWT using their id, the server/guild id and the current unix epoch
// JWT is associated with the specific users discord id and the channel
async fn generate_jwt(user: &User) -> String {

    "Out".into()
}
