import logging
import discord
from discord.ext import commands

logger = logging.getLogger(__name__)


def create_bot(token: str, prefix: str):
    intents = discord.Intents.default()
    intents.message_content = True
    intents.guilds = True

    bot = commands.Bot(command_prefix=prefix, intents=intents, help_command=None)

    async def setup_hook():
        """Runs exactly once on startup, before the bot connects."""
        from .db import init_db, set_started_at
        await init_db()
        await set_started_at()
        logger.info("setup_hook: db initialised and started_at recorded")

    bot.setup_hook = setup_hook

    @bot.event
    async def on_ready():
        logger.info("logged in as %s", bot.user)

    @bot.event
    async def on_message(message):
        if message.author.bot:
            return
        await bot.process_commands(message)

    return bot


def load_commands(bot):
    from .load import load
    load(bot)
