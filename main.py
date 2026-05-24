from core.init import create_bot, load_commands
import os
from dotenv import load_dotenv
import tomllib

def main():
    load_dotenv()

    TOKEN = os.getenv("BOT_TOKEN")

    with open("config.toml", "rb") as f:
        config = tomllib.load(f)

    PREFIX = config["prefix"]

    bot = create_bot(TOKEN, PREFIX)

    load_commands(bot)

    bot.run(TOKEN)

if __name__ == "__main__":
    main()