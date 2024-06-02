#!/usr/bin/env nix-shell
#!nix-shell -p "pkgs.python3.withPackages (p: with p; [tabulate])"
#!nix-shell -p tesseract5 imagemagick
#!nix-shell -i python3
import csv
import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
from tabulate import tabulate
from datetime import datetime


# {{{ String helpers
def levenshtein_distance(str1, str2):
    # Create a matrix to store distances
    rows = len(str1) + 1
    cols = len(str2) + 1
    dist = [[0 for _ in range(cols)] for _ in range(rows)]

    # Initialize the matrix
    for i in range(1, rows):
        dist[i][0] = i
    for j in range(1, cols):
        dist[0][j] = j

    # Compute distances
    for i in range(1, rows):
        for j in range(1, cols):
            if str1[i - 1] == str2[j - 1]:
                cost = 0
            else:
                cost = 1
            dist[i][j] = min(
                dist[i - 1][j] + 1,  # Deletion
                dist[i][j - 1] + 1,  # Insertion
                dist[i - 1][j - 1] + cost,
            )  # Substitution

    return dist[rows - 1][cols - 1]


def closest_string(target, strings, f=lambda x: x):
    min_distance = float("inf")
    closest = None

    for string in strings:
        distance = levenshtein_distance(target, f(string))
        if distance < min_distance:
            min_distance = distance
            closest = string

    return closest, min_distance


# }}}
# {{{ Shell helpers
def run(command):
    result = subprocess.run(command, shell=True, text=True, stdout=subprocess.PIPE)
    return result.stdout


def tesseract(path, mode, extra=""):
    return run(f"tesseract {path} - --psm {mode} {extra}")


def crop(in_path, out_path, x, y, w, h):
    return run(
        f"convert '{in_path}' -crop '{w}x{h}+{x}+{y}' -colorspace Gray -auto-level -edge 1 '{out_path}'"
    )


def tesseract_region(path, region, extra="", mode=13, debug=False):
    with tempfile.TemporaryDirectory() as temp_dir:
        if debug:
            print(temp_dir)
        out_path = temp_dir + "/out.png"
        crop(path, out_path, *region)
        if debug:
            input("Press enter to continue")
        return tesseract(out_path, mode, extra).strip()


# }}}

data_dir = os.environ.get("ARCAEA_DATA_DIR")
db_path = data_dir + "/db.sqlite"
if not os.path.exists(db_path):
    run(f"cat ./schema.sql | sqlite3 {db_path}")
conn = sqlite3.connect(db_path)

tesseract_nums_only = "-c tessedit_char_whitelist=0123456789"


# {{{ Chart helpers
def parse_db_chart(db_chart):
    (id, title, difficulty, level, note_count, chart_constant, artist) = db_chart
    return {
        "id": id,
        "title": title,
        "difficulty": difficulty,
        "level": level,
        "note_count": int(note_count),
        "chart_constant": float(chart_constant),
        "artist": artist,
    }


# Returns all characters appearing at least once in an input string
def unique_characters(strings):
    unique_chars = set()

    for string in strings:
        unique_chars.update(set(string))

    return "".join(unique_chars)


raw_charts = conn.execute("SELECT * FROM charts").fetchall()
all_charts = [*map(parse_db_chart, raw_charts)]
chart_whitelist = unique_characters([chart["title"] for chart in all_charts])
chart_whitelist = chart_whitelist.replace('"', '\\"')
tesseract_song_name = f'-c tessedit_char_whitelist="{chart_whitelist}"'


def find_chart_by_name(name, difficulty, artist):
    closest, distance = closest_string(name, all_charts, lambda chart: chart["title"])
    filtered = [chart for chart in all_charts if chart["title"] == closest["title"]]
    if (
        artist is not None and closest["title"] == "Quon"
    ):  # AAAAAAAAAA, why are there two quons
        chosen_artist, _ = closest_string(
            artist, filtered, lambda chart: chart["artist"]
        )
        filtered = [
            chart for chart in filtered if chart["artist"] == chosen_artist["artist"]
        ]

    filtered = [chart for chart in filtered if chart["difficulty"] == difficulty]

    return filtered[0], distance


