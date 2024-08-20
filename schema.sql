# {{{ users
create table IF NOT EXISTS users (
    id INTEGER NOT NULL PRIMARY KEY,
    discord_id TEXT UNIQUE NOT NULL,
    is_pookie BOOL NOT NULL DEFAULT 0
);
# }}}
# {{{ songs
CREATE TABLE IF NOT EXISTS songs (
    id INTEGER NOT NULL PRIMARY KEY,
    title TEXT NOT NULL,
    artist TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('light', 'conflict', 'silent')),
    bpm TEXT NOT NULL,
    pack TEXT,

    UNIQUE(title, artist)
);
# }}}
# {{{ charts
CREATE TABLE IF NOT EXISTS charts (
    id INTEGER NOT NULL PRIMARY KEY,
    song_id INTEGER NOT NULL,
    note_design TEXT,
    shorthand TEXT,

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

    max_recall INTEGER,
    far_notes INTEGER,

    FOREIGN KEY (chart_id) REFERENCES charts(id),
    FOREIGN KEY (user_id) REFERENCES users(id)
);
# }}}
# {{{ scores
CREATE TABLE IF NOT EXISTS scores (
   id INTEGER NOT NULL PRIMARY KEY,
   play_id INTEGER NOT NULL,

   score INTEGER NOT NULL,
   creation_ptt INTEGER,
   scoring_system NOT NULL CHECK (scoring_system IN ('standard', 'sdf', 'ex')),

   FOREIGN KEY (play_id) REFERENCES plays(id),
   UNIQUE(play_id, scoring_system)
)
# }}}

insert into users(discord_id) values (385759924917108740);
