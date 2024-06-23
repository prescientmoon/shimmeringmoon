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
def import_charts_from_csv():
    chart_count = 0
    songs = dict()

    with open(data_dir + "/charts.csv", mode="r") as file:
        for row in csv.reader(file):
            if len(row) > 0:
                chart_count += 1
                [title, difficulty, level, cc, _, note_count, _, _, _] = row
                if songs.get(title) is None:
                    songs[title] = {"charts": [], "shorthand": None}
                songs[title]["charts"].append([difficulty, level, cc, note_count, None])

    with open(data_dir + "/jackets.csv", mode="r") as file:
        for row in csv.reader(file):
            if len(row) > 0:
                [title, jacket, difficulty] = row
                if difficulty.strip() != "":
                    changed = 0

                    for i in range(len(songs[title]["charts"])):
                        if songs[title]["charts"][i][0] == difficulty:
                            songs[title]["charts"][i][4] = jacket
                            changed += 1

                    if changed == 0:
                        raise f"Nothing changed for chart {title} [{difficulty}]"
                else:
                    for i in range(len(songs[title]["charts"])):
                        songs[title]["charts"][i][4] = jacket

    with open(data_dir + "/shorthands.csv", mode="r") as file:
        for row in csv.reader(file):
            if len(row) > 0:
                [title, shorthand] = row
                songs[title]["shorthand"] = shorthand

    for title, entry in songs.items():
        artist = None

        # Problematic titles that can belong to multiple artists
        for possibility in ["Quon", "Gensis"]:
            if title.startswith(possibility):
                artist = title[len(possibility) + 2 : -1]
                title = possibility
                break

        row = conn.execute(
            """
                INSERT INTO songs(title,artist,ocr_alias)
                VALUES (?,?,?)
                RETURNING id
            """,
            (title, artist, entry.get("shorthand")),
        ).fetchone()
        song_id = row[0]

        for difficulty, level, cc, note_count, jacket in entry["charts"]:
            conn.execute(
                """
                        INSERT INTO charts(song_id, difficulty, level, note_count, chart_constant, jacket)
                        VALUES(?,?,?,?,?, ?)
                    """,
                (
                    song_id,
                    difficulty,
                    level,
                    int(note_count.replace(",", "").replace(".", "")),
                    int(float(cc) * 100),
                    jacket,
                ),
            )

    conn.commit()

    print(f"Imported {chart_count} charts and {len(songs)} songs")


# }}}

command = sys.argv[1]
subcommand = sys.argv[2]

if command == "import" and subcommand == "charts":
    import_charts_from_csv()
&song_title
if command == "export" and subcommand == "jackets":
    import_charts_from_csv()
