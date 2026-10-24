use std::time::Instant;

use poise::serenity_prelude as serenity;
use poise::CreateReply;

use super::{guild_channel, parse_member, tag_names};
use crate::{Context, Error};

/// checks the bot's response time
#[poise::command(prefix_command)]
pub async fn ping(ctx: Context<'_>, #[rest] _rest: Option<String>) -> Result<(), Error> {
    let start = Instant::now();
    let handle = ctx.say("pinging...").await?;
    let latency = start.elapsed().as_millis();
    let embed = serenity::CreateEmbed::new().title("pong!").description(format!("latency: `{latency}ms`"));
    handle.edit(ctx, CreateReply::default().content("").embed(embed)).await?;
    Ok(())
}

/// shows all available commands
#[poise::command(prefix_command)]
pub async fn help(ctx: Context<'_>, #[rest] _rest: Option<String>) -> Result<(), Error> {
    let mut commands: Vec<_> = ctx.framework().options().commands.iter().collect();
    commands.sort_by(|a, b| a.name.cmp(&b.name));
    let mut embed = serenity::CreateEmbed::new().title("commands").description("here's everything you can do:");
    for cmd in commands {
        embed = embed.field(
            format!("`+{}`", cmd.name),
            cmd.description.as_deref().unwrap_or("no description"),
            false,
        );
    }
    ctx.send(CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// shows a user's avatar mention them or leave blank for yours
#[poise::command(prefix_command)]
pub async fn avatar(ctx: Context<'_>, #[rest] who: Option<String>) -> Result<(), Error> {
    let member = match who.as_deref().and_then(|s| s.split_whitespace().next()) {
        Some(token) => match parse_member(ctx, token).await {
            Some(m) => m,
            None => {
                ctx.say("user not found. mention them or use their exact server nickname or user id.").await?;
                return Ok(());
            }
        },
        None => match ctx.author_member().await {
            Some(m) => m.into_owned(),
            None => return Ok(()),
        },
    };

    let embed = serenity::CreateEmbed::new()
        .title(format!("{}'s avatar", member.display_name()))
        .image(member.face())
        .footer(serenity::CreateEmbedFooter::new(format!("requested by {}", ctx.author().tag())));
    ctx.send(CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// check for unsolved threads in the forum in the last 7 days
#[poise::command(prefix_command)]
pub async fn unsolved(ctx: Context<'_>, #[rest] _rest: Option<String>) -> Result<(), Error> {
    let fc = &ctx.data().config.forum;
    let Some(guild_id) = ctx.guild_id() else { return Ok(()) };
    let forum_id = serenity::ChannelId::new(fc.channel_id.max(1));
    let forum = guild_channel(ctx, forum_id).await;
    let forum_name = forum.as_ref().map_or("forum".to_string(), |f| f.name.clone());

    let now = chrono::Utc::now().timestamp();
    let week_ago = now - 7 * 24 * 3600;

    let active = guild_id.get_active_threads(ctx.http()).await?;
    let mut unsolved = Vec::new();
    for thread in &active.threads {
        if thread.parent_id != Some(forum_id) {
            continue;
        }
        let meta = thread.thread_metadata;
        if meta.is_some_and(|m| m.archived || m.locked) {
            continue;
        }
        let created = meta
            .and_then(|m| m.create_timestamp)
            .unwrap_or_else(|| thread.id.created_at())
            .unix_timestamp();
        if created <= week_ago || created > now {
            continue;
        }
        let tags = tag_names(forum.as_ref(), thread);
        if tags.iter().any(|t| t.eq_ignore_ascii_case("solved")) {
            continue;
        }
        let owner = match thread.owner_id {
            Some(id) => ctx.cache().user(id).map(|u| u.name.clone()).unwrap_or_else(|| format!("<@{id}>")),
            None => "Unknown".to_string(),
        };
        unsolved.push((thread.name.clone(), format!("https://discord.com/channels/{guild_id}/{}", thread.id), tags, owner, created));
    }

    if unsolved.is_empty() {
        let embed = serenity::CreateEmbed::new()
            .title("No Unsolved Threads")
            .description(format!("No unsolved threads in #{forum_name} created within the last week."))
            .color(0xff0000);
        ctx.send(CreateReply::default().embed(embed)).await?;
        return Ok(());
    }

    let color = if unsolved.len() > 5 { 0xff0000 } else { 0xffff00 };
    for (page, chunk) in unsolved.chunks(25).enumerate() {
        let title = if page == 0 {
            format!("Unsolved Threads in {forum_name} (Last Week)")
        } else {
            format!("Unsolved Threads in {forum_name} (Last Week) - Continued")
        };
        let mut embed = serenity::CreateEmbed::new().title(title).color(color);
        for (name, url, tags, owner, created) in chunk {
            embed = embed.field(
                name,
                format!("OP: {owner}\nTags: {}\nLink: {url}\nCreated: <t:{created}:R>", tags.join(" ")),
                false,
            );
        }
        ctx.send(CreateReply::default().embed(embed)).await?;
    }
    Ok(())
}
