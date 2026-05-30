import logging
import discord
from discord.ext import commands
from .db import get_user_with_ranks, get_leaderboard_doubts, get_leaderboard_quiz
from .config import config

logger = logging.getLogger(__name__)


def format_time(total_seconds: float) -> str:
    minutes = int(total_seconds // 60)
    secs    = total_seconds % 60
    if minutes > 0:
        return f"{minutes}m {secs:.2f}s"
    return f"{secs:.2f}s"


def stats(bot):

    @bot.command(name="stats", help="shows quiz and doubt stats for you or another member")
    async def stats_cmd(ctx, member: discord.Member = None):
        target = member or ctx.author
        try:
            row = await get_user_with_ranks(target.id)
        except Exception as e:
            logger.error("stats: db error fetching user %s: %s", target.id, e)
            await ctx.send("couldn't fetch stats right now. please try again later.")
            return

        embed = discord.Embed(title=f"{target.display_name}'s stats", color=discord.Color.blurple())
        embed.set_thumbnail(url=target.display_avatar.url)

        if not row:
            embed.description = "no stats found for this user."
            await ctx.send(embed=embed)
            return

        attempted  = row["questions_attempted"]
        solved     = row["questions_solved"]
        skipped    = row["questions_skipped"]
        points     = row["points"]
        total_secs = row["total_time_taken"] or 0.0
        doubts     = row["doubts_solved"]
        quiz_rank   = f"#{row['quiz_rank']}"   if row["quiz_rank"]   is not None else "n/a"
        doubts_rank = f"#{row['doubts_rank']}" if row["doubts_rank"] is not None else "n/a"

        accuracy   = (solved  / attempted * 100) if attempted > 0 else 0.0
        skip_pct   = (skipped / attempted * 100) if attempted > 0 else 0.0
        avg_points = (points  / attempted)       if attempted > 0 else 0.0
        avg_secs   = (total_secs / attempted)    if attempted > 0 else 0.0

        embed.description = (
            f"**rankings**\nquiz rank: `{quiz_rank}`\ndoubts rank: `{doubts_rank}`\n\n"
            f"**doubts**\nsolved: `{doubts}`\n\n"
            f"**performance**\naccuracy: `{accuracy:.2f}%`\nskipped: `{skip_pct:.2f}%`\n"
            f"avg points: `{avg_points:.2f}`\navg time: `{avg_secs:.2f}s`\n\n"
            f"**quiz**\nattempted: `{attempted}`\nsolved: `{solved}`\nskipped: `{skipped}`\n"
            f"points: `{int(points)}`\ntotal time: `{format_time(total_secs)}`"
        )
        await ctx.send(embed=embed)

    @stats_cmd.error
    async def stats_error(ctx, error):
        if isinstance(error, commands.BadArgument):
            await ctx.send("couldn't find that user.")

    @bot.command(name="lb", help="shows the leaderboard — use `+lb doubts` or `+lb quiz`")
    async def lb_cmd(ctx, board: str = None):
        if board is None:
            await ctx.send("usage: `+lb doubts` or `+lb quiz`")
            return

        board = board.lower()
        if board == "doubts":
            fetch, title, key, label = get_leaderboard_doubts, "top doubt solvers", "doubts_solved", "doubts solved"
        elif board == "quiz":
            fetch, title, key, label = get_leaderboard_quiz, "top quiz solvers", "questions_solved", "questions solved"
        else:
            await ctx.send("unknown leaderboard. use `+lb doubts` or `+lb quiz`.")
            return

        try:
            rows = await fetch(config.excluded)
        except Exception as e:
            logger.error("lb: db error fetching %s leaderboard: %s", board, e)
            await ctx.send("couldn't fetch the leaderboard right now. please try again later.")
            return

        if not rows:
            await ctx.send("no data yet.")
            return

        embed = discord.Embed(
            title=title,
            description="\n".join(f"`#{i}` **{row['username']}** {row[key]} {label}" for i, row in enumerate(rows, 1)),
            color=discord.Color.blurple()
        )
        await ctx.send(embed=embed)
