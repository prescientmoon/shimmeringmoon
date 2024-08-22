-- {{{ songs
CREATE TABLE IF NOT EXISTS songs (
    id INTEGER NOT NULL PRIMARY KEY,
    title TEXT NOT NULL,
    artist TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('light', 'conflict', 'silent')),
    bpm TEXT NOT NULL,
    pack TEXT,

    UNIQUE(title, artist)
);
-- }}}
-- {{{ charts
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
-- }}}
