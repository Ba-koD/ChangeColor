use std::sync::Arc;
use std::time::Duration;

use poise::serenity_prelude as serenity;

use crate::config::{CLEAR_COMMANDS, CLEAR_GLOBAL_COMMANDS, CLEAR_GUILD_COMMANDS};
use crate::roles::process_grace_expirations;
use crate::storage::Storage;
use crate::{Data, Error, user_error};

pub(crate) async fn clear_commands_from_arg(
    token: &str,
    application_id: u64,
    command_action: &str,
    guild_id_arg: Option<String>,
) -> Result<(), Error> {
    let client = reqwest::Client::new();
    let empty_commands: Vec<serde_json::Value> = Vec::new();

    match command_action {
        CLEAR_COMMANDS => {
            let url = global_commands_url(application_id);
            put_commands(
                &client,
                token,
                &url,
                &empty_commands,
                "clear global commands",
            )
            .await?;
            println!("cleared global commands");
        }
        CLEAR_GLOBAL_COMMANDS => {
            let url = global_commands_url(application_id);
            put_commands(
                &client,
                token,
                &url,
                &empty_commands,
                "clear global commands",
            )
            .await?;
            println!("cleared global commands");
        }
        CLEAR_GUILD_COMMANDS => {
            let guild_id = parse_guild_id_arg(guild_id_arg.as_deref())?;
            let url = guild_commands_url(application_id, guild_id);
            put_commands(
                &client,
                token,
                &url,
                &empty_commands,
                "clear guild commands",
            )
            .await?;
            println!("cleared guild commands in guild {}", guild_id.get());
        }
        _ => {
            return Err(user_error(format!(
                "unknown command `{command_action}`. Use `{CLEAR_COMMANDS}`, `{CLEAR_GLOBAL_COMMANDS}`, or `{CLEAR_GUILD_COMMANDS} <guild_id>`"
            )));
        }
    }

    Ok(())
}

fn command_builders(commands: &[poise::Command<Data, Error>]) -> Vec<serenity::CreateCommand> {
    let mut builders = Vec::new();
    for command in commands {
        let Some(builder) = command.create_as_slash_command() else {
            continue;
        };
        builders.push(builder);
    }
    builders
}

fn parse_guild_id_arg(guild_id_arg: Option<&str>) -> Result<serenity::GuildId, Error> {
    let guild_id = guild_id_arg
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| user_error(format!("`{CLEAR_GUILD_COMMANDS}` requires <guild_id>")))?;
    Ok(serenity::GuildId::new(guild_id.parse::<u64>()?))
}

async fn put_commands(
    client: &reqwest::Client,
    token: &str,
    url: &str,
    body: &[serde_json::Value],
    action: &str,
) -> Result<(), Error> {
    let response = client
        .put(url)
        .header("Authorization", format!("Bot {token}"))
        .json(body)
        .send()
        .await?;
    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_else(|_| "".to_string());
        return Err(user_error(format!("{action} failed: HTTP {status} {text}")));
    }

    Ok(())
}

fn global_commands_url(application_id: u64) -> String {
    format!("https://discord.com/api/v10/applications/{application_id}/commands")
}

fn guild_commands_url(application_id: u64, guild_id: serenity::GuildId) -> String {
    format!(
        "https://discord.com/api/v10/applications/{application_id}/guilds/{}/commands",
        guild_id.get()
    )
}

pub(crate) async fn event_handler(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    framework: poise::FrameworkContext<'_, Data, Error>,
    data: &Data,
) -> Result<(), Error> {
    match event {
        serenity::FullEvent::GuildCreate { guild, .. } => {
            if let Err(error) =
                sync_guild_application_commands(ctx, guild.id, &framework.options.commands).await
            {
                tracing::warn!(
                    ?error,
                    guild_id = guild.id.get(),
                    "failed to sync guild commands"
                );
            }
        }
        // GuildMemberUpdate handling is disabled: it requires the privileged
        // GUILD_MEMBERS gateway intent (see main.rs). Re-enable the intent and
        // this arm together to restore automatic role reconciliation.
        _ => {}
    }
    Ok(())
}

async fn sync_guild_application_commands(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
    commands: &[poise::Command<Data, Error>],
) -> Result<(), Error> {
    guild_id
        .set_commands(&ctx.http, command_builders(commands))
        .await?;
    println!("synced guild commands in guild {}", guild_id.get());
    Ok(())
}

pub(crate) fn spawn_grace_task(ctx: serenity::Context, storage: Arc<Storage>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60 * 60));
        loop {
            interval.tick().await;
            if let Err(error) = process_grace_expirations(&ctx, &storage).await {
                tracing::warn!(?error, "failed to process color grace expirations");
            }
        }
    });
}
