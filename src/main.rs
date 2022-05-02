use axum::{response::Html, routing::{get, post}, Router, extract::{Path, Extension, RawBody}, body::StreamBody, Json};
use body_type::{Embed, EmbedData, Destination};
use tower::ServiceBuilder;
use tokio::spawn;
use tokio::sync::{RwLock};
use tokio::sync::mpsc::{Receiver, Sender, channel};
use std::sync::Arc;
use std::{net::SocketAddr, sync::atomic::Ordering};
use serenity::{framework::standard::macros::{group}, client::{EventHandler, Context}, model::{channel::Message, gateway::Ready, id::ChannelId}};
use serenity::async_trait;
use serenity::builder::{CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter};


mod body_type;

#[group]
struct Admin;

struct Handler {
    prefix: &'static str,
    incoming_embed: Arc<RwLock<Receiver<(Destination, EmbedData)>>>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        let mut content = msg.content.clone();
        if !content.starts_with(self.prefix) && msg.is_own(ctx) {
            return;
        }
        content.remove(0);
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        let mut receiver = self.incoming_embed.write().await;
        while let Some((dest, embed)) = receiver.recv().await {
            for guild in &ready.guilds {
                let guild = guild.id;
                if dest.server_id == guild.0 {
                    if let Ok(channels) = &guild.channels(&ctx.http).await {
                        if let Some(channel) = channels.get(&ChannelId(dest.channel_id)) {
                            channel.send_message(&ctx.http, |m| {
                                m.embed(|e| {
                                    e.title(embed.title.clone())
                                        .author(|a| a.name(embed.author.name.clone()).icon_url(embed.author.icon_url.clone()).url(embed.author.url.clone()))
                                        .description(embed.description.clone())
                                        .url(embed.url.clone())
                                        .fields({
                                            let mut fields: Vec<(String, String, bool)> = vec![];
                                            if let Some(e_fields) = embed.fields.clone() {
                                                for field in e_fields {
                                                    fields.push((field.name, field.value, field.inline.unwrap_or(false)));
                                                }
                                            }
                                            fields
                                        })
                                        .footer(|f| f.text(embed.footer.text.clone()))
                                })
                            }).await.expect("Faild to send embed");
                        }
                    }
                }
            }
        }
    }
}

impl Handler {
    pub fn new(prefix: &'static str, receiver: Receiver<(Destination, EmbedData)>) -> Handler {
        Handler {
            prefix,
            incoming_embed: Arc::new(RwLock::new(receiver))
        }
    }
}

type SendEmbed = Arc<RwLock<Sender<(Destination, EmbedData)>>>;

#[tokio::main]
async fn main() {
    let (sender, receiver) = channel::<(Destination, EmbedData)>(2048);
    let handler = Handler::new("`", receiver);

    // Run Discord Bot
    let discord_task = tokio::spawn(async move {
        use serenity::prelude::*;
        use serenity::framework::standard::{StandardFramework};
        let framework = StandardFramework::new()
            .configure(|c| c.prefix("`"))
            .group(&ADMIN_GROUP);
        let token = std::env::var("DISCORD_TOKEN").expect("Could not find Discord Token in environment");
        let intents = GatewayIntents::non_privileged();
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

// Webhook handling routes
/// Discord webhook handling route
async fn hook_discord(
    Json(body): Json<body_type::Discord>,
    Path((server_id, channel_id, user_id, app_id)): Path<(u64, u64, u64, u64)>,
    state: Extension<SendEmbed>,
) {
    // println!("{:?}", body);
    // println!("Server ID: {}", server_id);
    // println!("Channel ID: {}", channel_id);
    // println!("User ID: {}", user_id);
    // println!("App ID: {}", app_id);
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
// JWT is associated with the specific users discord name and id and the channel
async fn generate_jwt() {

}
