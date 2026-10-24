use std::sync::Arc;
use std::time::Duration;

use poise::serenity_prelude as serenity;

use crate::{Data, Error};

/// Red embed replying to `msg`, deleted after 10 seconds.
pub async fn send_error(http: &Arc<serenity::Http>, msg: &serenity::Message, title: &str, description: &str) {
    let embed = serenity::CreateEmbed::new().title(title).description(description).color(0xff0000);
    let builder = serenity::CreateMessage::new().embed(embed).reference_message(msg);
    match msg.channel_id.send_message(http, builder).await {
        Ok(sent) => {
            let http = http.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(10)).await;
                let _ = sent.delete(&http).await;
            });
        }
        Err(e) => tracing::warn!("error: could not send error embed in {}: {e}", msg.channel_id),
    }
}

/// True when a serenity error is Discord answering 403.
pub fn is_forbidden(err: &serenity::Error) -> bool {
    matches!(
        err,
        serenity::Error::Http(serenity::HttpError::UnsuccessfulRequest(resp)) if resp.status_code.as_u16() == 403
    )
}

pub async fn on_error(err: poise::FrameworkError<'_, Data, Error>) {
    match err {
        poise::FrameworkError::UnknownCommand { ctx, msg, msg_content, framework, .. } => {
            // "+" on its own or "+ something" isn't a command attempt
            if msg_content.trim().is_empty() || msg_content.starts_with(char::is_whitespace) {
                return;
            }
            let prefix = &framework.user_data.config.prefix;
            send_error(&ctx.http, msg, "Unknown command", &format!("Use `{prefix}help` to see available commands."))
                .await;
        }
        poise::FrameworkError::ArgumentParse { ctx, error, .. } => {
            if let poise::Context::Prefix(p) = ctx {
                send_error(&ctx.serenity_context().http, p.msg, "Invalid argument", &error.to_string()).await;
            }
        }
        poise::FrameworkError::MissingUserPermissions { ctx, .. } => {
            if let poise::Context::Prefix(p) = ctx {
                send_error(
                    &ctx.serenity_context().http,
                    p.msg,
                    "Permission denied",
                    "You don't have permission to use this command.",
                )
                .await;
            }
        }
        poise::FrameworkError::MissingBotPermissions { ctx, .. } => {
            if let poise::Context::Prefix(p) = ctx {
                send_error(
                    &ctx.serenity_context().http,
                    p.msg,
                    "Permission error",
                    "I don't have the necessary permissions to execute this command.",
                )
                .await;
            }
        }
        poise::FrameworkError::Command { ctx, error, .. } => {
            tracing::error!(
                "Unhandled command error in +{} by {}: {error}",
                ctx.command().name,
                ctx.author().name
            );
            if let poise::Context::Prefix(p) = ctx {
                send_error(
                    &ctx.serenity_context().http,
                    p.msg,
                    "Error",
                    "An unexpected error occurred while processing your command. Please try again later.",
                )
                .await;
            }
        }
        other => {
            if let Err(e) = poise::builtins::on_error(other).await {
                tracing::error!("error: while handling error: {e}");
            }
        }
    }
}
