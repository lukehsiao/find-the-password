CREATE TABLE IF NOT EXISTS players
(
    id          INTEGER PRIMARY KEY NOT NULL,
    name        TEXT                NOT NULL,
    solved      BOOLEAN             NOT NULL DEFAULT 0
);
