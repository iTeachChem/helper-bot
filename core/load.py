from .db import init_db
from .misc import misc
from .forum import forum
from .solved import solved
from .stats import stats


def load(bot):
    init_db()
    misc(bot)
    forum(bot)
    solved(bot)
    stats(bot)
