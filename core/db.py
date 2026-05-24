import sqlite3
import os
import logging

logger = logging.getLogger(__name__)

DB_PATH = os.environ.get("DB_PATH", "bot.db")

_RANK_COLUMNS = frozenset({"doubts_solved", "questions_solved"})

_connection: sqlite3.Connection | None = None


def get_connection() -> sqlite3.Connection:
    global _connection
    if _connection is None:
        _connection = sqlite3.connect(DB_PATH, check_same_thread=False)
        _connection.row_factory = sqlite3.Row
        _connection.execute("PRAGMA journal_mode=WAL")
        _connection.execute("PRAGMA foreign_keys=ON")
    return _connection


def init_db() -> None:
    con = get_connection()
    con.executescript("""
        CREATE TABLE IF NOT EXISTS user_stats (
            user_id              INTEGER PRIMARY KEY,
            username             TEXT    NOT NULL,
            doubts_solved        INTEGER NOT NULL DEFAULT 0,
            questions_attempted  INTEGER NOT NULL DEFAULT 0,
            questions_solved     INTEGER NOT NULL DEFAULT 0,
            questions_skipped    INTEGER NOT NULL DEFAULT 0,
            points               REAL    NOT NULL DEFAULT 0.0,
            total_time_taken     TEXT             DEFAULT '0'
        );
        CREATE INDEX IF NOT EXISTS idx_doubts_solved
            ON user_stats (doubts_solved DESC);
        CREATE INDEX IF NOT EXISTS idx_questions_solved
            ON user_stats (questions_solved DESC);

        CREATE TABLE IF NOT EXISTS questions (
            question_id    INTEGER PRIMARY KEY AUTOINCREMENT,
            set_id         INTEGER,
            question_image TEXT,
            answer_text    TEXT,
            solution_image TEXT,
            subject        TEXT,
            topic          TEXT
        );
    """)


def increment_doubts(user_id: int, username: str, count: int = 1) -> int:
    con = get_connection()
    con.execute("""
        INSERT INTO user_stats (user_id, username, doubts_solved)
        VALUES (?, ?, ?)
        ON CONFLICT(user_id) DO UPDATE SET
            username      = excluded.username,
            doubts_solved = doubts_solved + ?
    """, (user_id, username, count, count))
    con.commit()
    return con.execute(
        "SELECT doubts_solved FROM user_stats WHERE user_id = ?", (user_id,)
    ).fetchone()["doubts_solved"]


def get_user(user_id: int) -> sqlite3.Row | None:
    return get_connection().execute(
        "SELECT * FROM user_stats WHERE user_id = ?", (user_id,)
    ).fetchone()


def get_leaderboard_doubts(limit: int = 10) -> list[sqlite3.Row]:
    return get_connection().execute(
        "SELECT * FROM user_stats ORDER BY doubts_solved DESC LIMIT ?", (limit,)
    ).fetchall()


def get_leaderboard_quiz(limit: int = 10) -> list[sqlite3.Row]:
    return get_connection().execute(
        "SELECT * FROM user_stats ORDER BY questions_solved DESC LIMIT ?", (limit,)
    ).fetchall()


def get_rank(user_id: int, column: str) -> int | None:
    if column not in _RANK_COLUMNS:
        raise ValueError(
            f"get_rank(): '{column}' is not an allowed rank column. "
            f"Allowed: {sorted(_RANK_COLUMNS)}"
        )

    con = get_connection()

    exists = con.execute(
        "SELECT 1 FROM user_stats WHERE user_id = ?", (user_id,)
    ).fetchone()
    if not exists:
        return None

    row = con.execute(f"""
        SELECT COUNT(*) + 1 AS rank
        FROM user_stats
        WHERE {column} > (SELECT {column} FROM user_stats WHERE user_id = ?)
    """, (user_id,)).fetchone()

    return row["rank"] if row else None
