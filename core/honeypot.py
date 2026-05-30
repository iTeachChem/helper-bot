import asyncio
import logging
import discord
from .config import config

logger = logging.getLogger(__name__)

_UNBAN_DELAY = 3


def _is_exempt(member: discord.Member) -> bool:
    if member.guild_permissions.administrator:
        return True
    exempt = config.honeypot.exempt_roles
    return any(role.id in exempt for role in member.roles)


def honeypot(bot):
    hc = config.honeypot

    @bot.event
    async def on_message(message: discord.Message):
        if message.author.bot:
            await bot.process_commands(message)
            return

        await bot.process_commands(message)

        if not hc.channel_id or message.channel.id != hc.channel_id:
            return

        member = message.guild.get_member(message.author.id)
        if member is None:
            logger.warning(
                "honeypot: could not resolve member for user %s — skipping",
                message.author.id,
            )
            return

        if _is_exempt(member):
            logger.info(
                "honeypot: skipping exempt member %s (%s)",
                member, member.id,
            )
            return

        guild = message.guild
        logger.info(
            "honeypot: actioning %s (%s) — posted in #%s",
            member, member.id, message.channel,
        )

        try:
            await guild.ban(
                member,
                reason="Honeypot: posted in honeypot channel (softban — auto-unbanned)",
                delete_message_days=1,
            )
            logger.info("honeypot: banned %s (%s) — waiting %ss before unban", member, member.id, _UNBAN_DELAY)
            await asyncio.sleep(_UNBAN_DELAY)
            await guild.unban(
                discord.Object(id=member.id),
                reason="Honeypot: softban complete, user may rejoin",
            )
            logger.info("honeypot: softban complete for %s (%s) — user may rejoin", member, member.id)

        except discord.Forbidden:
            logger.error(
                "honeypot: missing permissions to ban %s (%s) — "
                "ensure the bot role is above the target role",
                member, member.id,
            )
        except discord.HTTPException as e:
            logger.error("honeypot: failed to softban %s (%s): %s", member, member.id, e)
