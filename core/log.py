import asyncio
import logging
import os
import time
import httpx

_WEBHOOK_URL = os.environ.get("LOG_WEBHOOK", "")
_MAX_LEN = 1900
_BLOCKED_PREFIXES = ("httpx", "discord", "asyncio", "urllib3", "websockets")
_MIN_INTERVAL = 2.0

_queue: asyncio.Queue | None = None
_worker_task: asyncio.Task | None = None


def _is_blocked(name: str) -> bool:
    return any(name == p or name.startswith(p + ".") for p in _BLOCKED_PREFIXES)


def _split(text: str) -> list[str]:
    chunks = []
    while text:
        chunks.append(text[:_MAX_LEN])
        text = text[_MAX_LEN:]
    return chunks


async def _worker() -> None:
    last_sent = 0.0
    while True:
        payload = await _queue.get()
        try:
            now = time.monotonic()
            wait = _MIN_INTERVAL - (now - last_sent)
            if wait > 0:
                await asyncio.sleep(wait)
            async with httpx.AsyncClient() as client:
                resp = await client.post(_WEBHOOK_URL, json={"content": payload}, timeout=10)
                if resp.status_code == 429:
                    retry_after = float(resp.json().get("retry_after", _MIN_INTERVAL))
                    await asyncio.sleep(retry_after)
                    await _queue.put(payload)
                else:
                    last_sent = time.monotonic()
        except Exception:
            pass
        finally:
            _queue.task_done()


def _ensure_worker() -> None:
    global _queue, _worker_task
    try:
        loop = asyncio.get_running_loop()
    except RuntimeError:
        return
    if _queue is None:
        _queue = asyncio.Queue()
    if _worker_task is None or _worker_task.done():
        _worker_task = loop.create_task(_worker())


class WebhookHandler(logging.Handler):
    def emit(self, record: logging.LogRecord) -> None:
        if not _WEBHOOK_URL or _is_blocked(record.name):
            return
        try:
            line = self.format(record)
        except Exception:
            line = record.getMessage()
        _ensure_worker()
        if _queue is None:
            return
        for chunk in _split(line):
            try:
                _queue.put_nowait(f"```\n{chunk}\n```")
            except asyncio.QueueFull:
                pass


def setup() -> None:
    if not _WEBHOOK_URL:
        logging.getLogger(__name__).warning("log: LOG_WEBHOOK not set — webhook logging disabled")
        return
    handler = WebhookHandler()
    handler.setLevel(logging.WARNING)  # webhook only gets WARNING and above
    handler.setFormatter(logging.Formatter(
        "%(asctime)s [%(levelname)s] %(name)s: %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
    ))
    logging.getLogger().addHandler(handler)
