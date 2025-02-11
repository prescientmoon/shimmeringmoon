-- {{{ users
create table IF NOT EXISTS users (
    id INTEGER NOT NULL PRIMARY KEY,
    discord_id TEXT UNIQUE NOT NULL,
    private_server_id INTEGER,
    is_pookie BOOL NOT NULL DEFAULT 0,
    is_admin  BOOL NOT NULL DEFAULT 0
);
-- }}}