# }}}
# {{{ Time helpers
def current_timestamp():
    now = datetime.now()
    return now.strftime("%Y-%m-%d_%H-%M-%S")


# }}}
# {{{ Arcaea helpers
def compute_grade(score):
    if score > 9900000:
        return "EX+"
    elif score > 9800000:
        return "EX"
    elif score > 9500000:
        return "AA"
    elif score > 9200000:
        return "A"
    elif score > 8900000:
        return "B"
    elif score > 8600000:
        return "C"
    else:
        return "D"


def compute_play_rating(chart, score):
    play_rating = float(chart["chart_constant"])
    if score >= 10000000:
        play_rating += 2
    elif score >= 9800000:
        play_rating += 1 + (score - 9800000) / 200000
    else:
        play_rating += (score - 9500000) / 100000

    return play_rating


def print_scores(scores):
    data = []
    for chart, score_id, score in scores:
        title = chart["title"]
        difficulty = chart["difficulty"]
        level = chart["level"]
        note_count = chart["note_count"]

        play_rating = compute_play_rating(chart, score)
        grade = compute_grade(score)

        pm_string = ""
        if score > 10000000:
            pm_rating = score - 10000000 - note_count
            pm_string = f"({pm_rating})"

        data.append(
            [
                title,
                f"{difficulty} {level}",
                f"{score} ({grade})",
                f"{play_rating} {pm_string}",
                score_id,
            ]
        )

    print(
        tabulate(
            data,
            ["Title", "Difficulty", "Score", "Play rating", "ID"],
        )
    )


# }}}
# {{{ User helpers
def get_user_id(discord_id):
    row = conn.execute(
        "SELECT id FROM users WHERE discord_id = ?", [discord_id]
    ).fetchone()

    if row is None:
        row = conn.execute(
            "INSERT INTO users(discord_id) VALUES (?) RETURNING id", [discord_id]
        ).fetchone()

    return row[0]


# }}}


# {{{ Score parsing
def parse_score_image(image):
    topleft_text = tesseract_region(image, [0, 0, 320, 75])
    is_song_select = levenshtein_distance(
        topleft_text, "Select a Song"
    ) < levenshtein_distance(topleft_text, "Result")

    if is_song_select:
        name = tesseract_region(image, [10, 360, 1100, 80], tesseract_song_name)
        score = int(tesseract_region(image, [0, 260, 320, 60], tesseract_nums_only))
        return (name, score, "FTR", None, None)
    else:
        name = tesseract_region(image, [300, 320, 1200, 110], tesseract_song_name)
        score = int(tesseract_region(image, [850, 675, 470, 120], tesseract_nums_only))
        max_recall = int(
            tesseract_region(image, [380, 590, 130, 50], tesseract_nums_only)
        )
        difficulty = tesseract_region(image, [150, 540, 200, 40])
        difficulty, _ = closest_string(
            difficulty, ["PAST", "PRESENT", "FUTURE", "ETERNAL", "BEYOND"]
        )
        difficulty = {
            "PAST": "PST",
            "PRESENT": "PRS",
            "FUTURE": "FTR",
            "ETERNAL": "ETR",
            "BEYOND": "BYD",
        }[difficulty]
        return (name, score, difficulty, None, max_recall)


