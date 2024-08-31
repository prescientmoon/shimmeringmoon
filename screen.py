#!/usr/bin/env nix-shell
#!nix-shell -p "pkgs.python3.withPackages (p: with p; [numpy matplotlib])"
#!nix-shell -i python3
import numpy as np
import matplotlib.pyplot as plt

raw_training_data = [
    (1600, 720, 65),
    (2778, 1284, 115),
    (2532, 1170, 105),
    (2000, 1200, 140),
    (2560, 1600, 210),
    (2160, 1620, 312),
    (2224, 1668, 321),
]

raw_test_data = [
    (2160, 1620, 312),
    (2732, 2048, 395),
    (2560, 1600, 210),
    (1560, 720, 65),
]


def process_raw_data(raw):
    data = [
        {"width": width, "height": height, "score_box_y": sb_y}
        for (width, height, sb_y) in raw
    ]

    for entry in data:
        entry["aspect_ratio"] = entry["width"] / entry["height"]
        entry["score_box_y_ratio"] = entry["score_box_y"] / entry["height"]

    data = sorted(data, key=lambda x: x["aspect_ratio"])
    return data


training_data = process_raw_data(raw_training_data)
test_data = process_raw_data(raw_test_data)

print(test_data)


def manual_fit(aspect_ratio, data=training_data):
    for i in range(len(data) - 1):
        curr_ar = data[i]["aspect_ratio"]
        next_ar = data[i + 1]["aspect_ratio"]
        if curr_ar == next_ar:
            continue

        if (
            curr_ar <= aspect_ratio <= next_ar
            or (i == 0 and aspect_ratio < curr_ar)
            or (i == len(data) - 2 and next_ar < aspect_ratio)
        ):
            p = (aspect_ratio - curr_ar) / (next_ar - curr_ar)
            curr_sbyr = data[i]["score_box_y_ratio"]
            next_sbyr = data[i + 1]["score_box_y_ratio"]
            return curr_sbyr + (next_sbyr - curr_sbyr) * p

    return None


rx = np.array([e["aspect_ratio"] for e in training_data])
ry = np.array([e["score_box_y_ratio"] for e in training_data])
mry = np.array([manual_fit(ar) for ar in rx])

plt.plot(rx, ry, linestyle="-")
plt.scatter(rx, ry, label="Real data (training)")
plt.plot(rx, mry, linestyle="--", label="Segmented fit (training)")

tx = np.array([e["aspect_ratio"] for e in test_data])
ty = np.array([e["score_box_y_ratio"] for e in test_data])
mty = np.array([manual_fit(ar) for ar in tx])
plt.plot(tx, ty, linestyle="-")
plt.scatter(tx, ty, label="Real data (test)")
plt.plot(tx, mty, linestyle="--", label="Segmented fit (test)")

print(ty)
print(mty)

plt.legend()
plt.show()
