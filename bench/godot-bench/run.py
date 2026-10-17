#!/usr/bin/env python3
"""Exports the Forward+ benchmark projects as release builds and compares them.

    bench/godot-bench/run.py [--projects official ours] [--godot <editor binary>] [--runs 3]
                             [--reps 1] [--frames 600] [--no-export]

Each project (official/, ours/) is exported with the release export template for the system it runs on
(signed ad hoc on macOS) and run once per scene. The two builds alternate and nothing runs in parallel. The first
project is the baseline the other is compared with: the official plugin against ours. The whole pass is
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
ALL_PROJECTS = ("official", "ours")
# asset dir, skeleton, atlas, animation, skeleton count, sprite scale
CASES = [
    ("spineboy", "spineboy-pro.skel", "spineboy.atlas", "run", 0, 0.08),  # empty scene: the floor
    ("spineboy", "spineboy-pro.skel", "spineboy.atlas", "run", 100, 0.08),
    ("spineboy", "spineboy-pro.skel", "spineboy.atlas", "run", 500, 0.04),
    ("spineboy", "spineboy-pro.skel", "spineboy.atlas", "run", 1000, 0.03),
    ("spineboy", "spineboy-pro.skel", "spineboy.atlas", "run", 5000, 0.02),
    ("spineboy", "spineboy-pro.skel", "spineboy.atlas", "run", 10000, 0.012),
    ("spineboy", "spineboy-pro.skel", "spineboy.atlas", "churn", 3000, 0.025),  # scripted changes
    ("mixed", "-", "-", "churn", 3000, 0.025),  # five skeleton kinds, scripted changes
    ("mixed", "-", "-", "constraints", 300, 0.06),  # five kinds, constraint access every frame
    ("spineboy", "spineboy-pro.skel", "spineboy.atlas", "materials", 500, 0.04),  # materials set and cleared
    ("mix-and-match", "mix-and-match-pro.skel", "mix-and-match.atlas", "skins", 300, 0.04),  # new skins
    ("spineboy", "spineboy-pro.skel", "spineboy.atlas", "nodes", 300, 0.04),  # bone and slot nodes
    ("spineboy", "spineboy-pro.skel", "spineboy.atlas", "events", 1000, 0.03),  # event signals
    ("coin", "coin-pro.skel", "coin.atlas", "animation", 200, 0.08),
    ("alien", "alien-pro.skel", "alien.atlas", "run", 200, 0.08),
    ("raptor", "raptor-pro.skel", "raptor.atlas", "walk", 200, 0.06),
    ("tank", "tank-pro.skel", "tank.atlas", "drive", 100, 0.04),
]
RESULT = re.compile(r"RESULT (.*)")
# `/usr/bin/time -l` reports the peak physical memory footprint of the process, in bytes.
PEAK_MEMORY = re.compile(r"(\d+)\s+peak memory footprint")
# Some runs report exactly 10.00 ms per frame: a throttled frame loop, not a measurement. They are
# rejected and the scene is run again, up to ATTEMPTS times.
THROTTLED_MS = 10.0
THROTTLED_TOLERANCE = 0.15
# The game cuts a scene short when it averages more than this per frame (see SLOW_MS in bench.gd);
# the table then shows "> 100" instead of a time.
SLOW_MS = 100.0
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
    command = ["/usr/bin/time", "-l", str(app_binary(project)), "--", case[0], case[1], case[2], case[3], str(case[4]),
               str(frames), str(case[5])]
    try:
        finished = subprocess.run(command, capture_output=True, text=True, timeout=300)
        output = finished.stdout
    except subprocess.TimeoutExpired:
        print(f"{project} {case[0]} x{case[4]}: timed out", file=sys.stderr)
        return None
    match = RESULT.search(output)
    if not match and finished.returncode != 0:
        print(f"{project} {case[0]} {case[3]} x{case[4]}: crashed (exit code {finished.returncode})",
              file=sys.stderr)
        return {"crashed": 1.0, "mean_ms": float("inf")}
    if not match:
        print(f"{project} {case[0]} x{case[4]}: no result\n{output[-400:]}", file=sys.stderr)
        return None
    values = dict(pair.split("=", 1) for pair in match[1].split())
    peak = PEAK_MEMORY.search(finished.stderr)
    values["peak_mb"] = str(int(peak[1]) / 2**20) if peak else "0"
    if values.get("check") == "0":
        print(f"{project} {case[0]} {case[3]} x{case[4]}: the scene did not do its work (check failed)",
              file=sys.stderr)
        return None
    if values["renderer"] != "forward_plus" or values["build"] != "release":
        print(f"{project}: refusing {values['renderer']} / {values['build']}", file=sys.stderr)
        return None
    if values.get("focused") == "false" or abs(float(values["mean_ms"]) - THROTTLED_MS) < THROTTLED_TOLERANCE:
        print(f"{project} {case[0]} x{case[4]}: rejected a throttled run ({values['mean_ms']} ms, "
              f"focused={values.get('focused')})", file=sys.stderr)
        return None
    return {key: float(value) if re.fullmatch(r"[\d.]+", value) else value
            for key, value in values.items()}


def selected_cases(args: argparse.Namespace) -> list:
    """The scenes to run: all of them, or the ones whose label contains one of `--only`."""
    cases = [case for case in CASES if label(case) not in args.skip]
    if not args.only:
        return cases
    return [case for case in cases if any(text in label(case) for text in args.only)]


def label(case: tuple) -> str:
    if case[3] in ("churn", "constraints", "materials", "skins", "nodes", "events"):
        return f"{case[0]} {case[3]} x{case[4]}"
    return "empty scene" if case[4] == 0 else f"{case[0]} x{case[4]}"


def cells(row: tuple, key: str, fmt: str) -> str:
    """The projects' values in one table cell, in project order."""
    return " / ".join(timed(result, key, fmt) for result in row)


