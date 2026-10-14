use spine2d_core::*;
use std::sync::Arc;

// Spine 4.3 defaults to a y-down world; these tests are written for y-up like the reference's
// headless test.
#[ctor::ctor(unsafe)]
fn init_y_up() {
    spine2d_core::set_y_down(false);
}

fn approx(a: f32, b: f32) {
    assert!((a - b).abs() < 1e-3, "{a} != {b}");
}

fn point(name: &str) -> Arc<Attachment> {
    Arc::new(Attachment::Point(PointAttachment::new(name)))
}

fn mesh(name: &str, vertices: Vec<f32>) -> MeshAttachment {
    let mut m = MeshAttachment::new(name, Sequence::new(1, false));
    m.vertex.world_vertices_length = vertices.len();
    m.vertex.vertices = vertices;
    m
}

/// root(0) -> a(1) -> b(2), one slot on bone `a`.
fn data_with_skins(default: Skin, others: Vec<Skin>) -> Arc<SkeletonData> {
    let mut data = SkeletonData::new();
    data.bones = vec![
        BoneData::new(0, "root", None),
        BoneData::new(1, "a", Some(0)),
        BoneData::new(2, "b", Some(1)),
    ];
    let mut slot = SlotData::new(0, "slot", BoneId(1));
    slot.attachment_name = "att".into();
    data.slots = vec![slot];
    let default = Arc::new(default);
    data.skins.push(default.clone());
    data.skins.extend(others.into_iter().map(Arc::new));
    data.default_skin = Some(default);
    Arc::new(data)
}

#[test]
fn setup_pose_resolves_attachment_from_default_skin() {
    let mut skin = Skin::new("default");
    let att = point("att");
    skin.set_attachment(0, "att", att.clone());
    let sk = Skeleton::new(data_with_skins(skin, vec![]));
    let shown = sk.slot(SlotId(0)).pose.attachment.as_ref().unwrap();
    assert!(Arc::ptr_eq(shown, &att));
    assert_eq!(sk.slot(SlotId(0)).pose.sequence_index, -1);
}

#[test]
fn switching_skins_replaces_shown_attachment() {
    let mut default = Skin::new("default");
    let base = point("att");
    default.set_attachment(0, "att", base.clone());
    let mut other = Skin::new("other");
    let alt = point("att");
    other.set_attachment(0, "att", alt.clone());

    let mut sk = Skeleton::new(data_with_skins(default, vec![other]));
    assert!(sk.set_skin_by_name("other"));
    assert!(Arc::ptr_eq(
        sk.slot(SlotId(0)).pose.attachment.as_ref().unwrap(),
        &alt
    ));

    // Switching to a skin without the placeholder leaves the attachment alone.
    sk.set_skin(None);
    assert!(Arc::ptr_eq(
        sk.slot(SlotId(0)).pose.attachment.as_ref().unwrap(),
        &alt
    ));
    assert!(!sk.set_skin_by_name("missing"));
}

#[test]
fn deform_is_kept_only_when_timeline_attachment_is_shared() {
    let mut default = Skin::new("default");
    let source = Arc::new(Attachment::Mesh(mesh(
        "att",
        vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
    )));
    default.set_attachment(0, "att", source.clone());
    let mut other = Skin::new("other");
    let linked = Arc::new(Attachment::Mesh(
        MeshAttachment::new_linked_mesh(&source).unwrap(),
    ));
    other.set_attachment(0, "att", linked.clone());
    let unrelated = Arc::new(Attachment::Mesh(mesh(
        "att",
        vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
    )));
    other.set_attachment(0, "x", unrelated.clone());

    let mut sk = Skeleton::new(data_with_skins(default, vec![other]));
    sk.slot_mut(SlotId(0)).pose.deform = vec![1.0; 6];

    // Linked mesh shares the source's timeline: deform survives.
    sk.set_skin_by_name("other");
    assert!(Arc::ptr_eq(
        sk.slot(SlotId(0)).pose.attachment.as_ref().unwrap(),
        &linked
    ));
    assert_eq!(sk.slot(SlotId(0)).pose.deform.len(), 6);

    // An unrelated attachment clears it.
    assert!(sk.set_attachment("slot", "x"));
    assert!(Arc::ptr_eq(
        sk.slot(SlotId(0)).pose.attachment.as_ref().unwrap(),
        &unrelated
    ));
    assert!(sk.slot(SlotId(0)).pose.deform.is_empty());

    // Clearing, and rejecting unknown names.
    assert!(sk.set_attachment("slot", ""));
    assert!(sk.slot(SlotId(0)).pose.attachment.is_none());
    assert!(!sk.set_attachment("slot", "nope"));
    assert!(!sk.set_attachment("nope", "x"));
}

