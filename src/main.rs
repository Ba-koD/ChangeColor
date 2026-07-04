use std::path::PathBuf;
use std::sync::Arc;

use poise::serenity_prelude as serenity;

mod commands;
mod config;
mod discord;
mod error;
mod roles;
mod runtime;
mod storage;
mod util;

#[cfg(test)]
mod tests;

pub(crate) use error::{Error, user_error};

use crate::config::DEFAULT_DATA_PATH;
use crate::discord::resolve_application_id;
use crate::runtime::{clear_commands_from_arg, event_handler, spawn_grace_task};
use crate::storage::Storage;

pub(crate) type Context<'a> = poise::Context<'a, Data, Error>;

pub(crate) struct Data {
    pub(crate) storage: Arc<Storage>,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "change_color=info,serenity=warn,poise=warn".into()),
        )
        .init();

    let token = std::env::var("DISCORD_TOKEN")
        .map_err(|_| user_error("DISCORD_TOKEN env var is required"))?;
    let http = serenity::Http::new(&token);
    let application_id = resolve_application_id(&http).await?;

    if let Some(command_action) = std::env::args().nth(1) {
        clear_commands_from_arg(
            &token,
            application_id,
            &command_action,
            std::env::args().nth(2),
        )
        .await?;
        return Ok(());
    }

    let data_path = std::env::var("DATA_PATH").unwrap_or_else(|_| DEFAULT_DATA_PATH.to_string());
    let storage = Arc::new(Storage::load(PathBuf::from(data_path))?);

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: commands::bot_commands(),
            event_handler: |ctx, event, framework, data| {
                Box::pin(event_handler(ctx, event, framework, data))
            },
            on_error: |error| {
                Box::pin(async move {
                    if let Err(error) = poise::builtins::on_error(error).await {
                        tracing::error!(?error, "failed to handle poise error");
                    }
                })
            },
            ..Default::default()
        })
        .setup(move |ctx, ready, _framework| {
            let storage = storage.clone();
            Box::pin(async move {
                spawn_grace_task(ctx.clone(), storage.clone());
                println!("bot is ready: {}", ready.user.name);
                tracing::info!(user = %ready.user.name, "bot is ready");
                Ok(Data { storage })
            })
        })
        .build();

    // GUILD_MEMBERS is a privileged intent. Without it enabled in the Discord
    // Developer Portal the gateway rejects the connection, so it is omitted here.
    // Consequence: GuildMemberUpdate events are not delivered, disabling the
    // automatic role reconciliation feature (reconcile_member_roles).
    let intents = serenity::GatewayIntents::GUILDS;
    let mut client = serenity::ClientBuilder::new(token, intents)
        .application_id(serenity::ApplicationId::new(application_id))
        .framework(framework)
        .await?;

    client.start().await?;
    Ok(())
}
