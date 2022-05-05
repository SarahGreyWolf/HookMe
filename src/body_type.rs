use serde::{Serialize, Deserialize};

pub trait Embed {
    fn get_username(&self) -> String;
    fn get_avatar_url(&self) -> String;
    fn get_first_embed(&self) -> EmbedData;
    fn get_embeds(&self) -> Vec<EmbedData>;
}

#[derive(Debug, Clone)]
pub struct Destination {
    pub(crate) username: String,
    pub(crate) avatar_url: String,
    pub(crate) server_id: u64,
    pub(crate) channel_id: u64,
    pub(crate) user_id: u64,
    pub(crate) app_id: u64
}

impl Destination {
    pub fn new(username: &str, avatar_url: &str, server_id: u64, channel_id: u64, user_id: u64, app_id: u64) -> Destination {
        Destination {
            username: username.into(),
            avatar_url: avatar_url.into(),
            server_id,
            channel_id,
            user_id,
            app_id
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EmbedData {
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) url: String,
    pub(crate) color: u32,
    pub(crate) footer: EmbedFooter,
    pub(crate) author: EmbedAuthor,
    pub(crate) fields: Option<Vec<EmbedField>>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EmbedFooter {
    pub(crate) text: String
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EmbedAuthor {
    pub(crate) name: String,
    pub(crate) url: String,
    pub(crate) icon_url: String
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EmbedField {
    pub(crate) name: String,
    pub(crate) value: String,
    pub(crate) inline: Option<bool>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DiscordWebhook {
    wait: bool,
    content: String,
    username: String,
    avatar_url: String,
    tts: bool,
    embeds: Vec<EmbedData>
}

impl Embed for DiscordWebhook {
    fn get_username(&self) -> String {
        self.username.clone()
    }

    fn get_avatar_url(&self) -> String {
        self.avatar_url.clone()
    }

    fn get_first_embed(&self) -> EmbedData {
        self.embeds[0].clone()
    }

    fn get_embeds(&self) -> Vec<EmbedData> {
        self.embeds.clone()
    }
}