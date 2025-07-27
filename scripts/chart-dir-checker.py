import glob
import json
import os
import sys

# {{{ Validate input vars
chart_dir = os.environ.get("SHIMMERING_PRIVATE_CHART_DIR")
if chart_dir is None:
    print("The SHIMMERING_PRIVATE_CHART_DIR environment variable is not set.")
    sys.exit(1)

private_config = os.environ.get("SHIMMERING_PRIVATE_CONFIG_DIR")
if private_config is None:
    print("The SHIMMERING_PRIVATE_CONFIG_DIR environment variable is not set.")
    sys.exit(1)
# }}}
# {{{ Find charts
charts = []
pattern = os.path.join(chart_dir, "*", "*.aff")
for filepath in glob.glob(pattern):
    parts = os.path.normpath(filepath).split(os.sep)
    if len(parts) >= 3:
        charts.append((parts[-2], int(parts[-1].removesuffix(".aff"))))
# }}}
# {{{ Find expected charts based on the songlist file
songlist_path = os.path.join(private_config, "songlist.json")

with open(songlist_path, "r", encoding="utf-8") as f:
    songlist = json.load(f)

expected_charts = []
for s in songlist["songs"]:
    if "deleted" in s and s["deleted"]:
        continue

    if "remote_dl" not in s or not s["remote_dl"]:
        continue

    for d in s["difficulties"]:
        expected_charts.append((s["id"], d["ratingClass"]))
# }}}
# {{{ Print diff & delete entries
charts = set(charts)
expected_charts = set(expected_charts)

missing_charts = sorted(list(expected_charts - charts))
if missing_charts:
    for e in missing_charts:
        print(f"Missing chart {e}")
else:
    print("No charts are missing.")

# Some remote dl / deleted charts are still there fsr, oh well...
extra_charts = sorted(list(charts - expected_charts))
if extra_charts:
    for e in extra_charts:
        print(f"Unexpected chart {e}")
else:
    print("No additional charts found.")
# }}}
