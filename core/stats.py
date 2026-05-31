import io
import logging
import aiohttp
import discord
from discord.ext import commands

from .db import get_user_with_ranks, get_leaderboard_doubts, get_leaderboard_quiz
from .config import config
from .leaderboard.lb_template import generate_lb_card
from .leaderboard.stats_template import generate_stats_card

logger = logging.getLogger(__name__)


def _fmt_time(secs: float) -> str:
    h = int(secs // 3600)
    m = int((secs % 3600) // 60)
    s = secs % 60
    if h:  return f"{h}h {m}m"
    if m:  return f"{m}m {s:.0f}s"
    return f"{s:.2f}s"


async def _fetch_avatar(session: aiohttp.ClientSession, url: str) -> bytes | None:
    """Fetch avatar bytes using a shared session. Returns None on failure."""
    try:
        async with session.get(str(url)) as r:
            return await r.read() if r.status == 200 else None
    except Exception as exc:
        logger.warning("avatar fetch failed: %s", exc)
        return None


def stats(bot):

    @bot.command(name="stats", help="show quiz & doubt stats for you or another member")
    async def stats_cmd(ctx: commands.Context, member: discord.Member = None):
        target = member or ctx.author

        try:
            row = await get_user_with_ranks(target.id)
        except Exception as e:
            logger.error("stats db error %s: %s", target.id, e)
            await ctx.send("couldn't fetch stats right now, try again later.")
            return
        
        if not row:
            await ctx.send(f"no stats found for **{target.display_name}**.")
            return

        try:
            attempted  = int(row.get("questions_attempted", 0))
            solved     = int(row.get("questions_solved", 0))
            skipped    = int(row.get("questions_skipped", 0))
            points     = int(row.get("points", 0))
            total_secs = float(row.get("total_time_taken", 0))
            doubts     = int(row.get("doubts_solved", 0))

            quiz_rank   = f"#{row['quiz_rank']}"   if row.get("quiz_rank")   is not None else "n/a"
            doubts_rank = f"#{row['doubts_rank']}" if row.get("doubts_rank") is not None else "n/a"

            accuracy = (solved / attempted * 100) if attempted > 0 else 0.0
            avg_pts  = (points / attempted)       if attempted > 0 else 0.0
            avg_secs = (total_secs / attempted)   if attempted > 0 else 0.0

            data = dict(
                username       = target.display_name,
                server_name    = ctx.guild.name if ctx.guild else "Direct Message",
                quiz_rank      = quiz_rank,
                doubts_rank    = doubts_rank,
                attempted      = attempted,
                solved         = solved,
                skipped        = skipped,
                points         = int(points) if points else None,
                total_time_str = _fmt_time(total_secs) if total_secs else None,
                doubts_solved  = doubts,
                accuracy       = accuracy,
                avg_time       = avg_secs,
                avg_points     = avg_pts,
            )
        except Exception as e:
            logger.error("stats data error %s: %s", target.id, e)
            await ctx.send("Stats data error, try again later.")
            return

        async with aiohttp.ClientSession() as session:
            avatar_bytes = await _fetch_avatar(session, target.display_avatar.url)

        async with ctx.typing():
            try:
                png = await generate_stats_card(avatar_bytes, data)
            except Exception as e:
                logger.error("stats render error %s: %s", target.id, e)
                await ctx.send("couldn't generate stats card, try again later.")
                return
            finally:
                avatar_bytes = None

        buf = io.BytesIO(png)
        png = None  
        try:
            button = discord.ui.Button(
                label="View Full Leaderboard",
                style=discord.ButtonStyle.link,
                url=config.urls.site_url,
            )
            view = discord.ui.View()
            view.add_item(button)
            await ctx.send(file=discord.File(buf, "stats.png"), view=view)
        finally:
            buf.close()

    @stats_cmd.error
    async def stats_error(ctx, error):
        if isinstance(error, commands.BadArgument):
            await ctx.send("couldn't find that user.")

    # leaderboard command for doubts and quiz
    @bot.command(name="lb", help="leaderboard — `+lb doubts` or `+lb quiz`")
    async def lb_cmd(ctx, board: str = None):
        if not board:
            await ctx.send("usage: `+lb doubts` or `+lb quiz`")
            return

        board = board.lower()
        if board == "doubts":
            fetch   = get_leaderboard_doubts
            val_key = "doubts_solved"
        elif board == "quiz":
            fetch   = get_leaderboard_quiz
            val_key = "questions_solved"
        else:
            await ctx.send("unknown leaderboard — use `+lb doubts` or `+lb quiz`.")
            return

        try:
            db_rows = await fetch(config.excluded)
        except Exception as e:
            logger.error("lb db error %s: %s", board, e)
            await ctx.send("couldn't fetch leaderboard, try again later.")
            return

        if not db_rows:
            await ctx.send("no data yet.")
            return

        card_rows = [{"username": r["username"], "value": r[val_key]} for r in db_rows]

        async with ctx.typing():
            try:
                png = await generate_lb_card(board, card_rows)
            except Exception as e:
                logger.error("lb render error %s: %s", board, e)
                await ctx.send("couldn't generate leaderboard card, try again later.")
                return

        buf = io.BytesIO(png)
        png = None  
        try:
            button = discord.ui.Button(
                label="View Full Leaderboard",
                style=discord.ButtonStyle.link,
                url=config.urls.site_url,
            )
            view = discord.ui.View()
            view.add_item(button)
            await ctx.send(file=discord.File(buf, f"lb_{board}.png"), view=view)
        finally:
            buf.close()