# }}}
# {{{ Add scores
def add_scores(discord_id, input_files):
    user_id = get_user_id(discord_id)
    added_scores = []

    for input_file in input_files:
        if not os.path.exists(input_file):
            print(f"Skipping non-existent {input_file}")
            continue

        name, score, difficulty, artist, max_recall = parse_score_image(input_file)
        chart, distance = find_chart_by_name(name, difficulty, artist)

        if distance < 8:
            # {{{ Happy path
            # print(f"chart title distance {distance} (parsed: {name})")
            score_id = conn.execute(
                """
                    INSERT INTO scores(chart_id,user_id,parsed_name,max_recall,score)
                    VALUES (?,?,?,?,?)
                    RETURNING id
                    """,
                (chart["id"], user_id, name, max_recall, score),
            ).fetchone()[0]
            conn.commit()
            added_scores.append((chart, score_id, score))
            # }}}
        else:
            # {{{ Confused path
            image_dir = data_dir + "/images"
            if not os.path.exists(image_dir):
                os.makedirs(image_dir)

            base_name, extension = os.path.splitext(os.path.basename(input_file))
            timestamp = current_timestamp()
            destination = f"{image_dir}/{timestamp}-{user_id}{extension}"

            shutil.copy(input_file, destination)

            artist_string = ""
            if artist is not None:
                artist_string = f" by {artist}"

            print(
                f'Couldn\'t identify "{name} ({difficulty}){artist_string}" as a chart title (distance={distance}); '
                + f"Saved file to {destination}."
            )
            # }}}

    print_scores(added_scores)


# }}}
# {{{ List scores
def list_scores(discord_id):
    rows = conn.execute(
        """
            SELECT s.id, s.score, c.id, c.title, c.difficulty, c.level, c.note_count, c.chart_constant
            FROM scores s
            JOIN charts c ON s.chart_id = c.id
            JOIN users u ON s.user_id = u.id
            WHERE u.discord_id = ?
            GROUP BY s.chart_id
            ORDER BY s.score DESC
        """,
        [discord_id],
    ).fetchall()

    print_scores(
        [
            (parse_db_chart(raw_chart), score_id, score)
            for (score_id, score, *raw_chart) in rows
        ]
    )


# }}}
# {{{ List users
def list_users():
    rows = conn.execute(
        """
        SELECT u.id, u.discord_id, u.nickname, u.ocr_config, COUNT(s.id)
        FROM users u
        LEFT JOIN scores s ON u.id = s.user_id
        GROUP BY u.id
        """
    ).fetchall()

    print(tabulate(rows, ["Id", "Discord id", "Nickname", "OCR config", "Scores"]))


# }}}
# {{{ List charts
def most_played_charts(amount):
    scores = conn.execute(
        """
        SELECT c.title, COUNT(s.id) AS score_count
        FROM charts c
        LEFT JOIN scores s ON c.id = s.chart_id
        GROUP BY c.id
        ORDER BY score_count DESC
        LIMIT ?
        """,
        [amount],
    ).fetchall()
    print(f"Here's the {amount} most played charts")
    print(tabulate(scores, ["Title", "Plays"]))


# }}}
# {{{ Import charts
def import_charts_from_csv(input_file):
    with open(input_file, mode="r") as file:
        csv_reader = csv.reader(file)

        count = 0

        for row in csv_reader:
            if len(row) > 0:
                count += 1
                [title, difficulty, level, cc, _, note_count, _, _, _, artist] = row
                row = (
                    title,
                    difficulty,
                    level,
                    int(note_count.replace(".", "").replace(",", "")),
                    float(cc),
                    artist,
                )

                conn.execute(
                    """
                        INSERT INTO charts(title,difficulty,level,note_count,chart_constant,artist)
                        VALUES (?,?,?,?,?,?)
                    """,
                    row,
                )

        conn.commit()

        print(f"Imported {count} charts")


# }}}
# {{{ Set nickname
def set_nickname(discord_id, nickname):
    user_id = get_user_id(discord_id)
    conn.execute("UPDATE users SET nickname = ? WHERE id = ?", (nickname, user_id))


# }}}

command = sys.argv[1]
subcommand = sys.argv[2]

if command == "import" and subcommand == "charts":
    import_charts_from_csv(sys.argv[3])
elif command == "add" and subcommand == "scores":
    add_scores(sys.argv[3], sys.argv[4:])
elif command == "set" and subcommand == "nickname":
    set_nickname(sys.argv[3], sys.argv[4])
elif command == "list" and subcommand == "scores":
    list_scores(sys.argv[3])
elif command == "list" and subcommand == "users":
    list_users()
elif command == "list" and subcommand == "charts":
    most_played_charts(10)
