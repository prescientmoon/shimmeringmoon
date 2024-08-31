#!/usr/bin/env nix-shell
#!nix-shell -p "pkgs.python3.withPackages (p: with p; [tabulate])"
#!nix-shell -p tesseract5 imagemagick
#!nix-shell -i python3
import csv
import re
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
    str1 = str1.lower()
    str2 = str2.lower()
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
    result = subprocess.run(command, shell=True, text=True, capture_output=True)
    return str(result.stdout)


def tesseract(path, mode, extra="", num=False):
    if num and extra == "":
        extra = "-c tessedit_char_whitelist=0123456789"
    output = run(f"tesseract {path} - --psm {mode} {extra}")
    return output


def crop(in_path, out_path, x, y, w, h, hard_to_read=True, debug=False):
    bt = 10  # black threshold
    sw = w  # small width
    sh = h  # small height
    br = 0.1  # blur radius

    # We only downscale large images
    if w >= 100 and h >= 100:
        sw = w / 3
        sh = h / 3
        br = 2
        # this can lead to weird results
        if hard_to_read:
            bt = 150

    # Only apply more stuff when we are unsure
    if hard_to_read:
        bonus_opts = f"-blur '{br}x10' -resize '{sw}x{sh}' -edge 0.75 -black-threshold {bt} -negate"
    else:
        bonus_opts = ""

    command = f"convert '{in_path}' -crop '{w}x{h}+{x}+{y}' -colorspace Gray -auto-level {bonus_opts} '{out_path}'"
    if debug:
        print(command)
    return run(command)


def tesseract_region(
    path, region, extra="", hard_to_read=True, mode=7, debug=False, num=False
):
    with tempfile.TemporaryDirectory() as temp_dir:
        out_path = temp_dir + "/out.png"
        if debug:
            print(out_path)
        crop(path, out_path, *region, debug=debug, hard_to_read=hard_to_read)
        if debug:
            input("Press enter to continue")
        output = tesseract(out_path, mode, extra, num=num).strip()
        if num:
            output = int(output)
    return output


def parse_srgb(srgb_string):
    # Regular expression to match the numbers within the srgb() string
    match = re.match(r"srgb\((\d+),(\d+),(\d+)\)", srgb_string)

    if not match:
        raise ValueError(f"Invalid srgb format: {srgb_string}")

    r, g, b = map(int, match.groups())
    return r, g, b


def pixel_at(image, x, y):
    return parse_srgb(run(f'convert {image} -format "%[pixel:u.p{{{x},{y}}}]\n" info:'))


# }}}

data_dir = os.environ.get("ARCAEA_DATA_DIR")
db_path = data_dir + "/db.sqlite"
if not os.path.exists(db_path):
    run(f"cat ./schema.sql | sqlite3 {db_path}")
conn = sqlite3.connect(db_path)


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

all_difficulties = ["PAST", "PRESENT", "FUTURE", "ETERNAL", "BEYOND"]
difficulty_whitelist = unique_characters(all_difficulties)
tesseract_difficulty = f"-c tessedit_char_whitelist={difficulty_whitelist}"


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

    if len(filtered) > 0:
        closest = filtered[0]

    # This is so hacky, omg
    if distance >= len(closest["title"]) / 3 and len(name) > 3:
        nested = find_chart_by_name(name[:-1].strip(), difficulty, artist)
        # print(name[:-1], nested[0]["title"], nested[1], closest["title"], distance)
        if nested[1] < 10:
            return nested

    return closest, distance


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
        chart_constant = chart["chart_constant"]

        play_rating = "%2.2f" % compute_play_rating(chart, score)
        grade = compute_grade(score)

        pm_rating = None
        if score > 10000000:
            pm_rating = score - 10000000 - note_count
            pm_rating = f"({pm_rating})"

        data.append(
            [
                title,
                f"{difficulty:>3} {level} ({chart_constant})",
                score,
                grade,
                play_rating,
                pm_rating,
                score_id,
            ]
        )

    print(
        tabulate(
            data,
            ["Title", "Difficulty", "Score", "Grade", "Play rating", "PM rating", "ID"],
            colalign=("left", "left", "right", "center", "center", "center", "center"),
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
    topleft_text = tesseract_region(image, [0, 15, 320, 60], mode=13)
    is_song_select = levenshtein_distance(
        topleft_text, "Select a Song"
    ) < levenshtein_distance(topleft_text, "Result")

    if is_song_select:
        name = tesseract_region(image, [10, 350, 1100, 105], tesseract_song_name)
        artist = tesseract_region(image, [10, 445, 800, 50])
        score = tesseract_region(image, [0, 260, 320, 60], num=True)

        # {{{ Difficulty parsing
        difficulty = None
        pst_pixel = pixel_at(image, 25, 150)
        prs_pixel = pixel_at(image, 210, 150)
        ftr_pixel = pixel_at(image, 400, 150)
        byd_pixel = pixel_at(image, 650, 125)

        if 190 < prs_pixel[0] < 210 and prs_pixel[1] > 200 and 100 < prs_pixel[2] < 170:
            difficulty = "PRS"
        elif (
            200 < ftr_pixel[0] and 100 < ftr_pixel[1] < 160 and 100 < ftr_pixel[2] < 200
        ):
            difficulty = "FTR"
        elif 190 < pst_pixel[0] < 210 and 235 < pst_pixel[2]:
            difficulty = "PST"
        elif 150 < byd_pixel[0] and byd_pixel[1] < 30 and byd_pixel[2] < 70:
            difficulty = "BYD"
        elif (
            100 < byd_pixel[0] < 200
            and 100 < byd_pixel[1] < 200
            and 100 < byd_pixel[2] < 200
        ):
            difficulty = "ETR"
        else:
            print(pst_pixel)
            print(prs_pixel)
            print(ftr_pixel)
            print(byd_pixel)
            difficulty = "UKN"  # Unknown
        # }}}

        return (name, score, difficulty, artist, None)
    else:
        name = tesseract_region(image, [300, 320, 1200, 110], tesseract_song_name)
        artist = tesseract_region(image, [300, 420, 1200, 40])
        score = tesseract_region(image, [850, 675, 470, 120], num=True)
        max_recall = tesseract_region(
            image, [380, 590, 130, 40], num=True, hard_to_read=False, mode=13
        )
        difficulty = tesseract_region(
            image,
            [150, 540, 200, 50],
            mode=7,
            extra=tesseract_difficulty,
        )
        difficulty, _ = closest_string(difficulty, all_difficulties)
        difficulty = {
            "PAST": "PST",
            "PRESENT": "PRS",
            "FUTURE": "FTR",
            "ETERNAL": "ETR",
            "BEYOND": "BYD",
        }[difficulty]
        return (name, score, difficulty, artist, max_recall)


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

        budget = len(chart["title"]) / 3

        if distance < budget:
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
            SELECT s.id, s.score, c.id, c.title, c.difficulty, c.level, c.note_count, c.chart_constant, c.artist
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
