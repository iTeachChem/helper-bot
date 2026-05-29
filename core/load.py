from .misc import misc
from .forum import forum
from .solved import solved
from .stats import stats


def load(bot):
    misc(bot)
    forum(bot)
    solved(bot)
    stats(bot)
