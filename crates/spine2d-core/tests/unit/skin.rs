use spine2d_core::attachment::{Attachment, MeshAttachment, PointAttachment};
use spine2d_core::bone::{BoneData, BoneId};
use spine2d_core::sequence::Sequence;
use spine2d_core::skeleton_data::SkeletonData;
use spine2d_core::skin::Skin;
use spine2d_core::slot::SlotData;
use std::sync::Arc;

fn point(name: &str) -> Arc<Attachment> {
    Arc::new(Attachment::Point(PointAttachment::new(name)))
}

#[test]
fn set_get_replace_remove() {
    let mut skin = Skin::new("s");
    let (a, b) = (point("a"), point("b"));
    skin.set_attachment(2, "p", a.clone());
    assert!(Arc::ptr_eq(skin.attachment(2, "p").unwrap(), &a));
    assert!(skin.attachment(0, "p").is_none());
    skin.set_attachment(2, "p", b.clone());
    assert!(Arc::ptr_eq(skin.attachment(2, "p").unwrap(), &b));
    assert_eq!(skin.entries().count(), 1);
    skin.remove_attachment(2, "p");
    assert!(skin.attachment(2, "p").is_none());
}

#[test]
fn entries_ordered_by_slot_then_insertion() {
    let mut skin = Skin::new("s");
    skin.set_attachment(3, "z", point("z"));
    skin.set_attachment(1, "b", point("b"));
    skin.set_attachment(1, "a", point("a"));
    let order: Vec<_> = skin
        .entries()
        .map(|e| (e.slot_index, e.placeholder.as_str()))
        .collect();
    assert_eq!(order, [(1, "b"), (1, "a"), (3, "z")]);
}

#[test]
fn add_skin_shares_copy_skin_clones() {
    let mut base = Skin::new("base");
    let a = point("a");
    base.set_attachment(0, "a", a.clone());
    base.bones = vec![1];

    let mut added = Skin::new("added");
    added.bones = vec![1, 2];
    added.add_skin(&base);
    assert!(Arc::ptr_eq(added.attachment(0, "a").unwrap(), &a));
    assert_eq!(added.bones, [1, 2]);

    let mut copied = Skin::new("copied");
    copied.copy_skin(&base);
    let c = copied.attachment(0, "a").unwrap();
    assert!(!Arc::ptr_eq(c, &a));
    assert_eq!(c.timeline_id(), a.id());
}

#[test]
fn fits_checks_slots_bones_and_constraints() {
    let mut data = SkeletonData::default();
    data.bones.push(BoneData::new(0, "root", None));
    for (index, name) in ["a", "b"].into_iter().enumerate() {
        data.slots.push(SlotData::new(index, name, BoneId(0)));
    }

    let mut skin = Skin::new("s");
    skin.set_attachment(1, "p", point("p"));
    assert!(skin.fits(&data));
    assert!(Skin::new("empty").fits(&data));

    skin.set_attachment(2, "p", point("p"));
    assert!(!skin.fits(&data), "slot 2 does not exist");

    let mut with_bone = Skin::new("b");
    with_bone.bones.push(1);
    assert!(!with_bone.fits(&data), "bone 1 does not exist");

    let mut with_constraint = Skin::new("c");
    with_constraint.constraints.push(0);
    assert!(!with_constraint.fits(&data), "there are no constraints");
}

#[test]
fn fits_checks_the_bones_of_weighted_attachments() {
    let mut data = SkeletonData::default();
    data.bones.push(BoneData::new(0, "root", None));
    data.bones.push(BoneData::new(1, "arm", Some(0)));
    data.slots.push(SlotData::new(0, "a", BoneId(0)));

    let weighted = |bones: Vec<usize>| {
        let mut mesh = MeshAttachment::new("m", Sequence::new(1, false));
        mesh.vertex.bones = bones;
        Arc::new(Attachment::Mesh(mesh))
    };
    // One vertex with two influences, then one vertex with one influence.
    let fitting = weighted(vec![2, 0, 1, 1, 1]);
    let missing_bone = weighted(vec![1, 2]);
    let truncated = weighted(vec![2, 0]);

    let mut skin = Skin::new("s");
    skin.set_attachment(0, "m", fitting);
    assert!(skin.fits(&data));
    skin.set_attachment(0, "m", missing_bone);
    assert!(!skin.fits(&data), "bone 2 does not exist");
    skin.set_attachment(0, "m", truncated);
    assert!(!skin.fits(&data), "the bone list ends inside a vertex");
}
