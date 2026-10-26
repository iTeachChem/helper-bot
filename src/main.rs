mod commands;
mod config;
mod db;
mod error;
mod events;
mod health;
mod log;
mod render;

use std::sync::Arc;

use anyhow::Context as _;
use poise::serenity_prelude as serenity;

pub struct Data {
    pub config: Arc<config::Config>,
    pub db: db::Db,
    pub http: reqwest::Client,
    pub cooldowns: commands::Cooldowns,
}

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    log::init(http.clone());

    tokio::spawn(health::serve());

    let token = std::env::var("BOT_TOKEN").context("BOT_TOKEN is not set")?;
    let config = Arc::new(config::Config::load("config.toml")?);

    let db = db::Db::from_env(http.clone())?;
    db.init().await?;
    db.set_started_at().await?;
    tracing::info!("setup: db initialised and started_at recorded");

    let data = Data { config: config.clone(), db, http, cooldowns: Default::default() };

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: commands::all(),
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some(config.prefix.clone()),
                mention_as_prefix: false,
                case_insensitive_commands: false,
                ..Default::default()
            },
            on_error: |err| Box::pin(error::on_error(err)),
            event_handler: |ctx, event, _framework, data| Box::pin(events::handle(ctx, event, data)),
            ..Default::default()
        })
        .setup(|_ctx, ready, _framework| {
            Box::pin(async move {
                tracing::info!("bot: logged in as {} (id: {})", ready.user.name, ready.user.id);
                Ok(data)
            })
        })
        .build();

    let intents = serenity::GatewayIntents::non_privileged()
        | serenity::GatewayIntents::MESSAGE_CONTENT
        | serenity::GatewayIntents::GUILD_MEMBERS;

    // keep the cache small: no message cache
    let mut cache = serenity::cache::Settings::default();
    cache.max_messages = 0;

    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .cache_settings(cache)
        .await?;
    client.start().await?;
    Ok(())
}
