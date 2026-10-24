use poise::serenity_prelude as serenity;
use poise::CreateReply;

use super::{author_can_manage_threads, current_thread, error_embed, guild_channel, looks_like_user, parse_member, tag_names};
use crate::error::is_forbidden;
use crate::{Context, Error};

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

    let Some(thread) = current_thread(ctx).await? else {
        ctx.say("this command can only be used inside a forum thread.").await?;
        return Ok(());
    };
    let Some(parent_id) = thread.parent_id.filter(|p| p.get() == fc.channel_id) else {
        ctx.say("this command can only be used inside the designated forum channel.").await?;
        return Ok(());
    };

    let author = ctx.author().id;
    let is_op = thread.owner_id == Some(author);
    let is_whitelisted = fc.whitelist.contains(&author.get());
    if !(is_op || is_whitelisted || author_can_manage_threads(ctx, thread.id).await) {
        ctx.say("only the thread op or a moderator can mark a thread as solved.").await?;
        return Ok(());
    }

    // mentions / ids must resolve; plain words ("thanks") are ignored
    let mut members: Vec<serenity::Member> = Vec::new();
    for token in helpers.as_deref().unwrap_or("").split_whitespace() {
        match parse_member(ctx, token).await {
            Some(m) => {
                if !members.iter().any(|x| x.user.id == m.user.id) {
                    members.push(m);
                }
            }
            None if looks_like_user(token) => {
                ctx.say("couldn't find one of those users. make sure you're mentioning them properly.").await?;
                return Ok(());
            }
            None => {}
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
    if tag_names(forum.as_ref(), &thread).iter().any(|t| t.eq_ignore_ascii_case("solved")) {
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

    for m in &helpers {
        if let Err(e) = ctx.data().db.increment_doubts(m.user.id.get(), m.display_name(), 1).await {
            tracing::error!("solved: db error while incrementing doubts: {e}");
            ctx.say("thread closed, but there was a problem updating stats. please contact an admin.").await?;
            return Ok(());
        }
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
    if !author_can_manage_threads(ctx, ctx.channel_id()).await {
        error_embed(ctx, "Permission denied", "You don't have permission to use this command.").await;
        return Ok(());
    }

    let fc = &ctx.data().config.forum;
    let Some(thread) = current_thread(ctx).await? else {
        ctx.say("this command can only be used inside a forum thread.").await?;
        return Ok(());
    };
    let Some(parent_id) = thread.parent_id.filter(|p| p.get() == fc.channel_id) else {
        ctx.say("this command can only be used inside the designated forum channel.").await?;
        return Ok(());
    };

    let forum = guild_channel(ctx, parent_id).await;
    let locked = thread.thread_metadata.is_some_and(|m| m.locked);
    let has_solved = tag_names(forum.as_ref(), &thread).iter().any(|t| t.eq_ignore_ascii_case("solved"));
    if !locked && !has_solved {
        ctx.say("this thread is not closed.").await?;
        return Ok(());
    }

    let solved_ids: Vec<serenity::ForumTagId> = forum
        .as_ref()
        .map(|f| f.available_tags.iter().filter(|t| t.name == "Solved").map(|t| t.id).collect())
        .unwrap_or_default();
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
