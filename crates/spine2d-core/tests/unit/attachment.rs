use spine2d_core::attachment::{
    Attachment, MeshAttachment, MeshError, PointAttachment, RegionAttachment,
};
use spine2d_core::sequence::Sequence;
use spine2d_core::texture_region::TextureRegion;
use std::sync::Arc;

#[test]
fn ids_are_unique_and_timeline_defaults_to_self() {
    let a = Attachment::Point(PointAttachment::new("a"));
    let b = Attachment::Point(PointAttachment::new("a"));
    assert_ne!(a.id(), b.id());
    assert_eq!(a.timeline_id(), a.id());
}

#[test]
fn copy_shares_timeline_but_not_identity() {
    let a = Arc::new(Attachment::Region(RegionAttachment::new(
        "r",
        Sequence::new(1, false),
    )));
    let c = Attachment::copy(&a);
    assert_ne!(c.id(), a.id());
    assert_eq!(c.timeline_id(), a.id());
}

#[test]
fn linked_mesh_copy_follows_source() {
    let mut src = MeshAttachment::new("m", Sequence::new(1, false));
    src.vertex.vertices = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    src.region_uvs = vec![0.0; 6];
    src.triangles = vec![0, 1, 2];
    src.hull_length = 6;
    let src = Arc::new(Attachment::Mesh(src));

    let linked = Arc::new(Attachment::Mesh(
        MeshAttachment::new_linked_mesh(&src).unwrap(),
    ));
    assert_eq!(
        linked.as_mesh().unwrap().vertex.vertices,
        vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]
    );
    assert_eq!(linked.timeline_id(), src.id());
    assert!(Arc::ptr_eq(
        linked.as_mesh().unwrap().source_mesh().unwrap(),
        &src
    ));

    // Copying a linked mesh re-links to the original source, not to the intermediate.
    let copy = Attachment::copy(&linked);
    assert!(Arc::ptr_eq(
        copy.as_mesh().unwrap().source_mesh().unwrap(),
        &src
    ));
}

#[test]
fn a_plain_texture_region_attachment_is_centered_and_maps_the_whole_region() {
    let region = Arc::new(TextureRegion {
        renderer_object: 7,
        u: 0.25,
        v: 0.5,
        u2: 0.75,
        v2: 1.0,
        region_width: 100,
        region_height: 50,
        atlas: None,
    });
    let attachment = RegionAttachment::with_region("r", region, 100.0, 50.0);

    let offsets = attachment.sequence.offsets(0);
    // Bottom-left, upper-left, upper-right, bottom-right.
    assert_eq!(
        offsets,
        [-50.0, -25.0, -50.0, 25.0, 50.0, 25.0, 50.0, -25.0]
    );
    let uvs = attachment.sequence.uvs(0);
    assert_eq!(uvs, [0.75, 1.0, 0.25, 1.0, 0.25, 0.5, 0.75, 0.5]);
    assert_eq!(attachment.sequence.region(0).unwrap().renderer_object, 7);
}

fn plain_region() -> Arc<TextureRegion> {
    Arc::new(TextureRegion {
        renderer_object: 3,
        u: 0.0,
        v: 0.0,
        u2: 1.0,
        v2: 1.0,
        region_width: 64,
        region_height: 64,
        atlas: None,
    })
}

const QUAD_UVS: [f32; 8] = [0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0];
const QUAD_TRIANGLES: [u16; 6] = [0, 1, 2, 2, 3, 0];
const QUAD_VERTICES: [f32; 8] = [-10.0, -10.0, 10.0, -10.0, 10.0, 10.0, -10.0, 10.0];

fn quad(vertices: &[f32]) -> Result<MeshAttachment, MeshError> {
    MeshAttachment::from_arrays("m", plain_region(), &QUAD_UVS, &QUAD_TRIANGLES, vertices, 4)
}

#[test]
fn an_unweighted_mesh_keeps_its_arrays() {
    let mesh = quad(&QUAD_VERTICES).unwrap();
    assert!(mesh.vertex.bones.is_empty());
    assert_eq!(mesh.vertex.vertices, QUAD_VERTICES);
    assert_eq!(mesh.triangles, QUAD_TRIANGLES);
    assert_eq!(mesh.hull_length, 8);
    assert_eq!(mesh.vertex.world_vertices_length, 8);
    assert_eq!(mesh.sequence.uvs(0), QUAD_UVS);
}

