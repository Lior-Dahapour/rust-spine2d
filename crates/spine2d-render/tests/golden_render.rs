//! Compares render commands, bounds and hit tests against the C++ reference runtime.
//!
//! Needs `SPINE_RUNTIMES_DIR` and `SPINE_CPP_RENDER_DRIVER` (see `tests/cpp/build.sh`).
//! `GOLDEN_ALL=1` runs every example; `GOLDEN_SKELETON=substr` narrows to matching files.

#[allow(dead_code)]
#[path = "../../spine2d-core/tests/common/mod.rs"]
mod common;
use common::*;
use serde_json::Value;
use spine2d_core::*;
use spine2d_render::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

const DT: f32 = 1.0 / 60.0;
const FRAMES: usize = 40;

fn f(v: f32) -> String {
    if v.is_nan() {
        "NaN".into()
    } else {
        format!("{v:.6}")
    }
}

fn floats(v: &[f32]) -> Vec<String> {
    v.iter().map(|x| f(*x)).collect()
}

fn jf(x: &Value) -> String {
    match x.as_str() {
        Some("NaN") => "NaN".into(),
        Some(s) => s.into(),
        None => format!("{:.6}", x.as_f64().unwrap()),
    }
}

fn jfloats(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(jf).collect()
}

fn jints<T: std::str::FromStr>(v: &Value) -> Vec<T> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.to_string().parse().ok().unwrap())
        .collect()
}

fn parse(text: &str) -> Vec<Value> {
    text.split("=== ANIMATION FRAME")
        .skip(1)
        .map(|b| serde_json::from_str(b.split_once("===\n").unwrap().1.trim()).unwrap())
        .collect()
}

fn rect(v: &Value) -> Vec<String> {
    ["x", "y", "w", "h"].iter().map(|k| jf(&v[k])).collect()
}

fn hit(h: Option<&Arc<spine2d_core::attachment::Attachment>>) -> String {
    h.map_or("null".to_string(), |a| format!("\"{}\"", a.name()))
}

