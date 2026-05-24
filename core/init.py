import discord
from discord.ext import commands
import tomllib
import time

def create_bot(token: str, prefix: str):
    intents = discord.Intents.default()
    intents.message_content = True
    intents.guilds = True       

    bot = commands.Bot(command_prefix=prefix, intents=intents, help_command=None)

    @bot.event
    async def on_ready():
        print(f"logged in as {bot.user}")

    @bot.event
    async def on_message(message):
        if message.author.bot:
            return
        await bot.process_commands(message)

    return bot

def load_commands(bot):
    from .load import load
    load(bot)