#[test]
fn skin_required_bones_activate_with_their_skin() {
    let mut data = SkeletonData::new();
    let mut a = BoneData::new(1, "a", Some(0));
    a.skin_required = true;
    let mut b = BoneData::new(2, "b", Some(1));
    b.skin_required = true;
    data.bones = vec![BoneData::new(0, "root", None), a, b];
    let mut skin = Skin::new("with-b");
    skin.bones = vec![2];
    let skin = Arc::new(skin);
    data.skins.push(skin.clone());
    let mut sk = Skeleton::new(Arc::new(data));

    assert!(!sk.bone(BoneId(1)).is_active());
    assert_eq!(sk.update_cache_entries().len(), 1);

    sk.set_skin(Some(skin));
    // Activating bone b activates its ancestors too.
    assert!(sk.bone(BoneId(1)).is_active() && sk.bone(BoneId(2)).is_active());
    assert_eq!(sk.update_cache_entries().len(), 3);

    sk.set_skin(None);
    assert!(!sk.bone(BoneId(2)).is_active());
}

#[test]
fn region_world_vertices_without_atlas() {
    let mut region = RegionAttachment::new("r", Sequence::new(1, false));
    region.width = 10.0;
    region.height = 20.0;
    region.update_sequence();

    let mut data = SkeletonData::new();
    let mut bone = BoneData::new(0, "root", None);
    bone.setup_pose.x = 100.0;
    data.bones = vec![bone];
    data.slots = vec![SlotData::new(0, "slot", BoneId(0))];
    let mut sk = Skeleton::new(Arc::new(data));
    sk.update_world_transform(Physics::None);

    let mut out = [0.0f32; 8];
    let pose = &sk.slot(SlotId(0)).pose;
    RegionAttachment::compute_world_vertices(&sk, SlotId(0), region.offsets(pose), &mut out, 0, 2);
    // Output order is bottom-right, bottom-left, upper-left, upper-right.
    let expected = [105.0, -10.0, 95.0, -10.0, 95.0, 10.0, 105.0, 10.0];
    for (o, e) in out.iter().zip(expected) {
        approx(*o, e);
    }
    // Without a region the UVs cover the whole texture.
    assert_eq!(
        region.sequence.uvs(0),
        &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 0.0]
    );
}

#[test]
fn region_uvs_follow_texture_region_and_rotation() {
    let region = TextureRegion {
        u: 0.1,
        v: 0.2,
        u2: 0.3,
        v2: 0.4,
        ..Default::default()
    };
    let (mut off, mut uvs) = ([0.0; 8], [0.0; 8]);
    RegionAttachment::compute_uvs(
        Some(&region),
        0.0,
        0.0,
        1.0,
        1.0,
        0.0,
        2.0,
        2.0,
        &mut off,
        &mut uvs,
    );
    // bottom-left = (u2, v2), upper-right = (u, v)
    assert_eq!((uvs[0], uvs[1]), (0.3, 0.4));
    assert_eq!((uvs[4], uvs[5]), (0.1, 0.2));

    let mut rotated = region.clone();
    rotated.atlas = Some(AtlasRegionInfo {
        degrees: 90,
        original_width: 2,
        original_height: 2,
        packed_width: 2,
        packed_height: 2,
        ..Default::default()
    });
    RegionAttachment::compute_uvs(
        Some(&rotated),
        0.0,
        0.0,
        1.0,
        1.0,
        0.0,
        2.0,
        2.0,
        &mut off,
        &mut uvs,
    );
    assert_eq!((uvs[0], uvs[1]), (0.3, 0.2)); // bly = v when rotated
}

#[test]
fn unweighted_mesh_world_vertices_with_and_without_deform() {
    let m = mesh("m", vec![1.0, 0.0, 0.0, 1.0]);
    let mut data = SkeletonData::new();
    let mut bone = BoneData::new(0, "root", None);
    bone.setup_pose.rotation = 90.0;
    bone.setup_pose.x = 10.0;
    data.bones = vec![bone];
    data.slots = vec![SlotData::new(0, "slot", BoneId(0))];
    let mut sk = Skeleton::new(Arc::new(data));
    sk.update_world_transform(Physics::None);

    let mut out = [0.0; 4];
    m.vertex
        .compute_world_vertices(&sk, SlotId(0), 0, 4, &mut out, 0, 2);
    // Rotated 90 degrees: (1,0) -> (0,1), (0,1) -> (-1,0), then translated by (10, 0).
    for (o, e) in out.iter().zip([10.0, 1.0, 9.0, 0.0]) {
        approx(*o, e);
    }

    sk.slot_mut(SlotId(0)).pose.deform = vec![1.0, 0.0, 0.0, 0.0];
    m.vertex
        .compute_world_vertices(&sk, SlotId(0), 0, 4, &mut out, 0, 2);
    // With deform the slot's deform replaces the vertices array.
    for (o, e) in out.iter().zip([10.0, 1.0, 10.0, 0.0]) {
        approx(*o, e);
    }

    // Stride and offset interleave into a larger buffer.
    let mut strided = [-1.0; 8];
    m.vertex
        .compute_world_vertices(&sk, SlotId(0), 0, 4, &mut strided, 1, 3);
    assert_eq!((strided[0], strided[3]), (-1.0, -1.0));
    approx(strided[1], 10.0);
    approx(strided[4], 10.0);
}

