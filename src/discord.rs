use std::sync::Arc;
use tokio::sync::{RwLock};
use tokio::sync::mpsc::{Receiver, Sender, channel};
use serenity::{prelude::*, model::{user::User}, builder::CreateMessage};
use serenity::client::bridge::gateway::ShardManager;
use serenity::model::{channel::Message, gateway::Ready, id::ChannelId};
use serenity::client::{EventHandler, Context};
use serenity::framework::standard::macros::{group};
use serenity::async_trait;
use crate::body_type::{Embed, EmbedData, Destination};

#[group]
struct Admin;


pub(crate) struct Handler {
    prefix: char,
    incoming_embed: Arc<RwLock<Receiver<(Destination, EmbedData)>>>,
}

impl Handler {
    pub fn new(prefix: char, receiver: Receiver<(Destination, EmbedData)>) -> Handler {
        Handler {
            prefix,
            incoming_embed: Arc::new(RwLock::new(receiver))
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        let content = &mut msg.content.chars();
        if content.next().unwrap_or(' ') != self.prefix || msg.is_own(&ctx) {
            return;
        }
        let content = content.collect::<String>();
        let mut content = content.split_whitespace();
        let command = content.next().expect("Failed to get command");
        match command {
            "generate" => {
                let mentions = msg.mentions;
                let user = if mentions.len() > 0 {
                    mentions[0].clone()
                } else {
                    msg.channel_id.say(&ctx.http, "Please provide a user").await.expect("Failed to send message");
                    return;
                };
                msg.channel_id.say(&ctx.http, format!("{}'s id is {}", user.name, user.id)).await.expect("Failed to send message");
            },
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
                    e.title(embed.title.clone())
                        .author(|a| a.name(&embed.author.name).icon_url(&embed.author.icon_url).url(&embed.author.url))
                        .description(&embed.description)
                        .url(&embed.url)
                        .fields({
                            let mut fields: Vec<(String, String, bool)> = vec![];
                            if let Some(e_fields) = &embed.fields {
                                for field in e_fields {
                                    fields.push((field.name.to_string(), field.value.to_string(), field.inline.unwrap_or(false)));
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
                        if thread.name() == format!("{} - {}", &dest.username, user.name) {
                            if let Ok(messages) = &mut ctx.http.get_messages(thread.id.0, "").await {
                                // Reverse the messages because they are listed from last to first
                                messages.reverse();
                                let mut messages = messages.iter();
                                // Skip the first one
                                messages.next().unwrap();
                                if let Some(id_message) = messages.next() {
                                    if id_message.content == format!("{:#}", &dest.app_id) {
                                        let mut message_clone = message.clone();
                                        thread.send_message(&ctx.http, |_m| &mut message_clone).await
                                            .expect("Failed to send embed");
                                        continue 'receive;
                                    }
                                }
                            }
                        }
                    }
                }
                // Create a thread for the applicaiton id if one doesn't exist
                if let Ok(channels) = &guild.channels(&ctx.http).await {
                    if let Some(channel) = channels.get(&ChannelId(dest.channel_id)) {
                        let start_message = channel.send_message(&ctx.http, |m|
                             m.content(format!("{} - {}", &dest.username, user.name))
                        ).await.expect("Failed to create ID Message");
                        let thread = channel.create_public_thread(&ctx.http, start_message.id, |thread| {
                            thread
                                .name(format!("{} - {}", &dest.username, user.name))
                        }).await.expect("Failed to create public thread");
                        thread.send_message(&ctx.http, |m|
                            m.content(format!("{:#}", &dest.app_id)
                       )).await.expect("Failed to create ID Message");
                        let mut message_clone = message.clone();
                        thread.send_message(&ctx.http, |_m| &mut message_clone).await
                            .expect("Failed to send embed");
                    }
                }
            }
        }
    }
}