CREATE TABLE user (
    user_id PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    solved INTEGER NOT NULL DEFAULT 0,
    solved_at TEXT,
    hit_before_solved INTEGER NOT NULL DEFAULT 0,
    total_hits INTEGER NOT NULL DEFAULT 0,
    seed INTEGER NOT NULL,
    secret TEXT NOT NULL
);
