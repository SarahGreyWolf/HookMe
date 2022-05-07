use crate::body_type::{Destination, Embed, EmbedData};
use crate::{UserCollection, UserRef, AppCollection};
use mongodb::{
    bson::oid::ObjectId,
    bson::{doc, Document, Bson},
    options::ClientOptions,
    Database,
};
use serenity::{async_trait, model::{id::{RoleId, GuildId}, guild::{Guild, GuildContainer}}};
use serenity::client::{Context, EventHandler};
use serenity::model::{channel::Message, gateway::Ready, id::ChannelId};
use serenity::{builder::CreateMessage, model::user::User, prelude::*};
use std::error::Error;
use std::sync::Arc;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::RwLock;
use yyid::*;
use bcrypt::{DEFAULT_COST, hash, verify};

pub(crate) struct Handler {
    prefix: char,
    incoming_embed: Arc<RwLock<Receiver<(Destination, EmbedData)>>>,
    db: Database,
}

impl Handler {
    pub fn new(
        prefix: char,
        receiver: Receiver<(Destination, EmbedData)>,
        db: Database,
    ) -> Handler {
        Handler {
            prefix,
            incoming_embed: Arc::new(RwLock::new(receiver)),
            db
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.is_private() {
            return;
        }
        let content = &mut msg.content.chars();
        if content.next().unwrap_or(' ') != self.prefix || msg.is_own(&ctx) {
            return;
        }
        let content = content.collect::<String>();
        let mut content = content.split_whitespace();
        let command = content.next().expect("Failed to get command");
        let parameters: Vec<&str> = content.collect();
        match command {
            "request" => request(&self.prefix, &self.db, parameters, &ctx, &msg).await,
            "approve" => approve(&self.db, parameters, &ctx, &msg).await,
            "decline" => decline().await,
            "revoke" => revoke(&self.db, parameters, &ctx, &msg).await,
            "help" => help(&self.prefix, &ctx, &msg).await,
            _ => {}
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        let mut receiver = self.incoming_embed.write().await;
        'receive: while let Some((dest, embed)) = receiver.recv().await {
            for guild in &ready.guilds {
                let guild = guild.id;
                if dest.server_id != guild.0 {
                    continue;
                }
                let user = &ctx.http.get_user(dest.user_id).await.expect("Failed to get user");
                let mut message = CreateMessage::default();
                message.embed(|e| {
                    e.title(escape(&embed.title.clone()))
                        .author(|a| {
                            a.name(escape(&embed.author.name))
                                .icon_url(&embed.author.icon_url)
                                .url(&embed.author.url)
                        })
                        .description(escape(&embed.description))
                        .url(escape(&embed.url))
                        .fields({
                            let mut fields: Vec<(String, String, bool)> = vec![];
                            if let Some(e_fields) = &embed.fields {
                                for field in e_fields {
                                    fields.push((
                                        escape(&field.name),
                                        escape(&field.value),
                                        field.inline.unwrap_or(false),
                                    ));
                                }
                            }
                            fields
                        })
                        .footer(|f| f.text(&embed.footer.text))
                });
                // Check to see if a thread already exists for this application id
                if let Ok(threadsdata) = &ctx.http.get_guild_active_threads(guild.0).await {
                    let threads = &threadsdata.threads;
                    for thread in threads {
                        if thread.name() != format!("{} - {}", &dest.username, user.name) {
                            continue;
                        }
                        if let Ok(messages) = &mut ctx.http.get_messages(thread.id.0, "").await {
                            // Reverse the messages because they are listed from last to first
                            messages.reverse();
                            let mut messages = messages.iter();
                            // Skip the first one
                            messages.next().unwrap();
                            if let Some(id_message) = messages.next() {
                                if id_message.content == format!("{:#}", &dest.app_id) {
                                    let mut message_clone = message.clone();
                                    thread
                                        .send_message(&ctx.http, |_m| &mut message_clone)
                                        .await
                                        .expect("Failed to send embed");
                                    continue 'receive;
                                }
                            }
                        }
                    }
                }
                // Create a thread for the applicaiton id if one doesn't exist
                if let Ok(channels) = &guild.channels(&ctx.http).await {
                    if let Some(channel) = channels.get(&ChannelId(dest.channel_id)) {
                        let start_message = channel
                            .send_message(&ctx.http, |m| {
                                m.content(format!("{} - {}", &dest.username, user.name))
                            })
                            .await
                            .expect("Failed to create ID Message");
                        let thread = channel
                            .create_public_thread(&ctx.http, start_message.id, |thread| {
                                thread.name(format!("{} - {}", &dest.username, user.name))
                            })
                            .await
                            .expect("Failed to create public thread");
                        thread
                            .send_message(&ctx.http, |m| m.content(format!("{:#}", &dest.app_id)))
                            .await
                            .expect("Failed to create ID Message");
                        let mut message_clone = message.clone();
                        thread
                            .send_message(&ctx.http, |_m| &mut message_clone)
                            .await
                            .expect("Failed to send embed");
                    }
                }
            }
        }
    }
}

/// Request an app_id and token to use a webhook
async fn request(prefix: &char, db: &Database, parameters: Vec<&str>, ctx: &Context, msg: &Message) {
    let user = &msg.author;
    let guild_id = &msg.guild_id.expect("Failed to get guild id");
    if !has_permission("GENERAL_ROLE_ID", &ctx, &msg, &user, guild_id.0).await {
        return;
    }
    if parameters.len() != 1 {
        msg.channel_id
            .say(&ctx.http, "Please provide an app name (one word)")
            .await
            .expect("Failed to send message");
        return;
    }
    if let Some(app) = parameters.get(0) {
        let app_id: u32 = rand::random();
        insert_new_app(db, user, app_id, app, guild_id.0, msg.channel_id.0).await;
        user.direct_message(&ctx.http, |m| {
            m.content(format!("Request Submitted for {}", app))
        }).await.expect("Failed to tell user about submitted request");
        let mut destination = msg.channel_id;
        if let Ok(id) = std::env::var("APPROVAL_CHANNEL_ID") {
            if let Ok(channel) = &ctx.http.get_channel(id.parse().unwrap()).await {
                destination = channel.id();
            }
        }
        destination
            .say(
                &ctx.http,
                format!(
                    "{} is requesting hook privileges for app {}",
                    user.mention(),
                    app
                ),
            )
            .await
            .expect("Failed to send message");
        destination
            .say(
                &ctx.http,
                format!("Admins can approve it with `{}approve {}`", prefix, app_id),
            )
            .await
            .expect("Failed to send message");
    }
}

/// Approve a request
async fn approve(db: &Database, parameters: Vec<&str>, ctx: &Context, msg: &Message) {
    let user = &msg.author;
    let guild_id = &msg.guild_id.expect("Failed to get guild id");
    if !has_permission("ADMIN_ROLE_ID", &ctx, &msg, &user, guild_id.0).await {
        return;
    }
    if parameters.len() != 1 {
        msg.channel_id
            .say(&ctx.http, "Please provide only an app id")
            .await
            .expect("Failed to send message");
        return;
    }
    if let Some(app_id) = parameters.get(0) {
        let (user, app) = get_app_and_user(db, app_id.parse().unwrap()).await.unwrap();
        if app.approved == Bson::Boolean(true) {
            return;
        }
        ctx.http.broadcast_typing(msg.channel_id.0).await.expect("Failed to start typing");
        let token = Yyid::new();
        let hashed_token = hash(token.as_bytes(), DEFAULT_COST).expect("FAILED TO HASH TOKEN");
        ctx.http.broadcast_typing(msg.channel_id.0).await.expect("Failed to start typing");
        if verify(token.as_bytes(), &hashed_token).is_err() {
            panic!("Somehow hashed token was not verified for token");
        }
        let app_coll = db.collection::<AppCollection>("application");
        let token = token.to_string();
        let app_id_long = app_id.parse::<u32>().unwrap();
        app_coll.update_one(doc!{"app_id": app_id_long}, doc!{"$set":{"token": hashed_token, "approved": Bson::Boolean(true)}}, None)
            .await.expect("Failed to update app");
        let address = std::env::var("HOOK_ADDRESS").unwrap_or("http://localhost".into());
        if let Ok(end_user) = &ctx.http.get_user(user.id).await {
            end_user.direct_message(&ctx.http, |m| {
                m.content(format!("The address for your apps webhook is {address}/{app_id}/discord?token={token}"))
            }).await.expect("Failed to DM user");
        } else {
            panic!("Failed to get owner for app {}", app_id);
        }
        msg.channel_id
            .say(&ctx.http, "Approval Complete")
            .await
            .expect("Failed to send message");
    }
}

/// Decline a request
async fn decline() {}

/// Revoke an app_id and token
async fn revoke(db: &Database, parameters: Vec<&str>, ctx: &Context, msg: &Message) {
    let user = &msg.author;
    let guild_id = &msg.guild_id.expect("Failed to get guild id");
    if !has_permission("ADMIN_ROLE_ID", &ctx, &msg, &user, guild_id.0).await {
        return;
    }
    if parameters.len() != 1 {
        msg.channel_id
            .say(&ctx.http, "Please provide only an app id")
            .await
            .expect("Failed to send message");
        return;
    }
    if let Some(app_id) = parameters.get(0) {
        
    }
}

async fn help(prefix: &char, ctx: &Context, msg: &Message) {
    let user = &msg.author;
    let bot_user = &ctx.http.get_current_user().await
                .expect("Failed to get bot user");
    user.direct_message(&ctx.http, |m|{
        m.add_embed(|e|{
            e.title("Hook Me Commands:")
                .author(|a| a.name(&bot_user.name).icon_url(&bot_user.avatar_url().unwrap()))
                .fields(vec![
                    (format!("{prefix}request <app name>"), "Request webhook access for an app", false),
                    (format!("{prefix}approve <app id>"), "Approve the request for webhook access", false)
                ])
        })
    }).await.unwrap();
}

fn escape(input: &str) -> String {
    input.replace("@", "\\@")
}

async fn has_permission(key: &str, ctx: &Context, msg: &Message, user: &User, guild: u64) -> bool {
    if let Ok(role_id) = std::env::var(key) {
        if role_id != "" {
            if !user.has_role(&ctx.http, guild, RoleId(role_id.parse().unwrap())).await.unwrap_or(false) {
                msg.channel_id
                    .say(&ctx.http, "You do not have permission to use this command")
                    .await
                    .expect("Failed to send message");
                return false;
            }
        }
    }
    true
}

async fn get_app_and_user(db: &Database, app_id: u64) -> Result<(UserCollection, AppCollection), Box<dyn Error>> {
    let app_coll = db.collection::<AppCollection>("application");
    let user_coll = db.collection::<UserCollection>("user");
    if let Some(app) = app_coll.find_one(doc!{"app_id": app_id as i64}, None).await? {
        let user_id = app.owner.id;
        if let Some(user) = user_coll.find_one(doc!{"_id": user_id}, None).await? {
            return Ok((user, app));
        } else {
            return Err(Box::<dyn Error + Send + Sync>::from("Failed to find user"));
        }
    } else {
        return Err(Box::<dyn Error + Send + Sync>::from("Failed to find application"));
    }
}

async fn insert_new_app(db: &Database, user: &User, app_id: u32, app_name: &str, guild_id: u64, channel_id: u64) {
    let username = &*user.name;
    let user_collection = UserCollection {
        _id: ObjectId::new(),
        id: user.id.0,
        username: username.into(),
    };
    let user_coll = db.collection::<UserCollection>("user");
    let id = match user_coll.find_one(doc!{"id": user.id.0 as i64}, None).await {
        Ok(user) => {
            if let Some(user) = user {
                user._id
            } else {
                let id = user_coll.insert_one(user_collection, None).await.expect("Failed to write user to collection");
                id.inserted_id.as_object_id().unwrap()
            }
        },
        Err(_) => {
            let id = user_coll.insert_one(user_collection, None).await.expect("Failed to write user to collection");
            id.inserted_id.as_object_id().unwrap()
        },
    };
    let app_collection = AppCollection {
        _id: ObjectId::new(),
        app_id: app_id as u64,
        app_name: app_name.into(),
        token: "".into(),
        owner: crate::UserRef {
            reference: "user".into(),
            id,
        },
        server_id: guild_id,
        channel_id: channel_id,
        approved: Bson::Boolean(false),
    };
    let app_coll = db.collection::<AppCollection>("application");
    app_coll.insert_one(app_collection, None).await.expect("Failed to write app to collection");
}