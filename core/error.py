import discord
from discord.ext import commands
import logging
from .config import config

logger = logging.getLogger(__name__)

async def send_error(ctx, title, description, *, reply=False):
    embed = discord.Embed(
        title=title,
        description=description,
        color=0xff0000
    )

    sender = ctx.reply if reply else ctx.send
    await sender(embed=embed, delete_after=10)

def cooldown_exempt(id):
    return True if id in config.cooldown_exempt_ids else False

def error_handlers(bot):

    @bot.event
    async def on_command_error(ctx, error):
        if hasattr(ctx.command, "on_error"):
            return

        if isinstance(error, commands.CommandNotFound):
            await send_error(ctx, "Unknown command", f"Use `{config.prefix}help` to see available commands.", reply=True)
        elif isinstance(error, commands.MissingRequiredArgument):
            await send_error(ctx, "Missing argument", f"Usage: `{ctx.prefix}{ctx.command} {ctx.command.signature}`", reply=True)
        elif isinstance(error, commands.BadArgument):
            await send_error(ctx, "Invalid argument", str(error), reply=True)
        elif isinstance(error, commands.MissingPermissions):
            await send_error(ctx, "Permission denied", "You don't have permission to use this command.", reply=True)
        elif isinstance(error, commands.BotMissingPermissions):
            await send_error(ctx, "Permission error", "I don't have the necessary permissions to execute this command.", reply=True)
        elif isinstance(error, commands.CommandOnCooldown):
            if await cooldown_exempt(ctx.author.id):
                ctx.command.reset_cooldown(ctx)
                await ctx.reinvoke()
                return
            
            await send_error(ctx, "Cooldown active", f"Please wait {error.retry_after:.1f} seconds before using this command again.", reply=True)
        else:
            logger.error("Unhandled command error in +%s by %s: %s (%s)", ctx.command, ctx.author, type(error).__name__, error)
            await send_error(ctx, "Error", "An unexpected error occurred while processing your command. Please try again later.", reply=True)