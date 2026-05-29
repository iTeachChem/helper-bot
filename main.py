import logging
import os
from dotenv import load_dotenv
from core.config import config
from core.init import create_bot, load_commands

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
)

def main():
    load_dotenv()

    TOKEN = os.getenv("BOT_TOKEN")
    if not TOKEN:
        raise RuntimeError("BOT_TOKEN is not set")

    bot = create_bot(TOKEN, config.prefix)
    load_commands(bot)
    bot.run(TOKEN)


if __name__ == "__main__":
    main()
