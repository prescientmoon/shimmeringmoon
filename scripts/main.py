#!/usr/bin/env nix-shell
#!nix-shell -p "pkgs.python3.withPackages (p: with p; [tabulate])"
#!nix-shell -i python3
import csv
import os
import sqlite3
import sys

data_dir = os.environ.get("SHIMMERING_DATA_DIR")
db_path = data_dir + "/db.sqlite"
# if not os.path.exists(db_path):
# run(f"cat ./schema.sql | sqlite3 {db_path}")
conn = sqlite3.connect(db_path)


# {{{ Import songs
def import_charts_from_csv(input_file):
    with open(input_file, mode="r") as file:
        chart_count = 0
        songs = dict()

        for row in csv.reader(file):
            if len(row) > 0:
                chart_count += 1
                [title, difficulty, level, cc, _, note_count, _, _, _] = row
                if songs.get(title) is None:
                    songs[title] = []
                songs[title].append((difficulty, level, cc, note_count))

        for title, charts in songs.items():
            artist = None

            if title.startswith("Quon"):
                artist = title[6:-1]
                title = "Quon"

            row = conn.execute(
                """
                    INSERT INTO songs(title,artist)
                    VALUES (?,?)
                    RETURNING id
                """,
                (title, artist),
            ).fetchone()
            song_id = row[0]

            for difficulty, level, cc, note_count in charts:
                conn.execute(
                    """
                        INSERT INTO charts(song_id, difficulty, level, note_count, chart_constant)
                        VALUES(?,?,?,?,?)
                    """,
                    (
                        song_id,
                        difficulty,
                        level,
                        int(note_count.replace(",", "").replace(".", "")),
                        int(float(cc) * 100),
                    ),
                )

        conn.commit()

        print(f"Imported {chart_count} charts and {len(songs)} songs")


# }}}

command = sys.argv[1]
subcommand = sys.argv[2]

if command == "import" and subcommand == "charts":
    import_charts_from_csv(sys.argv[3])
