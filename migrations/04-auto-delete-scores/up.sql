-- Automatically delete all associated scores
-- every time a play is deleted.
CREATE TRIGGER auto_delete_scores AFTER DELETE ON plays
BEGIN
  DELETE FROM scores
  WHERE play_id = OLD.id;
END;