/// Merges `render_batches` output by hand, the way a backend writing its own buffers would.
fn merge_groups(renderer: &mut SkeletonRenderer, sk: &Skeleton) -> Vec<RenderCommand> {
    let batches = renderer.render_batches(&sk.borrow_ref());
    batches
        .groups
        .iter()
        .map(|g| {
            let mut out = RenderCommand {
                blend_mode: g.blend_mode,
                texture: g.texture,
                ..Default::default()
            };
            let mut offset = 0u16;
            for c in batches.group_commands(g) {
                out.positions.extend_from_slice(&c.positions);
                out.uvs.extend_from_slice(&c.uvs);
                out.colors.extend_from_slice(&c.colors);
                out.dark_colors.extend_from_slice(&c.dark_colors);
                out.indices
                    .extend(c.indices.iter().map(|&ix| ix.wrapping_add(offset)));
                offset = offset.wrapping_add(c.vertex_count() as u16);
            }
            assert_eq!(
                (out.vertex_count(), out.indices.len()),
                (g.vertex_count, g.index_count)
            );
            assert_eq!(
                (
                    out.colors.first().copied().unwrap_or(0),
                    out.dark_colors.first().copied().unwrap_or(0)
                ),
                (g.color, g.dark_color)
            );
            out
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn check_frame(
    label: &str,
    i: usize,
    sk: &Skeleton,
    renderer: &mut SkeletonRenderer,
    clipper: &mut SkeletonClipping,
    bounds: &mut SkeletonBounds,
    cpp: &Value,
    errors: &mut Vec<String>,
) {
    let mut err = |what: &str, a: String, b: String| {
        if a != b && errors.len() < 200 {
            errors.push(format!(
                "{label} frame {i}: {what}\n  rust: {}\n  cpp:  {}",
                &a[..a.len().min(300)],
                &b[..b.len().min(300)]
            ));
        }
    };
    let grouped = merge_groups(renderer, sk);
    let cmds = renderer.render(&sk.borrow_ref());
    err(
        "groups merge to render()",
        format!("{grouped:?}"),
        format!("{cmds:?}"),
    );
    let c = cpp["commands"].as_array().unwrap();
    err("command count", cmds.len().to_string(), c.len().to_string());
    for (n, (r, x)) in cmds.iter().zip(c).enumerate() {
        err(
            &format!("cmd {n} blend"),
            (r.blend_mode as i32).to_string(),
            x["blend"].to_string(),
        );
        err(
            &format!("cmd {n} texture"),
            r.texture.to_string(),
            x["texture"].to_string(),
        );
        err(
            &format!("cmd {n} positions"),
            floats(&r.positions).join(","),
            jfloats(&x["positions"]).join(","),
        );
        err(
            &format!("cmd {n} uvs"),
            floats(&r.uvs).join(","),
            jfloats(&x["uvs"]).join(","),
        );
        err(
            &format!("cmd {n} colors"),
            format!("{:?}", r.colors),
            format!("{:?}", jints::<u32>(&x["colors"])),
        );
        err(
            &format!("cmd {n} dark"),
            format!("{:?}", r.dark_colors),
            format!("{:?}", jints::<u32>(&x["dark"])),
        );
        err(
            &format!("cmd {n} indices"),
            format!("{:?}", r.indices),
            format!("{:?}", jints::<u16>(&x["indices"])),
        );
    }
    let b = compute_bounds(sk, &mut Vec::new(), None);
    err(
        "bounds",
        floats(&[b.x, b.y, b.width, b.height]).join(","),
        rect(&cpp["bounds"]).join(","),
    );
    let b = compute_bounds(sk, &mut Vec::new(), Some(clipper));
    err(
        "clipped bounds",
        floats(&[b.x, b.y, b.width, b.height]).join(","),
        rect(&cpp["clippedBounds"]).join(","),
    );

    bounds.update(sk, true);
    err(
        "box count",
        bounds.bounding_boxes().len().to_string(),
        cpp["boxes"].to_string(),
    );
    let a = &cpp["aabb"];
    err(
        "aabb",
        floats(&[bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y]).join(","),
        ["minX", "minY", "maxX", "maxY"]
            .iter()
            .map(|k| jf(&a[k]))
            .collect::<Vec<_>>()
            .join(","),
    );
    if !bounds.bounding_boxes().is_empty() {
        let mut pts = Vec::new();
        for gx in 0..5 {
            for gy in 0..5 {
                let px = bounds.min_x + (bounds.max_x - bounds.min_x) * (gx as f32 + 0.5) / 5.0;
                let py = bounds.min_y + (bounds.max_y - bounds.min_y) * (gy as f32 + 0.5) / 5.0;
                pts.push(format!(
                    "{}/{}",
                    bounds.aabb_contains_point(px, py) as i32,
                    hit(bounds.contains_point(px, py))
                ));
            }
        }
        let cp: Vec<String> = cpp["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| format!("{}/{}", p["in"], p["hit"]))
            .collect();
        err("points", pts.join(" "), cp.join(" "));
        let (x0, y0, x1, y1) = (bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y);
        let segs = [
            [x0 - 10.0, y0 - 10.0, x1 + 10.0, y1 + 10.0],
            [x0 - 10.0, y1 + 10.0, x1 + 10.0, y0 - 10.0],
            [x0, (y0 + y1) / 2.0, x1, (y0 + y1) / 2.0],
            [(x0 + x1) / 2.0, y0 - 50.0, (x0 + x1) / 2.0, y1 + 50.0],
        ];
        let rs: Vec<String> = segs
            .iter()
            .map(|s| {
                format!(
                    "{}/{}",
                    bounds.aabb_intersects_segment(s[0], s[1], s[2], s[3]) as i32,
                    hit(bounds.intersects_segment(s[0], s[1], s[2], s[3]))
                )
            })
            .collect();
        let cs: Vec<String> = cpp["segments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| format!("{}/{}", p["aabb"], p["hit"]))
            .collect();
        err("segments", rs.join(" "), cs.join(" "));
    }
}

fn check(
    driver: &Path,
    skeleton: &Path,
    atlas: Option<&Path>,
    data: &Arc<SkeletonData>,
    animation: Option<&str>,
    errors: &mut Vec<String>,
) {
    let mut cmd = Command::new(driver);
    cmd.arg(skeleton)
        .arg(atlas.map_or("-".as_ref(), |a| a.as_os_str()))
        .arg(animation.unwrap_or("-"))
        .arg(FRAMES.to_string())
        .arg("1");
    let out = cmd.output().expect("run render driver");
    assert!(
        out.status.success(),
        "driver failed for {}: {}",
        skeleton.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    let cpp = parse(&String::from_utf8_lossy(&out.stdout));
    assert_eq!(cpp.len(), FRAMES);

    let mut sk = Skeleton::new(data.clone());
    let mut state = AnimationState::new(Arc::new(AnimationStateData::new(data.clone())));
    if let Some(name) = animation {
        state.set_animation_by_name(0, name, true).unwrap();
    }
    let (mut renderer, mut clipper, mut bounds) = (
        SkeletonRenderer::new(),
        SkeletonClipping::new(),
        SkeletonBounds::new(),
    );
    let label = format!("{} {}", data.name, animation.unwrap_or("(setup)"));
    for (i, c) in cpp.iter().enumerate() {
        sk.set_position(
            (i as f32 * 0.15).sin() * 80.0,
            (i as f32 * 0.11).cos() * 30.0,
        );
        sk.update_time(DT);
        state.update(DT);
        state.apply(&mut sk.borrow());
        sk.update_world_transform(if i == 0 {
            Physics::Reset
        } else {
            Physics::Update
        });
        check_frame(
            &label,
            i,
            &sk,
            &mut renderer,
            &mut clipper,
            &mut bounds,
            c,
            errors,
        );
    }
}

fn selected(files: Vec<(PathBuf, PathBuf)>) -> Vec<(PathBuf, PathBuf)> {
    if let Ok(filter) = std::env::var("GOLDEN_SKELETON") {
        return files
            .into_iter()
            .filter(|(p, _)| p.to_string_lossy().contains(&filter))
            .collect();
    }
    if std::env::var_os("GOLDEN_ALL").is_some() {
        return files;
    }
    const DEFAULT: [&str; 6] = [
        "spineboy-pro",
        "coin-pro",
        "tank-pro",
        "alien-pro",
        "goblins-pro",
        "dragon-ess",
    ];
    files
        .into_iter()
        .filter(|(p, _)| {
            DEFAULT
                .iter()
                .any(|d| p.file_stem().is_some_and(|s| s == *d))
        })
        .collect()
}

fn golden(ext: &str) {
    let Some(driver) = std::env::var_os("SPINE_CPP_RENDER_DRIVER").map(PathBuf::from) else {
        eprintln!("SPINE_CPP_RENDER_DRIVER not set; skipping");
        return;
    };
    if !common::comparable_build() {
        return;
    }
    let files = selected(exports(ext));
    if files.is_empty() {
        eprintln!("SPINE_RUNTIMES_DIR not set; skipping");
        return;
    }
    let mut errors = Vec::new();
    for (skeleton, atlas) in &files {
        let data = load(skeleton, atlas);
        let mut animations: Vec<Option<String>> = vec![None];
        animations.extend(data.animations.iter().map(|a| Some(a.name().to_string())));
        for animation in animations {
            check(
                &driver,
                skeleton,
                Some(atlas),
                &data,
                animation.as_deref(),
                &mut errors,
            );
        }
        eprintln!("compared {}", data.name);
    }
    assert!(
        errors.is_empty(),
        "{} mismatches, first:\n{}",
        errors.len(),
        errors
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn json_render_matches_reference() {
    golden("json");
}

#[test]
fn binary_render_matches_reference() {
    golden("skel");
}

/// Clipping shapes the examples don't contain (convex flag, inverse, concave, restarts); the
/// fixtures come from `tests/fixtures/gen_clip.py`.
#[test]
fn synthetic_clipping_matches_reference() {
    let Some(driver) = std::env::var_os("SPINE_CPP_RENDER_DRIVER").map(PathBuf::from) else {
        eprintln!("SPINE_CPP_RENDER_DRIVER not set; skipping");
        return;
    };
    if !common::comparable_build() {
        return;
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let atlas = dir.join("clip.atlas");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    assert!(!files.is_empty());
    let mut errors = Vec::new();
    for skeleton in &files {
        let data = load(skeleton, &atlas);
        for animation in [None, Some("spin")] {
            check(
                &driver,
                skeleton,
                Some(&atlas),
                &data,
                animation,
                &mut errors,
            );
        }
    }
    assert!(
        errors.is_empty(),
        "{} mismatches, first:\n{}",
        errors.len(),
        errors
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// More than 0xffff indices in one run of same-texture quads: the renderer must split batches
/// exactly where the reference does.
#[test]
fn batch_split_matches_reference() {
    let Some(driver) = std::env::var_os("SPINE_CPP_RENDER_DRIVER").map(PathBuf::from) else {
        eprintln!("SPINE_CPP_RENDER_DRIVER not set; skipping");
        return;
    };
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let atlas = dir.join("clip.atlas");
    let slots: Vec<String> = (0..11_000)
        .map(|i| format!(r#"{{"name":"s{i}","bone":"root","attachment":"r1"}}"#))
        .collect();
    let json = format!(
        r#"{{"skeleton":{{"spine":"4.3.0"}},"bones":[{{"name":"root"}}],"slots":[{}],"skins":[{{"name":"default","attachments":{{{}}}}}]}}"#,
        slots.join(","),
        (0..11_000)
            .map(|i| format!(
                r#""s{i}":{{"r1":{{"x":{},"y":0,"width":4,"height":4}}}}"#,
                i % 100
            ))
            .collect::<Vec<_>>()
            .join(",")
    );
    let path = std::env::temp_dir().join("spine2d-batch-split.json");
    std::fs::write(&path, json).unwrap();
    let data = load(&path, &atlas);
    let mut errors = Vec::new();
    check(&driver, &path, Some(&atlas), &data, None, &mut errors);
    assert!(
        errors.is_empty(),
        "{} mismatches, first:\n{}",
        errors.len(),
        errors
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
