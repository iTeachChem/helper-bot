import logging
import tomllib
from dataclasses import dataclass

logger = logging.getLogger(__name__)


@dataclass(frozen=True)
class ForumConfig:
    server_id: int
    channel_id: int
    solved_tag_id: int
    whitelist: list[int]
    tag_roles: dict[int, int]


@dataclass(frozen=True)
class HoneypotConfig:
    channel_id: int
    exempt_roles: tuple[int, ...]

@dataclass(frozen=True)
class UrlsConfig:
    site_url: str
    uptime_url: str

@dataclass(frozen=True)
class CooldownExemptConfig:
    user_ids: tuple[int, ...]

@dataclass(frozen=True)
class Config:
    prefix: str
    forum: ForumConfig
    honeypot: HoneypotConfig
    excluded: tuple[str, ...]
    urls: UrlsConfig
    cooldown_exempt: CooldownExemptConfig


def _load() -> Config:
    with open("config.toml", "rb") as f:
        raw = tomllib.load(f)

    fc = raw.get("forum", {})
    forum = ForumConfig(
        server_id=int(fc.get("server_id", 0)),
        channel_id=int(fc.get("channel_id", 0)),
        solved_tag_id=int(fc.get("solved_tag_id", 0)),
        whitelist=[int(i) for i in fc.get("whitelist", [])],
        tag_roles={int(k): int(v) for k, v in fc.get("tag_roles", {}).items()},
    )

    hc = raw.get("honeypot", {})
    honeypot = HoneypotConfig(
        channel_id=int(hc.get("channel_id", 0)),
        exempt_roles=tuple(int(r) for r in hc.get("exempt_roles", [])),
    )

    uc = raw.get("urls", {})
    urls = UrlsConfig(
        site_url=uc.get("site_url", ""),
        uptime_url=uc.get("uptime_url", ""),
    )

    excluded = tuple(raw.get("excluded", []))

    cfg = Config(
        prefix=raw.get("prefix", "$"),
        forum=forum,
        honeypot=honeypot,
        excluded=excluded,
        urls=urls,
        cooldown_exempt=CooldownExemptConfig(
            user_ids=tuple(int(id) for id in raw.get("cooldown_exempt_ids", {}).get("user_ids", []))
        )
    )

    critical = {
        "forum.server_id": cfg.forum.server_id,
        "forum.channel_id": cfg.forum.channel_id,
        "forum.solved_tag_id": cfg.forum.solved_tag_id,
        "honeypot.channel_id": cfg.honeypot.channel_id,
    }
    for name, val in critical.items():
        if val == 0:
            logger.warning(
                "config: '%s' is 0 - events for this value will be silently ignored",
                name,
            )

    return cfg


config: Config = _load()
