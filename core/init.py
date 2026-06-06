import logging
import discord
from discord.ext import commands
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import os

logger = logging.getLogger(__name__)


class _HealthHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/health":
            body = json.dumps({"status": "Working ok"}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, format, *args):
        pass  # suppress per-request stderr noise


def start_health_server():
    port = int(os.getenv("HEALTH_PORT", 8080))
    server = HTTPServer(("0.0.0.0", port), _HealthHandler)
    logger.info("health server listening on port %d", port)
    server.serve_forever()


def create_bot(token: str, prefix: str):
    intents = discord.Intents.default()
    intents.message_content = True
    intents.members = True
    intents.guilds = True

    bot = commands.Bot(command_prefix=prefix, intents=intents, help_command=None)

    async def setup_hook():
        from .db import init_db, set_started_at
        await init_db()
        """
        ## remove comment below this before putting PR
        ## Keep commented during testing to avoid resetting the started_at time in db on every reload.
        """
        await set_started_at()
        logger.info("setup_hook: db initialised and started_at recorded")

    bot.setup_hook = setup_hook

    @bot.event
    async def on_ready():
        logger.info("bot: logged in as %s (id: %s)", bot.user, bot.user.id)

    # NOTE: on_message lives exclusively in honeypot.py

    return bot


def load_commands(bot):
    from .load import load
    load(bot)
