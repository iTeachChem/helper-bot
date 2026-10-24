use poise::serenity_prelude as serenity;
use poise::CreateReply;

use super::{author_can_manage_threads, error_embed, forum_thread, guild_channel, looks_like_user, parse_member, solved_tag_ids};
use crate::error::is_forbidden;
use crate::{Context, Error};

/// Discord allows at most this many tags on a forum post.
const MAX_APPLIED_TAGS: usize = 5;

/// Lock the thread and add the configured Solved tag (when it's a forum post).
async fn apply_solved_tag(
    ctx: Context<'_>,
    thread: &serenity::GuildChannel,
    forum: Option<&serenity::GuildChannel>,
) -> Result<(), serenity::Error> {
    let tag_id = ctx.data().config.forum.solved_tag_id;
    let mut edit = serenity::EditThread::new().locked(true);
    if let Some(forum) = forum.filter(|f| f.kind == serenity::ChannelType::Forum && tag_id != 0) {
        let mut tags = thread.applied_tags.clone();
        let exists = forum.available_tags.iter().any(|t| t.id.get() == tag_id);
        if exists && !tags.iter().any(|t| t.get() == tag_id) {
            // make room for Solved rather than have discord reject the edit
            tags.truncate(MAX_APPLIED_TAGS - 1);
            tags.push(serenity::ForumTagId::new(tag_id));
        }
        edit = edit.applied_tags(tags);
    }
    thread.id.edit_thread(ctx, edit).await.map(|_| ())
}

/// marks this thread as solved and credits the users who helped you
#[poise::command(prefix_command)]
pub async fn solved(ctx: Context<'_>, #[rest] helpers: Option<String>) -> Result<(), Error> {
    let fc = &ctx.data().config.forum;

    let Some((thread, parent_id)) = forum_thread(ctx).await? else { return Ok(()) };

    let author = ctx.author().id;
    let is_op = thread.owner_id == Some(author);
    let is_whitelisted = fc.whitelist.contains(&author.get());
    if !(is_op || is_whitelisted || author_can_manage_threads(ctx, parent_id).await) {
        ctx.say("only the thread op or a moderator can mark a thread as solved.").await?;
        return Ok(());
    }

    // only mentions / ids count as helpers and must resolve; plain words ("thanks so much")
    // are ignored, so they can't match some member's name
    let mut members: Vec<serenity::Member> = Vec::new();
    for token in helpers.as_deref().unwrap_or("").split_whitespace().filter(|t| looks_like_user(t)) {
        let Some(m) = parse_member(ctx, token).await else {
            ctx.say("couldn't find one of those users. make sure you're mentioning them properly.").await?;
            return Ok(());
        };
        if !members.iter().any(|x| x.user.id == m.user.id) {
            members.push(m);
        }
    }

    if members.iter().any(|m| m.user.id == author) {
        ctx.say(
            "you can't mention yourself as a helper. \
             mention the users who actually helped you, or run `+solved` with no mentions.",
        )
        .await?;
        return Ok(());
    }
    let helpers: Vec<_> = members.into_iter().filter(|m| !m.user.bot).collect();

    let forum = guild_channel(ctx, parent_id).await;
    let solved_ids = solved_tag_ids(ctx, forum.as_ref());
    if thread.applied_tags.iter().any(|t| solved_ids.contains(t)) {
        ctx.say("this thread is already marked as solved.").await?;
        return Ok(());
    }

    if let Err(e) = apply_solved_tag(ctx, &thread, forum.as_ref()).await {
        if is_forbidden(&e) {
            tracing::error!("solved: missing permissions to apply solved tag on thread '{}' ({})", thread.name, thread.id);
            ctx.say("i don't have permission to lock this thread — please close it manually.").await?;
        } else {
            tracing::error!("solved: HTTP error on thread '{}' ({}): {e}", thread.name, thread.id);
            ctx.say("something went wrong while closing the thread. please try again or contact an admin.").await?;
        }
        return Ok(());
    }

    let credits: Vec<(u64, &str)> = helpers.iter().map(|m| (m.user.id.get(), m.display_name())).collect();
    if let Err(e) = ctx.data().db.increment_doubts(&credits).await {
        tracing::error!("solved: db error while incrementing doubts: {e}");
        // nobody was credited; undo the lock/tag so `+solved` can simply be run again
        let revert = serenity::EditThread::new().locked(false).applied_tags(thread.applied_tags.clone());
        if let Err(e) = thread.id.edit_thread(ctx, revert).await {
            tracing::error!("solved: failed to revert thread '{}' ({}): {e}", thread.name, thread.id);
            ctx.say("there was a problem updating stats and the thread is still locked. please contact an admin.").await?;
        } else {
            ctx.say("there was a problem updating stats, so the thread wasn't closed. please try again.").await?;
        }
        return Ok(());
    }

    let mut embed = serenity::CreateEmbed::new()
        .color(0x2ecc71)
        .field("archived by", format!("<@{author}>"), true);
    if !helpers.is_empty() {
        let mentions: Vec<String> = helpers.iter().map(|m| format!("<@{}>", m.user.id)).collect();
        embed = embed.field("solved by", mentions.join(" "), true);
    }
    embed = embed.field("time", format!("<t:{}:F>", chrono::Utc::now().timestamp()), false);
    ctx.send(CreateReply::default().embed(embed)).await?;

    thread.id.edit_thread(ctx, serenity::EditThread::new().archived(true)).await?;
    Ok(())
}

#[poise::command(prefix_command)]
pub async fn reopen(ctx: Context<'_>, #[rest] _rest: Option<String>) -> Result<(), Error> {
    let Some((thread, parent_id)) = forum_thread(ctx).await? else { return Ok(()) };

    if !author_can_manage_threads(ctx, parent_id).await {
        error_embed(ctx, "Permission denied", "You don't have permission to use this command.").await;
        return Ok(());
    }

    let forum = guild_channel(ctx, parent_id).await;
    let solved_ids = solved_tag_ids(ctx, forum.as_ref());
    let locked = thread.thread_metadata.is_some_and(|m| m.locked);
    let has_solved = thread.applied_tags.iter().any(|t| solved_ids.contains(t));
    if !locked && !has_solved {
        ctx.say("this thread is not closed.").await?;
        return Ok(());
    }

    let new_tags: Vec<_> = thread.applied_tags.iter().copied().filter(|t| !solved_ids.contains(t)).collect();

    let edit = serenity::EditThread::new().locked(false).archived(false).applied_tags(new_tags);
    match thread.id.edit_thread(ctx, edit).await {
        Ok(_) => {
            let author = ctx.author().id;
            let embed = serenity::CreateEmbed::new()
                .title("Post unlocked and unarchived successfully!")
                .color(0x575287)
                .field("Unarchived by", format!("<@{author}> ({author})"), true);
            ctx.send(CreateReply::default().embed(embed)).await?;
        }
        Err(e) => {
            tracing::error!("reopen: error while reopening thread '{}' ({}): {e}", thread.name, thread.id);
            ctx.say("something went wrong while reopening the thread. please try again or contact an admin.").await?;
        }
    }
    Ok(())
}
