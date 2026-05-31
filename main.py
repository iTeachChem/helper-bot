import logging
import os
from dotenv import load_dotenv
from core.config import config
from core.init import create_bot, load_commands, start_health_server
from core.log import setup as setup_webhook_logging
import threading

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
)

def main():
    load_dotenv()

    threading.Thread(target=start_health_server, daemon=True).start()

    TOKEN = os.getenv("BOT_TOKEN")
    if not TOKEN:
        raise RuntimeError("BOT_TOKEN is not set")

    setup_webhook_logging()

    bot = create_bot(TOKEN, config.prefix)
    load_commands(bot)
    bot.run(TOKEN)


if __name__ == "__main__":
    main()