import time
import discord
from discord.ext import commands
from datetime import datetime, timedelta, timezone
from .config import config

def misc(bot):
    fc = config.forum

    @bot.command(help="checks the bot's response time")
    async def ping(ctx):
        start = time.perf_counter()
        msg = await ctx.send("pinging...")
        latency = round((time.perf_counter() - start) * 1000)

        embed = discord.Embed(
            title="pong!",
            description=f"latency: `{latency}ms`"
        )
        await msg.edit(content=None, embed=embed)

    @bot.command(help="shows all available commands")
    async def help(ctx):
        embed = discord.Embed(
            title="commands",
            description="here's everything you can do:"
        )
        for command in sorted(bot.commands, key=lambda c: c.name):
            embed.add_field(
                name=f"`+{command.name}`",
                value=command.help or "no description",
                inline=False
            )
        await ctx.send(embed=embed)

    @bot.command(help="shows a user's avatar mention them or leave blank for yours")
    async def avatar(ctx, member: discord.Member = None):
        member = member or ctx.author
        embed = discord.Embed(title=f"{member.display_name}'s avatar")
        embed.set_image(url=member.display_avatar.url)
        embed.set_footer(text=f"requested by {ctx.author}")
        await ctx.send(embed=embed)

    @avatar.error
    async def avatar_error(ctx, error):
        if isinstance(error, discord.ext.commands.MemberNotFound):
            await ctx.send("user not found. mention them or use their exact server nickname or user id.")


    @bot.command(name="unsolved", help="check for unsolved threads in the forum in the last 7 days")
    async def check_unsolved_threads(ctx: commands.Context):
        _fc = bot.get_channel(fc.channel_id) 

        unsolved_threads = []
        now = datetime.now(timezone.utc) + timedelta(hours= 5, minutes= 30)
        one_week_ago = now - timedelta(days= 7)

        for thread in _fc.threads:
            if (not thread.archived and not thread.locked and 
                one_week_ago < thread.created_at.replace(tzinfo=timezone.utc) <= now and
                not any(tag.name.lower() == "solved" for tag in thread.applied_tags)):
                created_time = int(thread.created_at.timestamp())
                owner_name = thread.owner.name if thread.owner else "Unknown"
                tag_names = [tag.name for tag in thread.applied_tags]
                unsolved_threads.append((thread.name, thread.jump_url, tag_names, owner_name, created_time))
        
        if unsolved_threads:
            color = 0xff0000 if len(unsolved_threads) > 5 else 0xffff00
            embeds = []
            embed = discord.Embed(title=f"Unsolved Threads in {_fc.name} (Last Week)", color=color)
            
            for i, (name, url, tags, owner_name, created_time) in enumerate(unsolved_threads):
                if i > 0 and i % 25 == 0:
                    embeds.append(embed)
                    embed = discord.Embed(title=f"Unsolved Threads in {_fc.name} (Last Week) - Continued", color=color)
                
                embed.add_field(name=name, value=f"OP: {owner_name}\nTags: {' '.join(tags)}\nLink: {url}\nCreated: <t:{created_time}:R>", inline=False)
            
            embeds.append(embed)
            
            for embed in embeds:
                await ctx.send(embed=embed)
        else:
            embed = discord.Embed(title="No Unsolved Threads", description=f'No unsolved threads in #{_fc.name} created within the last week.', color=0xff0000)
            await ctx.send(embed=embed)