def is_slow(result: dict) -> bool:
    return result.get("slow") == 1.0


def crashed(result: dict) -> bool:
    return result.get("crashed") == 1.0


def timed(result: dict, key: str, fmt: str) -> str:
    """A measured value, or the bound shown for a scene that was cut short as too slow."""
    if crashed(result):
        return "crashed"
    if not is_slow(result):
        return format(result[key], fmt)
    return {"mean_ms": f"> {SLOW_MS:.0f}", "fps": f"< {1000 / SLOW_MS:.0f}"}.get(key, "-")


def speedups(row: tuple) -> str:
    """How many times faster each project is than the first one, as table cells."""
    cells_ = []
    base = row[0]
    for other in row[1:]:
        if crashed(base) or crashed(other):
            cells_.append("-")
        elif is_slow(base) and is_slow(other):
            cells_.append("-")
        elif is_slow(base):
            cells_.append(f"> {SLOW_MS / other['mean_ms']:.1f}x")
        elif is_slow(other):
            cells_.append(f"< {base['mean_ms'] / SLOW_MS:.2f}x")
        else:
            cells_.append(f"{base['mean_ms'] / other['mean_ms']:.2f}x")
    return " | ".join(cells_)


def header(projects: tuple, first: str) -> None:
    names = " / ".join(projects)
    vs = " | ".join(f"{name} vs {projects[0]}" for name in projects[1:])
    print(f"| {first} | Frame ms ({names}) | FPS | p99 ms | Draw calls | Peak MB | {vs} |")
    print("|---|---|---|---|---|---|" + "---|" * (len(projects) - 1))


def run_table(args: argparse.Namespace, order: tuple) -> dict:
    """Runs every scene once in `order` and prints one table; returns the results per scene."""
    projects = tuple(args.projects)
    header(projects, "Scene")
    results = {}
    for case in selected_cases(args):
        best = {}
        for project in order:
            runs = []
            for _ in range(ATTEMPTS * args.reps):
                result = run_once(project, case, args.frames)
                if result:
                    runs.append(result)
                if result and result.get("crashed"):
                    break
                if len(runs) == args.reps:
                    break
            if runs:
                best[project] = min(runs, key=lambda r: r["mean_ms"])
        if len(best) < len(projects):
            continue
        row = tuple(best[project] for project in projects)
        results[label(case)] = row
        print(f"| {label(case)} | {cells(row, 'mean_ms', '.2f')} "
              f"| {cells(row, 'fps', '.0f')} "
              f"| {cells(row, 'p99_ms', '.2f')} "
              f"| {cells(row, 'draw_calls', '.0f')} "
              f"| {cells(row, 'peak_mb', '.0f')} "
              f"| {speedups(row)} |", flush=True)
    return results


def conclusion(args: argparse.Namespace, tables: list) -> None:
    """Prints the median of the mean frame time over all tables, with the range."""
    projects = tuple(args.projects)
    vs = " | ".join(f"{name} vs {projects[0]}" for name in projects[1:])
    print("| Scene | " + " | ".join(f"{name} ms (median, min-max)" for name in projects)
          + f" | {vs} |")
    print("|---|" + "---|" * len(projects) + "---|" * (len(projects) - 1))
    mid = len(tables) // 2
    for name in tables[0]:
        if not all(name in table for table in tables):
            continue
        spans = [sorted(table[name][index]["mean_ms"] for table in tables)
                 for index in range(len(projects))]
        medians = tuple({"mean_ms": span[mid]} for span in spans)
        text = " | ".join(f"{span[mid]:.2f} ({span[0]:.2f}-{span[-1]:.2f})" for span in spans)
        print(f"| {name} | {text} | {speedups(medians)} |")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--projects", nargs="+", choices=ALL_PROJECTS, default=list(ALL_PROJECTS),
                        help="the projects to compare; the first is the baseline")
    parser.add_argument("--only", nargs="+", default=[],
                        help="run only the scenes whose label contains one of these texts")
    parser.add_argument("--skip", nargs="+", default=[],
                        help="leave out the scenes with exactly these labels, for example scenes "
                        "the slowest project cannot finish")
    parser.add_argument("--godot", default=default_godot())
    parser.add_argument("--runs", type=int, default=3, help="number of full passes, one table each")
    parser.add_argument("--reps", type=int, default=1, help="runs per cell inside one pass (best kept)")
    parser.add_argument("--frames", type=int, default=600)
    parser.add_argument("--no-export", action="store_true", help="reuse the apps from the last export")
    args = parser.parse_args()
    if not args.no_export:
        for project in args.projects:
            export(args.godot, project)

    tables = []
    for number in range(args.runs):
        # Alternate which plugin goes first so neither always runs on a cooler machine.
        projects = tuple(args.projects)
        order = projects if number % 2 == 0 else tuple(reversed(projects))
        print(f"\nRun {number + 1} of {args.runs} (first: {order[0]})\n", flush=True)
        tables.append(run_table(args, order))
    if args.runs > 1:
        print("\nConclusion\n")
        conclusion(args, tables)


if __name__ == "__main__":
    main()
