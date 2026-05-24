import discord
from .config import config


def forum(bot):
    fc = config.forum

    GUILD_ID      = fc.server_id
    CHANNEL_ID    = fc.channel_id
    SOLVED_TAG_ID = fc.solved_tag_id
    TAG_ROLES     = fc.tag_roles

    async def apply_solved_tag(thread: discord.Thread) -> None:
        """Append the solved tag to a thread and lock+archive it."""
        if not SOLVED_TAG_ID or not isinstance(thread.parent, discord.ForumChannel):
            await thread.edit(locked=True, archived=True)
            return

        solved_tag = discord.utils.get(thread.parent.available_tags, id=SOLVED_TAG_ID)
        current_tags = list(thread.applied_tags)
        if solved_tag and solved_tag not in current_tags:
            current_tags.append(solved_tag)
        await thread.edit(locked=True, archived=True, applied_tags=current_tags)

    bot._apply_solved_tag = apply_solved_tag
    bot._forum_channel_id = CHANNEL_ID

    @bot.event
    async def on_thread_create(thread: discord.Thread):
        if thread.guild.id != GUILD_ID:
            return
        if thread.parent_id != CHANNEL_ID:
            return

        roles_to_ping: list[int] = []
        for tag in thread.applied_tags:
            role_id = TAG_ROLES.get(tag.id)
            if role_id and role_id not in roles_to_ping:
                roles_to_ping.append(role_id)

        mention_str = " ".join(f"<@&{rid}>" for rid in roles_to_ping) if roles_to_ping else None

        embed = discord.Embed(
            description=(
                "**Note for OP**\n"
                "`+solved @user1 @user2...` to close the thread when your doubt is solved. "
                "Mention the users who helped you solve the doubt. "
                "This will be added to their stats."
            ),
            color=discord.Color.blurple()
        )
        try:
            await thread.send(content=mention_str, embed=embed)
        except discord.Forbidden:
            print(f"[forum] Missing permissions to send in thread {thread.id} ({thread.name})")
        except discord.HTTPException as e:
            print(f"[forum] Failed to send in thread {thread.id}: {e}")