#[test]
fn weighted_mesh_blends_bone_influences() {
    // One vertex influenced by root (x+0) and by bone b (moved to x=10), weights 0.5/0.5.
    let mut m = mesh("m", vec![0.0, 0.0, 0.5, 0.0, 0.0, 0.5]);
    m.vertex.bones = vec![2, 0, 1];
    m.vertex.world_vertices_length = 2;

    let mut data = SkeletonData::new();
    let mut b = BoneData::new(1, "b", Some(0));
    b.setup_pose.x = 10.0;
    data.bones = vec![BoneData::new(0, "root", None), b];
    data.slots = vec![SlotData::new(0, "slot", BoneId(0))];
    let mut sk = Skeleton::new(Arc::new(data));
    sk.update_world_transform(Physics::None);

    let mut out = [0.0; 2];
    m.vertex
        .compute_world_vertices(&sk, SlotId(0), 0, 2, &mut out, 0, 2);
    approx(out[0], 5.0);
    approx(out[1], 0.0);

    // Deform offsets are added per influence before weighting.
    sk.slot_mut(SlotId(0)).pose.deform = vec![2.0, 0.0, 0.0, 4.0];
    m.vertex
        .compute_world_vertices(&sk, SlotId(0), 0, 2, &mut out, 0, 2);
    approx(out[0], 5.0 + 1.0);
    approx(out[1], 2.0);
}

#[test]
fn mesh_uvs_handle_atlas_rotation() {
    let page = AtlasRegionInfo {
        page_width: 100,
        page_height: 100,
        original_width: 50,
        original_height: 50,
        packed_width: 50,
        packed_height: 50,
        degrees: 0,
        ..Default::default()
    };
    let region = TextureRegion {
        u: 0.1,
        v: 0.2,
        u2: 0.6,
        v2: 0.7,
        atlas: Some(page.clone()),
        ..Default::default()
    };
    let region_uvs = [0.0, 0.0, 1.0, 1.0];
    let mut uvs = [0.0; 4];
    MeshAttachment::compute_uvs(Some(&region), &region_uvs, &mut uvs);
    for (o, e) in uvs.iter().zip([0.1, 0.2, 0.6, 0.7]) {
        approx(*o, e);
    }

    let mut r180 = region.clone();
    r180.atlas = Some(AtlasRegionInfo {
        degrees: 180,
        ..page
    });
    MeshAttachment::compute_uvs(Some(&r180), &region_uvs, &mut uvs);
    for (o, e) in uvs.iter().zip([0.6, 0.7, 0.1, 0.2]) {
        approx(*o, e);
    }

    // No region: uvs pass through.
    MeshAttachment::compute_uvs(None, &region_uvs, &mut uvs);
    assert_eq!(uvs, region_uvs);
}

#[test]
fn point_attachment_world_position_and_rotation() {
    let mut p = PointAttachment::new("p");
    p.x = 2.0;
    p.y = 0.0;
    p.rotation = 45.0;
    let mut data = SkeletonData::new();
    let mut bone = BoneData::new(0, "root", None);
    bone.setup_pose.rotation = 45.0;
    bone.setup_pose.x = 1.0;
    data.bones = vec![bone];
    let mut sk = Skeleton::new(Arc::new(data));
    sk.update_world_transform(Physics::None);
    let pose = sk.bone(BoneId(0)).applied_pose();
    let (x, y) = p.compute_world_position(pose);
    approx(x, 1.0 + 2.0 * 45f32.to_radians().cos());
    approx(y, 2.0 * 45f32.to_radians().sin());
    approx(p.compute_world_rotation(pose), 90.0);
}

#[test]
fn timeline_active_checks_current_and_extra_slots() {
    let mut default = Skin::new("default");
    let mut shared = PointAttachment::new("shared");
    shared.base.timeline_slots.push(1);
    let shared = Arc::new(Attachment::Point(shared));
    default.set_attachment(0, "att", shared.clone());
    default.set_attachment(1, "att2", shared.clone());

    let mut data = SkeletonData::new();
    data.bones = vec![BoneData::new(0, "root", None)];
    let mut s0 = SlotData::new(0, "s0", BoneId(0));
    s0.attachment_name = "att".into();
    let s1 = SlotData::new(1, "s1", BoneId(0));
    data.slots = vec![s0, s1];
    let default = Arc::new(default);
    data.skins.push(default.clone());
    data.default_skin = Some(default);
    let mut sk = Skeleton::new(Arc::new(data));

    assert!(shared.is_timeline_active(&sk, 0, false));
    sk.slot_mut(SlotId(0)).pose.set_attachment(None);
    assert!(!shared.is_timeline_active(&sk, 0, false));
    sk.set_attachment("s1", "att2");
    assert!(
        shared.is_timeline_active(&sk, 0, false),
        "extra timeline slot 1 shows it"
    );
}
