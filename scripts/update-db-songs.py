import sqlite3
import json
import sys
import os

# Check if the correct number of arguments are provided
if len(sys.argv) != 2:
    print("Usage: update-db-songs <db_file>")
    sys.exit(1)

# {{{ Collect data
json_file_path = f"{os.environ.get('SHIMMERING_CC_DIR')}/ptt.json"
db_file_path = sys.argv[1]

try:
    with open(json_file_path, "r") as json_file:
        json_data = json.load(json_file)
except Exception as e:
    print(f"Error reading JSON file: {e}")
    sys.exit(1)

try:
    conn = sqlite3.connect(db_file_path)
    cursor = conn.cursor()
except sqlite3.Error as e:
    print(f"Error connecting to SQLite database: {e}")
    sys.exit(1)

cursor.execute("SELECT song_id FROM chart")
current_entries = {row[0] for row in cursor.fetchall()}
# }}}
# {{{ Print diff & delete entries
json_entries = set(json_data.keys())
removed_entries = current_entries - json_entries
if removed_entries:
    print(f"Removed entries: {removed_entries}")
else:
    print("No entries were removed.")

added_entries = json_entries - current_entries
if added_entries:
    print(f"Added entries: {added_entries}")
else:
    print("No new entries were added.")

cursor.execute("DELETE FROM chart")
# }}}
# {{{ Add new entries
for song_id, ratings in json_data.items():
    cursor.execute(
        """
            INSERT INTO chart(song_id)
            VALUES (?)
        """,
        [song_id],
    )

    for rating_type, rating_value in ratings.items():
        rating_column = ["prs", "pst", "ftr", "byn", "etr"][int(rating_type)]
        rating_column = f"rating_{rating_column}"
        cursor.execute(
            f"""
                UPDATE chart
                SET {rating_column}=?
                WHERE song_id=?
            """,
            (rating_value, song_id),
        )

conn.commit()
conn.close()
# }}}

# Print final status
print("Database updated successfully.")
