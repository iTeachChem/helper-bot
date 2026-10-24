mod misc;
mod solved;
mod stats;

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use poise::serenity_prelude::{self as serenity, ArgumentConvert};

use crate::{Context, Data, Error};

pub fn all() -> Vec<poise::Command<Data, Error>> {
    vec![
        misc::ping(),
        misc::help(),
        misc::avatar(),
        misc::unsolved(),
        solved::solved(),
        solved::reopen(),
        stats::stats(),
        stats::lb(),
    ]
}

/// Per-user command cooldowns; users in `cooldown_exempt_ids` skip them.
#[derive(Default)]
pub struct Cooldowns(Mutex<HashMap<(&'static str, serenity::UserId), Instant>>);

impl Cooldowns {
    /// Returns the remaining wait if the user is still cooling down, otherwise
    /// starts a new cooldown window.
    pub fn hit(&self, command: &'static str, user: serenity::UserId, per: Duration) -> Option<Duration> {
        let mut map = self.0.lock().unwrap();
        let now = Instant::now();
        map.retain(|_, until| *until > now);
        if let Some(until) = map.get(&(command, user)) {
            return Some(*until - now);
        }
        map.insert((command, user), now + per);
        None
    }
}

/// Enforce a cooldown, replying with the "Cooldown active" embed when hit.
/// Returns false when the command should stop.
pub async fn check_cooldown(ctx: Context<'_>, command: &'static str, per: Duration) -> bool {
    let user = ctx.author().id;
    if ctx.data().config.is_cooldown_exempt(user.get()) {
        return true;
    }
    match ctx.data().cooldowns.hit(command, user, per) {
        None => true,
        Some(left) => {
            error_embed(
                ctx,
                "Cooldown active",
                &format!("Please wait {:.1} seconds before using this command again.", left.as_secs_f64()),
            )
            .await;
            false
        }
    }
}

pub async fn error_embed(ctx: Context<'_>, title: &str, description: &str) {
    if let poise::Context::Prefix(p) = ctx {
        crate::error::send_error(&ctx.serenity_context().http, p.msg, title, description).await;
    }
}

/// Resolve a mention, id, or name to a guild member.
pub async fn parse_member(ctx: Context<'_>, token: &str) -> Option<serenity::Member> {
    serenity::Member::convert(ctx.serenity_context(), ctx.guild_id(), Some(ctx.channel_id()), token)
        .await
        .ok()
}

/// Mentions (`<@123>` / `<@!123>`) and raw ids: tokens the user clearly meant as a member.
pub fn looks_like_user(token: &str) -> bool {
    let digits = match token.strip_prefix("<@").and_then(|t| t.strip_suffix('>')) {
        Some(inner) => inner.strip_prefix('!').unwrap_or(inner),
        None => token,
    };
    digits.len() >= 15 && digits.chars().all(|c| c.is_ascii_digit())
}

/// The current channel if it's a thread, fetched fresh so tags/lock state are current
/// (the cache can lag behind thread edits, and misses archived threads entirely).
async fn current_thread(ctx: Context<'_>) -> Result<Option<serenity::GuildChannel>, Error> {
    let channel = ctx.http().get_channel(ctx.channel_id()).await?;
    Ok(match channel {
        serenity::Channel::Guild(gc)
            if matches!(
                gc.kind,
                serenity::ChannelType::PublicThread
                    | serenity::ChannelType::PrivateThread
                    | serenity::ChannelType::NewsThread
            ) =>
        {
            Some(gc)
        }
        _ => None,
    })
}

/// The current thread and its parent id, if it's a thread in the configured forum.
/// Replies with the reason and returns None otherwise.
pub async fn forum_thread(ctx: Context<'_>) -> Result<Option<(serenity::GuildChannel, serenity::ChannelId)>, Error> {
    let Some(thread) = current_thread(ctx).await? else {
        ctx.say("this command can only be used inside a forum thread.").await?;
        return Ok(None);
    };
    let Some(parent_id) = thread.parent_id.filter(|p| p.get() == ctx.data().config.forum.channel_id) else {
        ctx.say("this command can only be used inside the designated forum channel.").await?;
        return Ok(None);
    };
    Ok(Some((thread, parent_id)))
}

/// Ids of the forum's Solved tag: the configured one, or any tag named "solved" when unset.
pub fn solved_tag_ids(ctx: Context<'_>, forum: Option<&serenity::GuildChannel>) -> Vec<serenity::ForumTagId> {
    let tag_id = ctx.data().config.forum.solved_tag_id;
    if tag_id != 0 {
        return vec![serenity::ForumTagId::new(tag_id)];
    }
    forum
        .map(|f| f.available_tags.iter().filter(|t| t.name.eq_ignore_ascii_case("solved")).map(|t| t.id).collect())
        .unwrap_or_default()
}

/// A guild channel by id, from the cache when possible.
pub async fn guild_channel(ctx: Context<'_>, id: serenity::ChannelId) -> Option<serenity::GuildChannel> {
    let cached = ctx.guild().and_then(|g| g.channels.get(&id).cloned());
    if cached.is_some() {
        return cached;
    }
    match ctx.http().get_channel(id).await {
        Ok(serenity::Channel::Guild(gc)) => Some(gc),
        _ => None,
    }
}

/// Names of the tags applied to a forum thread.
pub fn tag_names(forum: Option<&serenity::GuildChannel>, thread: &serenity::GuildChannel) -> Vec<String> {
    let Some(forum) = forum else { return Vec::new() };
    thread
        .applied_tags
        .iter()
        .filter_map(|id| forum.available_tags.iter().find(|t| t.id == *id).map(|t| t.name.clone()))
        .collect()
}

/// Does the author have Manage Threads in a thread's parent channel? Threads use their
/// parent's permissions; taking the parent avoids the thread cache, which skips archived threads.
pub async fn author_can_manage_threads(ctx: Context<'_>, parent: serenity::ChannelId) -> bool {
    let Some(member) = ctx.author_member().await else { return false };
    let Some(guild) = ctx.guild() else { return false };
    match guild.channels.get(&parent) {
        Some(ch) => guild.user_permissions_in(ch, &member).manage_threads(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_like_tokens() {
        assert!(looks_like_user("<@859821981057679410>"));
        assert!(looks_like_user("<@!859821981057679410>"));
        assert!(looks_like_user("859821981057679410"));
        assert!(!looks_like_user("thanks"));
        assert!(!looks_like_user("<@&1227987967939838003>"));
    }

    #[test]
    fn cooldown_window() {
        let c = Cooldowns::default();
        let u = serenity::UserId::new(1);
        assert!(c.hit("stats", u, Duration::from_secs(10)).is_none());
        assert!(c.hit("stats", u, Duration::from_secs(10)).is_some());
        assert!(c.hit("lb", u, Duration::from_secs(10)).is_none());
    }
}
