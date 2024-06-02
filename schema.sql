create table users (
    id INTEGER PRIMARY KEY,
    discord_id TEXT NOT NULL,
    nickname TEXT UNIQUE,
    ocr_config TEXT
);

CREATE TABLE charts (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    difficulty TEXT NOT NULL CHECK (difficulty IN ('PST','PRS','FTR','ETR','BYD')),
    level TEXT NOT NULL,
    note_count INTEGER NOT NULL,
    chart_constant REAL NOT NULL,
    artist TEXT
);

CREATE TABLE scores (
    id INTEGER PRIMARY KEY,
    chart_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    score INTEGER NOT NULL,
    parsed_name TEXT,
    max_recall INTEGER,
    creation_timestamp TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (chart_id) REFERENCES charts(id),
    FOREIGN KEY (user_id) REFERENCES users(id)
);

insert into users(discord_id,nickname) values (385759924917108740,'prescientmoon');
