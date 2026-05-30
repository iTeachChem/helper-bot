import asyncio
import logging
import os
import httpx

_WEBHOOK_URL = os.environ.get("LOG_WEBHOOK", "")
_MAX_LEN = 1900  

def _split(text: str) -> list[str]:
    """Split long text into chunks that fit inside a Discord code block."""
    chunks = []
    while text:
        chunks.append(text[:_MAX_LEN])
        text = text[_MAX_LEN:]
    return chunks


async def _send(content: str) -> None:
    if not _WEBHOOK_URL:
        return
    try:
        async with httpx.AsyncClient() as client:
            await client.post(_WEBHOOK_URL, json={"content": content}, timeout=10)
    except Exception:
        pass  

class WebhookHandler(logging.Handler):
    """Logging handler that ships records to a Discord webhook as code blocks."""

    def emit(self, record: logging.LogRecord) -> None:
        if not _WEBHOOK_URL:
            return
        try:
            line = self.format(record)
        except Exception:
            line = record.getMessage()

        for chunk in _split(line):
            payload = f"```\n{chunk}\n```"
            try:
                loop = asyncio.get_running_loop()
                loop.create_task(_send(payload))
            except RuntimeError:
               
                asyncio.run(_send(payload))


def setup(level: int = logging.INFO) -> None:
    """
    Call once from main.py before bot.run().
    Attaches the WebhookHandler to the root logger so every module's
    logger automatically ships to the webhook.
    """
    if not _WEBHOOK_URL:
        logging.getLogger(__name__).warning(
            "log: LOG_WEBHOOK not set — webhook logging disabled"
        )
        return

    handler = WebhookHandler()
    handler.setLevel(level)
    handler.setFormatter(logging.Formatter(
        "%(asctime)s [%(levelname)s] %(name)s: %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
    ))
    logging.getLogger().addHandler(handler)
