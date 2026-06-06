import asyncio
import logging
import discord
from .config import config

logger = logging.getLogger(__name__)


async def apply_solved_tag(thread: discord.Thread) -> None:
    fc = config.forum
    if not fc.solved_tag_id or not isinstance(thread.parent, discord.ForumChannel):
        await thread.edit(locked=True)
        return

    solved_tag = discord.utils.get(thread.parent.available_tags, id=fc.solved_tag_id)
    current_tags = list(thread.applied_tags)
    if solved_tag and solved_tag not in current_tags:
        current_tags.append(solved_tag)
    await thread.edit(locked=True, applied_tags=current_tags)


def forum(bot):
    fc = config.forum

    @bot.event
    async def on_thread_create(thread: discord.Thread):
        if thread.guild.id != fc.server_id:
            return
        if thread.parent_id != fc.channel_id:
            return
        await asyncio.sleep(1)

        roles_to_ping: list[int] = []
        for tag in thread.applied_tags:
            role_id = fc.tag_roles.get(tag.id)
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
            logger.warning("forum: missing permissions to send in thread %s (%s)", thread.id, thread.name)
        except discord.HTTPException as e:
            logger.error("forum: failed to send in thread %s (%s): %s", thread.id, thread.name, e)
