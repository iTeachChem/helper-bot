import logging
import time
import discord
from discord.ext import commands
from .db import increment_doubts
from .config import config
from .forum import apply_solved_tag

logger = logging.getLogger(__name__)


def solved(bot):
    fc = config.forum

    @bot.command(name="solved", help="marks this thread as solved and credits the users who helped you")
    async def solved_cmd(ctx, helpers: commands.Greedy[discord.Member]):
        if not isinstance(ctx.channel, discord.Thread):
            await ctx.send("this command can only be used inside a forum thread.")
            return

        thread: discord.Thread = ctx.channel

        if thread.parent_id != fc.channel_id:
            await ctx.send("this command can only be used inside the designated forum channel.")
            return

        is_op          = thread.owner_id == ctx.author.id
        is_whitelisted = ctx.author.id in fc.whitelist
        can_manage     = ctx.channel.permissions_for(ctx.author).manage_threads

        if not (is_op or is_whitelisted or can_manage):
            await ctx.send("only the thread op or a moderator can mark a thread as solved.")
            return

        if any(m.id == ctx.author.id for m in helpers):
            await ctx.send(
                "you can't mention yourself as a helper. "
                "mention the users who actually helped you, or run `+solved` with no mentions."
            )
            return

        valid_helpers = [m for m in helpers if not m.bot]

        try:
            await apply_solved_tag(thread)
        except discord.Forbidden:
            logger.error("solved: missing permissions to apply solved tag on thread '%s' (%s)", thread.name, thread.id)
            await ctx.send("i don't have permission to lock this thread — please close it manually.")
            return
        except discord.HTTPException as e:
            logger.error("solved: HTTP error on thread '%s' (%s): %s", thread.name, thread.id, e)
            await ctx.send("something went wrong while closing the thread. please try again or contact an admin.")
            return

        try:
            for member in valid_helpers:
                await increment_doubts(user_id=member.id, username=member.display_name)
        except Exception as e:
            logger.error("solved: db error while incrementing doubts: %s", e)
            await ctx.send("thread closed, but there was a problem updating stats. please contact an admin.")
            return

        timestamp = int(time.time())
        embed = discord.Embed(color=discord.Color.green())
        embed.add_field(name="archived by", value=ctx.author.mention, inline=True)
        if valid_helpers:
            embed.add_field(name="solved by", value=" ".join(m.mention for m in valid_helpers), inline=True)
        embed.add_field(name="time", value=f"<t:{timestamp}:F>", inline=False)
        await ctx.send(embed=embed)

    @solved_cmd.error
    async def solved_error(ctx, error):
        if isinstance(error, commands.BadArgument):
            await ctx.send("couldn't find one of those users. make sure you're mentioning them properly.")
