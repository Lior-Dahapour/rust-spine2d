#!/usr/bin/env python3
"""Runs the same benchmark scene against this extension and the official spine-godot extension.

    compare.py --official <godot project> --ours <godot project> [--godot <binary>] [--reps 3]

Each project must already contain its extension (they define the same class names, so they cannot
share a project) and the example assets under res://assets/<name>/ (spineboy, coin, alien, raptor,
tank, exported as .skel with their .atlas and .png). This script copies bench_cmp.gd/.tscn into both
projects. Runs alternate between the plugins, nothing runs in parallel, and each cell reports the
best of --reps runs. Close other applications first: the numbers are sensitive to machine load.
"""
import argparse
import os
import re
import shutil
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent


def default_godot():
    """The GODOT environment variable, else `godot` on PATH, else the usual macOS location."""
    return os.environ.get("GODOT") or shutil.which("godot") or "/Applications/Godot.app/Contents/MacOS/Godot"
# directory, skeleton, atlas, animation, skeleton count, sprite scale
CASES = [
    ("spineboy", "spineboy-pro.skel", "spineboy.atlas", "run", 100, 0.08),
    ("spineboy", "spineboy-pro.skel", "spineboy.atlas", "run", 500, 0.04),
    ("spineboy", "spineboy-pro.skel", "spineboy.atlas", "run", 1000, 0.03),
    ("coin", "coin-pro.skel", "coin.atlas", "animation", 200, 0.08),
    ("alien", "alien-pro.skel", "alien.atlas", "run", 200, 0.08),
    ("raptor", "raptor-pro.skel", "raptor.atlas", "walk", 200, 0.06),
    ("tank", "tank-pro.skel", "tank.atlas", "drive", 100, 0.04),
]
RESULT = re.compile(
    r"RESULT (\w+) count=(\d+) \| update_skeleton=([\d.]+) us/skeleton \| whole frame=([\d.]+) us "
    r"\(([\d.]+) us/skeleton\) \| draw_calls=(\d+)"
)


def run(godot, project, case, frames):
    command = [
        "perl", "-e", "alarm 300; exec @ARGV", godot, "--path", str(project), "--resolution", "900x700",
        "--scene", "res://bench_cmp.tscn", "--", case[0], case[1], case[2], case[3], str(case[4]), str(frames), str(case[5]),
    ]
    output = subprocess.run(command, capture_output=True, text=True, cwd=project).stdout
    match = RESULT.search(output)
    if not match:
        print("could not parse the result of", project, case, "\n", output[-400:])
        return None
    return {"update": float(match[3]), "frame": float(match[4]), "calls": int(match[6])}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--official", required=True, type=Path)
    parser.add_argument("--ours", required=True, type=Path)
    parser.add_argument("--godot", default=default_godot())
    parser.add_argument("--reps", type=int, default=3)
    parser.add_argument("--frames", type=int, default=250)
    args = parser.parse_args()
    for project in (args.official, args.ours):
        for name in ("bench_cmp.gd", "bench_cmp.tscn"):
            shutil.copy(HERE / name, project / name)

    print("| Scene | Update call us/skeleton (official / ours) | Whole frame ms (official / ours) | Draw calls (official / ours) |")
    print("|---|---|---|---|")
    for case in CASES:
        results = {"official": [], "ours": []}
        for _ in range(args.reps):
            for name, project in (("official", args.official), ("ours", args.ours)):
                result = run(args.godot, project, case, args.frames)
                if result:
                    results[name].append(result)
        if not all(results.values()):
            continue
        official = min(results["official"], key=lambda r: r["frame"])
        ours = min(results["ours"], key=lambda r: r["frame"])
        update_official = min(r["update"] for r in results["official"])
        update_ours = min(r["update"] for r in results["ours"])
        print(
            f"| {case[0]} x{case[4]} | {update_official:.1f} / {update_ours:.1f} | "
            f"{official['frame'] / 1000:.1f} / {ours['frame'] / 1000:.1f} | {official['calls']} / {ours['calls']} |",
            flush=True,
        )


if __name__ == "__main__":
    main()
