#!/usr/bin/env python3
"""Generates the synthetic clipping skeletons in this folder (run from this folder)."""
import json, math

SQUARE_CW = [-60, 60, 60, 60, 60, -60, -60, -60]            # clockwise (y up)
SQUARE_CCW = [-60, -60, 60, -60, 60, 60, -60, 60]
CONCAVE = [-90, 90, 0, 20, 90, 90, 90, -90, -90, -90]        # a notch in the top edge
STAR = [0, 100, 25, 30, 95, 30, 40, -10, 60, -80, 0, -35, -60, -80, -40, -10, -95, 30, -25, 30]
TRI = [-80, -70, 0, 90, 80, -70]
BIG = [-300, 300, 300, 300, 300, -300, -300, -300]
OFF = [250, 250, 320, 250, 320, 320, 250, 320]               # nowhere near the content

def skeleton(clips, two_color=False, spin_at=(20, 10)):
    """clips: list of (name, vertices, convex, inverse, end_slot_index_offset)."""
    slots, atts, anims = [], {}, {}
    bones = [{"name": "root"}, {"name": "spin", "parent": "root", "x": spin_at[0], "y": spin_at[1]}]
    # draw order: [clip..., body, mesh, ...] with each clip ending after the mesh slot
    mesh_slot = "mesh"
    for name, verts, convex, inverse, _ in clips:
        slots.append({"name": name, "bone": "spin", "attachment": name})
        a = {"type": "clipping", "end": mesh_slot, "vertexCount": len(verts) // 2, "vertices": verts}
        if convex: a["convex"] = True
        if inverse: a["inverse"] = True
        atts[name] = {name: a}
    body = {"name": "body", "bone": "root", "attachment": "r1", "color": "ffcc99ff"}
    if two_color: body["dark"] = "336699"
    slots.append(body)
    atts["body"] = {"r1": {"type": "region", "name": "r1", "x": 10, "y": 0, "width": 220, "height": 160, "rotation": 15}}
    slots.append({"name": "mesh", "bone": "root", "attachment": "m1", "blend": "additive", "color": "88ff88c0"})
    atts["mesh"] = {"m1": {"type": "mesh", "name": "r2", "uvs": [0, 0, 1, 0, 1, 1, 0, 1, .5, .5],
                           "triangles": [0, 1, 4, 1, 2, 4, 2, 3, 4, 3, 0, 4],
                           "vertices": [-110, -90, 110, -90, 110, 90, -110, 90, 0, 0], "hull": 4}}
    slots.append({"name": "box", "bone": "spin", "attachment": "bb"})
    atts["box"] = {"bb": {"type": "boundingbox", "vertexCount": 4, "vertices": [-40, -40, 40, -40, 40, 40, -40, 40]}}
    anims["spin"] = {"bones": {"spin": {"rotate": [{"value": 0}, {"time": 1, "value": 120}, {"time": 2, "value": 360}]}}}
    return {"skeleton": {"spine": "4.3.0"}, "bones": bones, "slots": slots,
            "skins": [{"name": "default", "attachments": atts}], "animations": anims}

variants = {
    "clip-cw-square": [("c", SQUARE_CW, False, False, 0)],
    "clip-ccw-square": [("c", SQUARE_CCW, False, False, 0)],
    "clip-concave": [("c", CONCAVE, False, False, 0)],
    "clip-concave-convexflag": [("c", CONCAVE, True, False, 0)],
    "clip-star": [("c", STAR, False, False, 0)],
    "clip-triangle": [("c", TRI, False, False, 0)],
    "clip-inverse-square": [("c", SQUARE_CW, False, True, 0)],
    "clip-inverse-concave": [("c", CONCAVE, False, True, 0)],
    "clip-inverse-star": [("c", STAR, False, True, 0)],
    "clip-covers-all": [("c", BIG, False, False, 0)],
    "clip-misses-all": [("c", OFF, False, False, 0)],
    "clip-nested-restart": [("c1", SQUARE_CW, False, False, 0), ("c2", TRI, False, False, 0)],
}
# Everything (including the bounding box) in negative space: exercises the FLT_MIN quirk in
# SkeletonBounds, whose maxima start at the smallest positive float.
json.dump(skeleton([("c", SQUARE_CW, False, False, 0)], False, (-500, -500)), open("bounds-negative.json", "w"), indent=1)
for name, clips in variants.items():
    for tc in (False, True):
        out = name + ("-twocolor" if tc else "")
        json.dump(skeleton(clips, tc), open(out + ".json", "w"), indent=1)
open("clip.atlas", "w").write("clip.png\n\tsize: 128, 128\n\tfilter: Linear, Linear\nr1\n\tbounds: 0, 0, 64, 64\nr2\n\tbounds: 64, 0, 64, 64\n")
