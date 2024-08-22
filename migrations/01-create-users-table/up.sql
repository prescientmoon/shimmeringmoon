-- {{{ users
create table IF NOT EXISTS users (
    id INTEGER NOT NULL PRIMARY KEY,
    discord_id TEXT UNIQUE NOT NULL,
    is_pookie BOOL NOT NULL DEFAULT 0
);
-- }}}
-- {{{ plays
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
-- }}}
-- {{{ scores
CREATE TABLE IF NOT EXISTS scores (
   id INTEGER NOT NULL PRIMARY KEY,
   play_id INTEGER NOT NULL,

   score INTEGER NOT NULL,
   creation_ptt INTEGER,
   scoring_system TEXT NOT NULL CHECK (scoring_system IN ('standard', 'sdf', 'ex')),

   FOREIGN KEY (play_id) REFERENCES plays(id),
   UNIQUE(play_id, scoring_system)
)
-- }}}
