use std::time::Instant;

use poise::serenity_prelude as serenity;
use poise::CreateReply;

use super::{guild_channel, parse_member, solved_tag_ids, tag_names};
use crate::{Context, Error};

/// Discord's per-embed limits; the character budget leaves room for the title.
const EMBED_MAX_FIELDS: usize = 25;
const EMBED_FIELD_CHARS: usize = 5800;

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

    let solved_ids = solved_tag_ids(ctx, forum.as_ref());

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
        if thread.applied_tags.iter().any(|t| solved_ids.contains(t)) {
            continue;
        }
        let tags = tag_names(forum.as_ref(), thread);
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

    let fields: Vec<(String, String)> = unsolved
        .into_iter()
        .map(|(name, url, tags, owner, created)| {
            (name, format!("OP: {owner}\nTags: {}\nLink: {url}\nCreated: <t:{created}:R>", tags.join(" ")))
        })
        .collect();
    let color = if fields.len() > 5 { 0xff0000 } else { 0xffff00 };
    for (page, chunk) in embed_pages(&fields).into_iter().enumerate() {
        let title = if page == 0 {
            format!("Unsolved Threads in {forum_name} (Last Week)")
        } else {
            format!("Unsolved Threads in {forum_name} (Last Week) - Continued")
        };
        let mut embed = serenity::CreateEmbed::new().title(title).color(color);
        for (name, value) in chunk {
            embed = embed.field(name, value, false);
        }
        ctx.send(CreateReply::default().embed(embed)).await?;
    }
    Ok(())
}

/// Split fields into embeds that stay under both the field count and total character limits.
fn embed_pages(fields: &[(String, String)]) -> Vec<&[(String, String)]> {
    let mut pages = Vec::new();
    let (mut start, mut chars) = (0, 0);
    for (i, (name, value)) in fields.iter().enumerate() {
        let len = name.chars().count() + value.chars().count();
        if i > start && (i - start == EMBED_MAX_FIELDS || chars + len > EMBED_FIELD_CHARS) {
            pages.push(&fields[start..i]);
            (start, chars) = (i, 0);
        }
        chars += len;
    }
    if start < fields.len() {
        pages.push(&fields[start..]);
    }
    pages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embed_pages_respect_limits() {
        let field = |n: usize| ("t".repeat(100), "v".repeat(n));
        let small: Vec<_> = (0..30).map(|_| field(10)).collect();
        assert_eq!(embed_pages(&small).iter().map(|p| p.len()).collect::<Vec<_>>(), [25, 5]);
        let big: Vec<_> = (0..25).map(|_| field(130)).collect();
        assert!(embed_pages(&big).iter().all(|p| p.iter().map(|(a, b)| a.len() + b.len()).sum::<usize>() <= EMBED_FIELD_CHARS));
        assert!(embed_pages(&[]).is_empty());
    }
}
