CREATE TABLE IF NOT EXISTS metadata (
  -- Only allow a single metadata row in the entire db.
  id INTEGER PRIMARY KEY CHECK (id == 0),

  -- The last hash computed for the directory
  -- containing all the raw jackets. If this
  -- hash changes, every jacket is reprocessed.
  raw_jackets_hash TEXT DEFAULT "" NOT NULL,

  -- If any of these files change, every chart/song is reprocessed.
  songlist_hash TEXT DEFAULT "" NOT NULL,
  cc_data_hash TEXT DEFAULT "" NOT NULL,
  notecount_hash TEXT DEFAULT "" NOT NULL
) STRICT;

-- Inserts initial metadata row.
INSERT INTO metadata(id) VALUES(0);
