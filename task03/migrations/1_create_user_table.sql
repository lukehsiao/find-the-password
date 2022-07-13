CREATE TABLE user (
    user_id PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    solved INTEGER NOT NULL DEFAULT 0,
    solved_at INTEGER,
    hit_before_solved INTEGER NOT NULL DEFAULT 0,
    total_hits INTEGER NOT NULL DEFAULT 0,
    seed INTEGER NOT NULL,
    secret TEXT NOT NULL
);
