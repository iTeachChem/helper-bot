//! Gateway events: forum thread greeting and the honeypot channel.

use std::time::Duration;

use poise::serenity_prelude as serenity;

use crate::error::is_forbidden;
use crate::{Data, Error};

const UNBAN_DELAY: Duration = Duration::from_secs(3);

pub async fn handle(ctx: &serenity::Context, event: &serenity::FullEvent, data: &Data) -> Result<(), Error> {
    match event {
        serenity::FullEvent::ThreadCreate { thread } => on_thread_create(ctx, thread, data).await,
        serenity::FullEvent::Message { new_message } => honeypot(ctx, new_message, data).await,
        _ => {}
    }
    Ok(())
}

async fn on_thread_create(ctx: &serenity::Context, thread: &serenity::GuildChannel, data: &Data) {
    let fc = &data.config.forum;
    if thread.guild_id.get() != fc.server_id || thread.parent_id.map(|p| p.get()) != Some(fc.channel_id) {
        return;
    }
    // discord sometimes rejects messages sent the instant a forum post is created
    tokio::time::sleep(Duration::from_secs(1)).await;

    let mut roles: Vec<u64> = Vec::new();
    for tag in &thread.applied_tags {
        if let Some(&role) = fc.tag_roles.get(&tag.get()) {
            if !roles.contains(&role) {
                roles.push(role);
            }
        }
    }

    let embed = serenity::CreateEmbed::new()
        .description(
            "**Note for OP**\n\
             `+solved @user1 @user2...` to close the thread when your doubt is solved. \
             Mention the users who helped you solve the doubt. \
             This will be added to their stats.",
        )
        .color(0x5865F2);
    let mut msg = serenity::CreateMessage::new().embed(embed);
    if !roles.is_empty() {
        let mentions: Vec<String> = roles.iter().map(|r| format!("<@&{r}>")).collect();
        msg = msg.content(mentions.join(" "));
    }

    if let Err(e) = thread.id.send_message(ctx, msg).await {
        if is_forbidden(&e) {
            tracing::warn!("forum: missing permissions to send in thread {} ({})", thread.id, thread.name);
        } else {
            tracing::error!("forum: failed to send in thread {} ({}): {e}", thread.id, thread.name);
        }
    }
}

async fn honeypot(ctx: &serenity::Context, message: &serenity::Message, data: &Data) {
    let hc = &data.config.honeypot;
    if message.author.bot || hc.channel_id == 0 || message.channel_id.get() != hc.channel_id {
        return;
    }
    let Some(guild_id) = message.guild_id else { return };

    let member = match message.member(ctx).await {
        Ok(m) => m,
        Err(_) => {
            tracing::warn!("honeypot: could not resolve member for user {} - skipping", message.author.id);
            return;
        }
    };

    let is_admin = ctx
        .cache
        .guild(guild_id)
        .map(|g| {
            #[allow(deprecated)]
            g.member_permissions(&member).administrator()
        })
        .unwrap_or(false);
    let has_exempt_role = member.roles.iter().any(|r| hc.exempt_roles.contains(&r.get()));
    if is_admin || has_exempt_role {
        return;
    }

    let _ = message.delete(ctx).await;

    let user = &member.user;
    let banned = ctx
        .http
        .ban_user(
            guild_id,
            user.id,
            7,
            Some("Honeypot: posted in honeypot channel (softban — auto-unbanned)"),
        )
        .await;
    let result = match banned {
        Ok(()) => {
            tokio::time::sleep(UNBAN_DELAY).await;
            ctx.http.remove_ban(guild_id, user.id, Some("Honeypot: softban complete, user may rejoin")).await
        }
        Err(e) => Err(e),
    };
    match result {
        Ok(()) => tracing::warn!("honeypot: softbanned {} ({})", user.name, user.id),
        Err(e) if is_forbidden(&e) => tracing::error!(
            "honeypot: missing permissions to ban {} ({}) - ensure bot role is above target role",
            user.name,
            user.id
        ),
        Err(e) => tracing::error!("honeypot: failed to softban {} ({}): {e}", user.name, user.id),
    }
}
