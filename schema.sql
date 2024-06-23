# {{{ users
create table IF NOT EXISTS users (
    id INTEGER NOT NULL PRIMARY KEY,
    discord_id TEXT UNIQUE NOT NULL,
    nickname TEXT UNIQUE
);
# }}}
# {{{ songs
CREATE TABLE IF NOT EXISTS songs (
    id INTEGER NOT NULL PRIMARY KEY,
    title TEXT NOT NULL,
    ocr_alias TEXT,
    artist TEXT,

    UNIQUE(title, artist)
);
# }}}
# {{{ charts
CREATE TABLE IF NOT EXISTS charts (
    id INTEGER NOT NULL PRIMARY KEY,
    song_id INTEGER NOT NULL,
    jacket TEXT,

    difficulty TEXT NOT NULL CHECK (difficulty IN ('PST','PRS','FTR','ETR','BYD')),
    level TEXT NOT NULL,

    note_count INTEGER NOT NULL,
    chart_constant INTEGER NOT NULL,

    FOREIGN KEY (song_id) REFERENCES songs(id),
    UNIQUE(song_id, difficulty)
);
# }}}
# {{{ plays
CREATE TABLE IF NOT EXISTS plays (
    id INTEGER NOT NULL PRIMARY KEY,
    chart_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    discord_attachment_id TEXT,

    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    creation_ptt INTEGER,
    creation_zeta_ptt INTEGER,

    score INTEGER NOT NULL,
    zeta_score INTEGER,

    max_recall INTEGER,
    far_notes INTEGER,

    FOREIGN KEY (chart_id) REFERENCES charts(id),
    FOREIGN KEY (user_id) REFERENCES users(id)
);
# }}}

insert into users(discord_id, nickname) values (385759924917108740, 'prescientmoon');
