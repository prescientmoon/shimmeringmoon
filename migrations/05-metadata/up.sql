CREATE TABLE IF NOT EXISTS metadata (
  -- We only a single metadata row in the entire db
  id INTEGER PRIMARY KEY CHECK (id == 0),

  -- The last hash computed for the directory
  -- containing all the raw jackets. If this
  -- hash changes, every jacket is reprocessed.
  raw_jackets_hash TEXT NOT NULL
) STRICT;

-- Inserts initial metadata row
INSERT INTO metadata VALUES(0, "");
