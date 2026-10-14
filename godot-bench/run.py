#!/usr/bin/env python3
"""Exports the two Forward+ benchmark projects as release builds and compares them.

    godot-bench/run.py [--godot <editor binary>] [--runs 3] [--reps 1] [--frames 600] [--no-export]

Each project (official/, ours/) is exported with the release export template for the system it runs on
(signed ad hoc on macOS) and run once per scene. The two builds alternate and nothing runs in parallel. The whole pass is
repeated --runs times, one table per pass, followed by a conclusion table with the median and the
range over the passes. A run is rejected unless the game reports the Forward+ renderer and a
release build.
Run `setup.sh` first, and close other applications: the numbers depend on machine load.
"""
import argparse
import os
import platform
import re
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
BUILD = HERE / "build"
PROJECTS = ("official", "ours")
# asset dir, skeleton, atlas, animation, skeleton count, sprite scale
CASES = [
    ("spineboy", "spineboy-pro.skel", "spineboy.atlas", "run", 0, 0.08),  # empty scene: the floor
    ("spineboy", "spineboy-pro.skel", "spineboy.atlas", "run", 100, 0.08),
    ("spineboy", "spineboy-pro.skel", "spineboy.atlas", "run", 500, 0.04),
    ("spineboy", "spineboy-pro.skel", "spineboy.atlas", "run", 1000, 0.03),
    ("coin", "coin-pro.skel", "coin.atlas", "animation", 200, 0.08),
    ("alien", "alien-pro.skel", "alien.atlas", "run", 200, 0.08),
    ("raptor", "raptor-pro.skel", "raptor.atlas", "walk", 200, 0.06),
    ("tank", "tank-pro.skel", "tank.atlas", "drive", 100, 0.04),
]
RESULT = re.compile(r"RESULT (.*)")
# Some runs report exactly 10.00 ms per frame: a throttled frame loop, not a measurement. They are
# rejected and the scene is run again, up to ATTEMPTS times.
THROTTLED_MS = 10.0
ATTEMPTS = 4


SYSTEM = platform.system()  # "Darwin" (macOS), "Linux" or "Windows"
PRESET = {"Darwin": "macOS", "Linux": "Linux", "Windows": "Windows Desktop"}[SYSTEM]


def default_godot() -> str:
    """The GODOT environment variable, else `godot` on PATH, else the usual macOS location."""
    return os.environ.get("GODOT") or shutil.which("godot") or "/Applications/Godot.app/Contents/MacOS/Godot"


def export_path(project: str) -> Path:
    """Where the exported game goes: an app bundle on macOS, a single executable elsewhere."""
    name = {"Darwin": "bench.app", "Windows": "bench.exe"}.get(SYSTEM, "bench")
    return BUILD / project / name


def app_binary(project: str) -> Path:
    path = export_path(project)
    return path / "Contents" / "MacOS" / f"bench-{project}" if SYSTEM == "Darwin" else path


def export(godot: str, project: str) -> None:
    """Imports the project and exports a release build (signed ad hoc on macOS)."""
    path = HERE / project
    app = export_path(project)
    app.parent.mkdir(parents=True, exist_ok=True)
    # --quit-after avoids the exit crash of `--import --quit` seen with gdext extensions.
    subprocess.run([godot, "--headless", "--editor", "--path", str(path), "--quit-after", "180"],
                   check=False, capture_output=True, timeout=600)
    subprocess.run([godot, "--headless", "--path", str(path), "--export-release", PRESET, str(app)],
                   check=True, capture_output=True, timeout=900)
    if SYSTEM == "Darwin":
        subprocess.run(["codesign", "--force", "--deep", "--sign", "-", str(app)], check=True)
    elif SYSTEM == "Linux":
        app.chmod(0o755)


