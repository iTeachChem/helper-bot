import time
import discord


def misc(bot):
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
