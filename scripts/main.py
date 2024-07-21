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
    song_count = 0
    shorthand_count = 0

    with open(data_dir + "/charts.csv", mode="r") as file:
        for i, row in enumerate(csv.reader(file)):
            if i == 0 or len(row) == 0:
                continue

            song_count += 1
            [
                title,
                artist,
                pack,
                *charts,
                side,
                bpm,
                version,
                date,
                ext_version,
                ext_date,
                original,
            ] = map(lambda v: v.strip().replace("\n", " "), row)

            song_id = conn.execute(
                """
                    INSERT INTO songs(title,artist,pack,side,bpm)
                    VALUES (?,?,?,?,?)
                    RETURNING id
                """,
                (title, artist, pack, side.lower(), bpm),
            ).fetchone()[0]

            for i in range(4):
                [note_design, level, cc, note_count] = charts[i * 4 : (i + 1) * 4]
                if note_design == "N/A":
                    continue
                chart_count += 2

                [difficulty, level] = level.split(" ")

                conn.execute(
                    """
                        INSERT INTO charts(song_id, difficulty, level, note_count, chart_constant, note_design)
                        VALUES(?,?,?,?,?, ?)
                    """,
                    (
                        song_id,
                        difficulty,
                        level,
                        int(note_count.replace(",", "").replace(".", "")),
                        int(round(float(cc) * 100)),
                        note_design if len(note_design) else None,
                    ),
                )

    with open(data_dir + "/shorthands.csv", mode="r") as file:
        for i, row in enumerate(csv.reader(file)):
            if i == 0 or len(row) == 0:
                continue

            shorthand_count += 1
            [name, difficulty, artist, shorthand] = map(lambda v: v.strip(), row)
            conn.execute(
                f"""
                    UPDATE charts
                    SET shorthand=?
                    WHERE EXISTS (
                        SELECT 1 FROM songs s
                        WHERE s.id = charts.song_id
                        AND s.title=?
                        {"" if artist=="" else "AND artist=?"}
                    )
                    {"" if difficulty=="" else "AND difficulty=?"}
                """,
                [
                    shorthand,
                    name,
                    *([] if artist == "" else [artist]),
                    *([] if difficulty == "" else [difficulty]),
                ],
            )

    conn.commit()

    print(
        f"Imported {chart_count} charts, {song_count} songs, and {shorthand_count} shorthands"
    )


# }}}

command = sys.argv[1]
subcommand = sys.argv[2]

if command == "import" and subcommand == "charts":
    import_charts_from_csv()
if command == "export" and subcommand == "jackets":
    import_charts_from_csv()