#[test]
fn a_weighted_mesh_is_split_into_bones_and_weights() {
    // Vertex 0 follows bones 1 and 2; the other three follow bone 0 alone.
    let mut data = vec![2.0, 1.0, -10.0, -10.0, 0.25, 2.0, -9.0, -9.0, 0.75];
    for (x, y) in [(10.0, -10.0), (10.0, 10.0), (-10.0, 10.0)] {
        data.extend_from_slice(&[1.0, 0.0, x, y, 1.0]);
    }
    let mesh = quad(&data).unwrap();
    assert_eq!(mesh.vertex.bones, [2, 1, 2, 1, 0, 1, 0, 1, 0]);
    assert_eq!(
        mesh.vertex.vertices,
        [
            -10.0, -10.0, 0.25, -9.0, -9.0, 0.75, 10.0, -10.0, 1.0, 10.0, 10.0, 1.0, -10.0, 10.0,
            1.0
        ]
    );
    assert!(mesh.vertex.bones_fit(3));
    assert!(!mesh.vertex.bones_fit(2));
}

#[test]
fn bad_mesh_arrays_are_refused() {
    let region = plain_region;
    let build = |uvs: &[f32], triangles: &[u16], vertices: &[f32], hull: usize| {
        MeshAttachment::from_arrays("m", region(), uvs, triangles, vertices, hull).err()
    };
    assert_eq!(
        build(&QUAD_UVS[..7], &QUAD_TRIANGLES, &QUAD_VERTICES, 4),
        Some(MeshError::OddUvCount(7))
    );
    assert_eq!(
        build(&QUAD_UVS[..4], &[0, 1, 0], &QUAD_VERTICES[..4], 2),
        Some(MeshError::TooFewVertices(2))
    );
    assert_eq!(
        build(&QUAD_UVS, &QUAD_TRIANGLES[..5], &QUAD_VERTICES, 4),
        Some(MeshError::PartialTriangle(5))
    );
    assert_eq!(
        build(&QUAD_UVS, &[0, 1, 4, 2, 3, 0], &QUAD_VERTICES, 4),
        Some(MeshError::TriangleOutOfRange {
            at: 2,
            value: 4,
            vertex_count: 4
        })
    );
    assert_eq!(
        build(&QUAD_UVS, &QUAD_TRIANGLES, &QUAD_VERTICES, 5),
        Some(MeshError::HullTooLarge {
            hull: 5,
            vertex_count: 4
        })
    );
    assert_eq!(
        build(&QUAD_UVS, &QUAD_TRIANGLES, &QUAD_VERTICES[..6], 4),
        Some(MeshError::VertexCountMismatch {
            len: 6,
            expected: 8
        })
    );
    let mut nan = QUAD_VERTICES;
    nan[3] = f32::NAN;
    assert_eq!(
        build(&QUAD_UVS, &QUAD_TRIANGLES, &nan, 4),
        Some(MeshError::NotFinite { array: "vertices" })
    );
}

#[test]
fn bad_weighted_vertices_are_refused() {
    let one = |bone_count: f32, bone: f32| {
        // Four vertices of one bone each, with the first vertex changed.
        let mut data = vec![bone_count, bone, 0.0, 0.0, 1.0];
        for _ in 0..3 {
            data.extend_from_slice(&[1.0, 0.0, 0.0, 0.0, 1.0]);
        }
        quad(&data).err()
    };
    assert!(matches!(
        one(1.5, 0.0),
        Some(MeshError::BadWeightedVertex { vertex: 0, .. })
    ));
    assert!(matches!(
        one(0.0, 0.0),
        Some(MeshError::BadWeightedVertex { vertex: 0, .. })
    ));
    assert!(matches!(
        one(1.0, -1.0),
        Some(MeshError::BadWeightedVertex { vertex: 0, .. })
    ));
    assert!(matches!(
        one(1.0, 0.5),
        Some(MeshError::BadWeightedVertex { vertex: 0, .. })
    ));
    assert!(matches!(
        one(9.0, 0.0),
        Some(MeshError::BadWeightedVertex { vertex: 0, .. })
    ));

    // Three vertices of data for four UVs.
    let three: Vec<f32> = (0..3).flat_map(|_| [1.0, 0.0, 0.0, 0.0, 1.0]).collect();
    assert_eq!(
        quad(&three).err(),
        Some(MeshError::WeightedVertexCountMismatch {
            found: 3,
            expected: 4
        })
    );
}

#[test]
fn a_linked_mesh_of_a_script_mesh_keeps_the_shape() {
    let source = Arc::new(Attachment::Mesh(quad(&QUAD_VERTICES).unwrap()));
    let linked = MeshAttachment::new_linked_mesh(&source).unwrap();
    assert_eq!(linked.vertex.vertices, QUAD_VERTICES);
    assert_eq!(linked.triangles, QUAD_TRIANGLES);
}