def run_once(project: str, case: tuple, frames: int) -> dict | None:
    command = [str(app_binary(project)), "--", case[0], case[1], case[2], case[3], str(case[4]),
               str(frames), str(case[5])]
    try:
        output = subprocess.run(command, capture_output=True, text=True, timeout=300).stdout
    except subprocess.TimeoutExpired:
        print(f"{project} {case[0]} x{case[4]}: timed out", file=sys.stderr)
        return None
    match = RESULT.search(output)
    if not match:
        print(f"{project} {case[0]} x{case[4]}: no result\n{output[-400:]}", file=sys.stderr)
        return None
    values = dict(pair.split("=", 1) for pair in match[1].split())
    if values["renderer"] != "forward_plus" or values["build"] != "release":
        print(f"{project}: refusing {values['renderer']} / {values['build']}", file=sys.stderr)
        return None
    if values.get("focused") == "false" or abs(float(values["mean_ms"]) - THROTTLED_MS) < 0.02:
        print(f"{project} {case[0]} x{case[4]}: rejected a throttled run ({values['mean_ms']} ms, "
              f"focused={values.get('focused')})", file=sys.stderr)
        return None
    return {key: float(value) if re.fullmatch(r"[\d.]+", value) else value
            for key, value in values.items()}


def label(case: tuple) -> str:
    return "empty scene" if case[4] == 0 else f"{case[0]} x{case[4]}"


def run_table(args: argparse.Namespace, order: tuple) -> dict:
    """Runs every scene once in `order` and prints one table; returns the results per scene."""
    print("| Scene | Frame ms (official / ours) | FPS (official / ours) | p99 ms (official / ours) "
          "| Draw calls (official / ours) | Speedup |")
    print("|---|---|---|---|---|---|")
    results = {}
    for case in CASES:
        best = {}
        for project in order:
            runs = []
            for _ in range(ATTEMPTS * args.reps):
                result = run_once(project, case, args.frames)
                if result:
                    runs.append(result)
                if len(runs) == args.reps:
                    break
            if runs:
                best[project] = min(runs, key=lambda r: r["mean_ms"])
        if len(best) < len(PROJECTS):
            continue
        o, w = best["official"], best["ours"]
        results[label(case)] = (o, w)
        print(f"| {label(case)} | {o['mean_ms']:.2f} / {w['mean_ms']:.2f} "
              f"| {o['fps']:.0f} / {w['fps']:.0f} | {o['p99_ms']:.2f} / {w['p99_ms']:.2f} "
              f"| {o['draw_calls']:.0f} / {w['draw_calls']:.0f} | {o['mean_ms'] / w['mean_ms']:.2f}x |",
              flush=True)
    return results


def conclusion(tables: list) -> None:
    """Prints the median and the range of the mean frame time over all tables."""
    print("| Scene | Official ms (median, min-max) | Ours ms (median, min-max) "
          "| Speedup of the medians | Speedup range |")
    print("|---|---|---|---|---|")
    for name in tables[0]:
        if not all(name in table for table in tables):
            continue
        official = sorted(table[name][0]["mean_ms"] for table in tables)
        ours = sorted(table[name][1]["mean_ms"] for table in tables)
        # Each ratio comes from the two builds of the same pass, never from different passes.
        ratios = sorted(table[name][0]["mean_ms"] / table[name][1]["mean_ms"] for table in tables)
        mid = len(tables) // 2
        print(f"| {name} | {official[mid]:.2f} ({official[0]:.2f}-{official[-1]:.2f}) "
              f"| {ours[mid]:.2f} ({ours[0]:.2f}-{ours[-1]:.2f}) "
              f"| {official[mid] / ours[mid]:.2f}x | {ratios[0]:.2f}x-{ratios[-1]:.2f}x |")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--godot", default=default_godot())
    parser.add_argument("--runs", type=int, default=3, help="number of full passes, one table each")
    parser.add_argument("--reps", type=int, default=1, help="runs per cell inside one pass (best kept)")
    parser.add_argument("--frames", type=int, default=600)
    parser.add_argument("--no-export", action="store_true", help="reuse the apps from the last export")
    args = parser.parse_args()
    if not args.no_export:
        for project in PROJECTS:
            export(args.godot, project)

    tables = []
    for number in range(args.runs):
        # Alternate which plugin goes first so neither always runs on a cooler machine.
        order = PROJECTS if number % 2 == 0 else tuple(reversed(PROJECTS))
        print(f"\nRun {number + 1} of {args.runs} (first: {order[0]})\n", flush=True)
        tables.append(run_table(args, order))
    print("\nConclusion\n")
    conclusion(tables)


if __name__ == "__main__":
    main()
