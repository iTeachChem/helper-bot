import logging
import discord
from discord.ext import commands

logger = logging.getLogger(__name__)


def create_bot(token: str, prefix: str):
    intents = discord.Intents.default()
    intents.message_content = True
    intents.members = True
    intents.guilds = True

    bot = commands.Bot(command_prefix=prefix, intents=intents, help_command=None)

    async def setup_hook():
        from .db import init_db, set_started_at
        await init_db()
        await set_started_at()
        logger.info("setup_hook: db initialised and started_at recorded")

    bot.setup_hook = setup_hook

    @bot.event
    async def on_ready():
        logger.info("bot: logged in as %s (id: %s)", bot.user, bot.user.id)

    # NOTE: on_message lives exclusively in honeypot.py

    @bot.event
    async def on_command_error(ctx, error):
        if hasattr(ctx.command, "on_error"):
            return
        if isinstance(error, commands.CommandNotFound):
            return
        logger.error(
            "command_error: %s raised in +%s by %s: %s",
            type(error).__name__, ctx.command, ctx.author, error,
        )

    return bot


def load_commands(bot):
    from .load import load
    load(bot)
