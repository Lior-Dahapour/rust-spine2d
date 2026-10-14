//! `render_batches_split`: groups break after the requested slots and the markers say where.

#[allow(dead_code)]
#[path = "../../spine2d-core/tests/common/mod.rs"]
mod common;

use spine2d_core::{Physics, Skeleton, SlotId};
use spine2d_render::{SkeletonRenderer, SlotMarker};
use std::path::Path;

/// Five slots on one texture: s0, s1, (s2 draws nothing), s3, s4. Unsplit they form one group.
fn skeleton() -> Skeleton {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let data = common::load(&dir.join("split/split-slots.json"), &dir.join("clip.atlas"));
    let mut skeleton = Skeleton::new(data);
    skeleton.update_world_transform(Physics::None);
    skeleton
}

fn split_at(slots: &[usize]) -> Vec<bool> {
    let mut flags = vec![false; 5];
    for &slot in slots {
        flags[slot] = true;
    }
    flags
}

/// Group sizes in sub-commands and the markers for a split at `slots`.
fn run(slots: &[usize]) -> (Vec<usize>, Vec<(usize, usize)>) {
    let skeleton = skeleton();
    let mut renderer = SkeletonRenderer::new();
    let batches = renderer.render_batches_split(&skeleton, &split_at(slots));
    let sizes = batches.groups.iter().map(|g| g.end - g.start).collect();
    let markers = batches
        .markers
        .iter()
        .map(|m: &SlotMarker| (m.slot.0, m.groups_before))
        .collect();
    (sizes, markers)
}

#[test]
fn no_split_slots_matches_render_batches() {
    let skeleton = skeleton();
    let mut renderer = SkeletonRenderer::new();
    let plain = renderer.render_batches(&skeleton).groups.to_vec();
    let split = renderer.render_batches_split(&skeleton, &[]);
    assert_eq!(split.groups, plain);
    assert!(split.markers.is_empty());
    assert_eq!(plain.len(), 1);
    assert_eq!(plain[0].end - plain[0].start, 4);
}

#[test]
fn a_drawing_slot_splits_the_group_after_it() {
    let (sizes, markers) = run(&[1]);
    assert_eq!(sizes, [2, 2]);
    assert_eq!(markers, [(1, 1)]);
}

#[test]
fn a_slot_without_attachment_still_splits_at_its_place() {
    let (sizes, markers) = run(&[2]);
    assert_eq!(sizes, [2, 2]);
    assert_eq!(markers, [(2, 1)]);
}

#[test]
fn first_and_last_slot() {
    let (sizes, markers) = run(&[0, 4]);
    assert_eq!(sizes, [1, 3]);
    assert_eq!(markers, [(0, 1), (4, 2)]);
}

#[test]
fn several_markers_between_the_same_groups_share_a_position() {
    let (sizes, markers) = run(&[1, 2]);
    assert_eq!(sizes, [2, 2]);
    assert_eq!(markers, [(1, 1), (2, 1)]);
}

#[test]
fn every_slot_split_keeps_every_command_once() {
    let skeleton = skeleton();
    let mut renderer = SkeletonRenderer::new();
    let batches = renderer.render_batches_split(&skeleton, &[true; 5]);
    let covered: usize = batches.groups.iter().map(|g| g.end - g.start).sum();
    assert_eq!(covered, batches.commands.len());
    assert_eq!(batches.groups.len(), 4);
    let groups_before: Vec<_> = batches.markers.iter().map(|m| m.groups_before).collect();
    assert_eq!(groups_before, [1, 2, 2, 3, 4]);
}

#[test]
fn slots_beyond_the_flags_are_not_split() {
    let skeleton = skeleton();
    let mut renderer = SkeletonRenderer::new();
    let batches = renderer.render_batches_split(&skeleton, &[false, true]);
    assert_eq!(batches.groups.len(), 2);
    assert_eq!(batches.markers.len(), 1);
    assert_eq!(batches.markers[0].slot, SlotId(1));
}
