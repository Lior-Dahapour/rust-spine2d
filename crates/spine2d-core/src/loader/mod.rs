// Spine Runtimes License Agreement
// Last updated April 5, 2025. Replaces all prior versions.
//
// Copyright (c) 2013-2025, Esoteric Software LLC
//
// Integration of the Spine Runtimes into software or otherwise creating
// derivative works of the Spine Runtimes is permitted under the terms and
// conditions of Section 2 of the Spine Editor License Agreement:
// http://esotericsoftware.com/spine-editor-license
//
// Otherwise, it is permitted to integrate the Spine Runtimes into software
// or otherwise create derivative works of the Spine Runtimes (collectively,
// "Products"), provided that each user of the Products must obtain their own
// Spine Editor license and redistribution of the Products in any form must
// include this license and copyright notice.
//
// THE SPINE RUNTIMES ARE PROVIDED BY ESOTERIC SOFTWARE LLC "AS IS" AND ANY
// EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL ESOTERIC SOFTWARE LLC BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES,
// BUSINESS INTERRUPTION, OR LOSS OF USE, DATA, OR PROFITS) HOWEVER CAUSED AND
// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF
// THE SPINE RUNTIMES, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
// Unofficial Rust port of the Spine 4.3 runtime (spine-cpp). Not an official Spine runtime.

//! Skeleton loaders: [`SkeletonJson`] and [`SkeletonBinary`].
//!
//! Both read the Spine 4.3 export formats into a [`SkeletonData`], including constraints and
//! their timelines.

mod binary;
mod json;

pub use binary::SkeletonBinary;
pub use json::SkeletonJson;

use crate::attachment::Attachment;
use crate::skeleton_data::SkeletonData;
use crate::skin::Skin;
use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use std::fmt;
use std::sync::Arc;

/// The export version the loaders accept: files must start with this string.
pub const VERSION: &str = "4.3";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadError(pub String);

impl LoadError {
    pub(crate) fn new(prefix: &str, detail: &str) -> LoadError {
        LoadError(format!("{prefix}{detail}"))
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LoadError {}

impl From<std::io::Error> for LoadError {
    fn from(e: std::io::Error) -> Self {
        LoadError(format!("Unable to read skeleton file: {e}"))
    }
}

impl From<serde_json::Error> for LoadError {
    fn from(e: serde_json::Error) -> Self {
        LoadError(format!("Invalid skeleton JSON: {e}"))
    }
}

/// The skin a linked mesh takes its source from.
pub(crate) enum SourceSkin {
    /// The default skin.
    Default,
    Name(SmolStr),
    Index(usize),
}

/// A linked mesh whose geometry is resolved once all skins have been read.
pub(crate) struct LinkedMesh {
    /// Index of the skin containing the linked mesh.
    pub skin: usize,
    pub slot_index: usize,
    /// The linked mesh's key in its skin.
    pub placeholder: SmolStr,
    pub source_skin: SourceSkin,
    pub source_slot: usize,
    pub source: SmolStr,
    pub inherit_timelines: bool,
}

/// Loader state shared by both formats: the data being built plus name lookups.
pub(crate) struct Build {
    pub data: SkeletonData,
    pub skins: Vec<Skin>,
    pub default_skin: Option<usize>,
    bone_names: FxHashMap<SmolStr, usize>,
    slot_names: FxHashMap<SmolStr, usize>,
    pub linked_meshes: Vec<LinkedMesh>,
    pub warnings: Vec<String>,
}

impl Build {
    pub fn new() -> Build {
        Build {
            data: SkeletonData::new(),
            skins: Vec::new(),
            default_skin: None,
            bone_names: FxHashMap::default(),
            slot_names: FxHashMap::default(),
            linked_meshes: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn push_bone(&mut self, bone: crate::bone::BoneData) {
        self.bone_names
            .entry(bone.name.clone())
            .or_insert(bone.index);
        self.data.bones.push(bone);
    }

    pub fn push_slot(&mut self, slot: crate::slot::SlotData) {
        self.slot_names
            .entry(slot.name.clone())
            .or_insert(slot.index);
        self.data.slots.push(slot);
    }

    pub fn find_bone(&self, name: &str) -> Option<usize> {
        if name.is_empty() {
            return None;
        }
        self.bone_names.get(name).copied()
    }

    pub fn find_slot(&self, name: &str) -> Option<usize> {
        if name.is_empty() {
            return None;
        }
        self.slot_names.get(name).copied()
    }

    pub fn find_skin(&self, name: &str) -> Option<usize> {
        if name.is_empty() {
            return None;
        }
        self.skins.iter().position(|s| s.name() == name)
    }

    /// Index of the first constraint of the wanted kind with the given name.
    pub fn find_constraint(&self, kind: ConstraintKind, name: &str) -> Option<usize> {
        self.data
            .constraints
            .iter()
            .position(|c| kind.matches(c) && c.name() == name)
    }

    pub fn finish(self) -> SkeletonData {
        let mut data = self.data;
        let skins: Vec<Arc<Skin>> = self.skins.into_iter().map(Arc::new).collect();
        data.default_skin = self.default_skin.map(|i| skins[i].clone());
        data.skins = skins;
        data
    }
}

#[derive(Clone, Copy)]
pub(crate) enum ConstraintKind {
    Ik,
    Transform,
    Path,
    Physics,
    Slider,
}

impl ConstraintKind {
    fn matches(self, c: &crate::constraint_data::ConstraintData) -> bool {
        use crate::constraint_data::ConstraintData as C;
        matches!(
            (self, c),
            (ConstraintKind::Ik, C::Ik(_))
                | (ConstraintKind::Transform, C::Transform(_))
                | (ConstraintKind::Path, C::Path(_))
                | (ConstraintKind::Physics, C::Physics(_))
                | (ConstraintKind::Slider, C::Slider(_))
        )
    }
}

/// Something curve data can be written into while reading keys.
pub(crate) trait CurveSink {
    fn set_stepped(&mut self, frame: usize);
    #[allow(clippy::too_many_arguments)]
    fn set_bezier(
        &mut self,
        bezier: usize,
        frame: usize,
        value: usize,
        t1: f32,
        v1: f32,
        cx1: f32,
        cy1: f32,
        cx2: f32,
        cy2: f32,
        t2: f32,
        v2: f32,
    );
}

impl CurveSink for crate::animation::CurveTimeline {
    fn set_stepped(&mut self, frame: usize) {
        CurveTimeline::set_stepped(self, frame);
    }

    fn set_bezier(
        &mut self,
        bezier: usize,
        frame: usize,
        value: usize,
        t1: f32,
        v1: f32,
        cx1: f32,
        cy1: f32,
        cx2: f32,
        cy2: f32,
        t2: f32,
        v2: f32,
    ) {
        CurveTimeline::set_bezier(
            self, bezier, frame, value, t1, v1, cx1, cy1, cx2, cy2, t2, v2,
        );
    }
}

impl CurveSink for crate::animation::DeformTimeline {
    fn set_stepped(&mut self, frame: usize) {
        self.curve.set_stepped(frame);
    }

    fn set_bezier(
        &mut self,
        bezier: usize,
        frame: usize,
        value: usize,
        t1: f32,
        v1: f32,
        cx1: f32,
        cy1: f32,
        cx2: f32,
        cy2: f32,
        t2: f32,
        v2: f32,
    ) {
        DeformTimeline::set_bezier(
            self, bezier, frame, value, t1, v1, cx1, cy1, cx2, cy2, t2, v2,
        );
    }
}

use crate::animation::{
    AlphaTimeline, BoneTimeline1, BoneTimeline2, ColorTimeline, ConstraintTimeline1, CurveTimeline,
    DeformTimeline, IkConstraintTimeline, PathConstraintMixTimeline, TransformConstraintTimeline,
};

macro_rules! sink_via_curve {
    ($($t:ty),*) => {$(
        impl CurveSink for $t {
            fn set_stepped(&mut self, frame: usize) {
                self.curve.set_stepped(frame);
            }

            fn set_bezier(&mut self, bezier: usize, frame: usize, value: usize, t1: f32, v1: f32, cx1: f32, cy1: f32, cx2: f32, cy2: f32, t2: f32, v2: f32) {
                self.curve.set_bezier(bezier, frame, value, t1, v1, cx1, cy1, cx2, cy2, t2, v2);
            }
        }
    )*};
}

sink_via_curve!(
    AlphaTimeline,
    BoneTimeline1,
    BoneTimeline2,
    ColorTimeline,
    ConstraintTimeline1,
    IkConstraintTimeline,
    PathConstraintMixTimeline,
    TransformConstraintTimeline
);

/// Resolves linked meshes: links each to its source mesh, shares its timelines if requested, and
/// rebuilds its sequence UVs. `update_timeline_slots` adds the linked mesh's slot to the
/// source's timeline slots (the JSON format does this; the binary format stores them in the mesh).
pub(crate) fn resolve_linked_meshes(
    build: &mut Build,
    update_timeline_slots: bool,
) -> Result<(), LoadError> {
    let linked = std::mem::take(&mut build.linked_meshes);

    let source_skin_index = |build: &Build, l: &LinkedMesh| -> Result<usize, LoadError> {
        match &l.source_skin {
            SourceSkin::Default => build
                .default_skin
                .ok_or_else(|| LoadError::new("Skin not found: ", "default")),
            SourceSkin::Name(n) => build
                .find_skin(n)
                .ok_or_else(|| LoadError::new("Skin not found: ", n)),
            SourceSkin::Index(i) if *i < build.skins.len() => Ok(*i),
            SourceSkin::Index(i) => Err(LoadError::new("Skin not found: ", &i.to_string())),
        }
    };

    // First pass: extend the sources' timeline slots while they are still uniquely owned.
    if update_timeline_slots {
        for l in &linked {
            if !(l.inherit_timelines && l.slot_index != l.source_slot) {
                continue;
            }
            let si = source_skin_index(build, l)?;
            let source = build.skins[si]
                .attachment_mut(l.source_slot, &l.source)
                .ok_or_else(|| LoadError::new("Source mesh not found: ", &l.source))?;
            let attachment = Arc::get_mut(source)
                .ok_or_else(|| LoadError::new("Source mesh is shared: ", &l.source))?;
            let slots = &mut attachment.base_mut().timeline_slots;
            if !slots.contains(&l.slot_index) {
                slots.push(l.slot_index);
            }
        }
    }

    // Second pass: link the meshes.
    for l in &linked {
        let si = source_skin_index(build, l)?;
        let source: Arc<Attachment> = build.skins[si]
            .attachment(l.source_slot, &l.source)
            .cloned()
            .ok_or_else(|| LoadError::new("Source mesh not found: ", &l.source))?;
        if source.as_mesh().is_none() {
            return Err(LoadError::new("Source is not a mesh: ", &l.source));
        }
        let own = build.skins[l.skin]
            .attachment_mut(l.slot_index, &l.placeholder)
            .ok_or_else(|| LoadError::new("Linked mesh not found: ", &l.placeholder))?;
        let own = Arc::get_mut(own)
            .ok_or_else(|| LoadError::new("Linked mesh is shared: ", &l.placeholder))?;
        let Attachment::Mesh(mesh) = own else {
            return Err(LoadError::new(
                "Linked attachment is not a mesh: ",
                &l.placeholder,
            ));
        };
        mesh.vertex.base.timeline_attachment = if l.inherit_timelines {
            Some(source.id())
        } else {
            None
        };
        mesh.set_source_mesh(Some(source));
        mesh.update_sequence();
    }
    Ok(())
}

/// Turns a list of draw order changes into the draw order: for each position, the setup index of
/// the slot drawn there. `changes` are `(setup index, offset)` pairs in increasing setup index.
/// An empty change list means the setup order and returns an empty vector.
pub(crate) fn draw_order_from_changes(
    slot_count: usize,
    changes: &[(usize, i32)],
) -> Result<Vec<usize>, LoadError> {
    if changes.is_empty() {
        return Ok(Vec::new());
    }
    let bad = || LoadError::new("Invalid draw order", "");
    let mut order: Vec<i64> = vec![-1; slot_count];
    let mut unchanged: Vec<usize> = Vec::with_capacity(slot_count.saturating_sub(changes.len()));
    let mut original = 0usize;
    for &(slot, offset) in changes {
        if slot > slot_count {
            return Err(bad());
        }
        while original != slot {
            unchanged.push(original);
            original += 1;
        }
        let target = original as i64 + offset as i64;
        if target < 0 || target as usize >= slot_count {
            return Err(bad());
        }
        order[target as usize] = original as i64;
        original += 1;
    }
    while original < slot_count {
        unchanged.push(original);
        original += 1;
    }
    for i in (0..slot_count).rev() {
        if order[i] == -1 {
            order[i] = unchanged.pop().ok_or_else(bad)? as i64;
        }
    }
    Ok(order.into_iter().map(|v| v as usize).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_order_changes() {
        // Move slot 0 two places down in a 4-slot skeleton: [1, 2, 0, 3].
        assert_eq!(draw_order_from_changes(4, &[(0, 2)]).unwrap(), [1, 2, 0, 3]);
        // Swap neighbors 1 and 2.
        assert_eq!(
            draw_order_from_changes(4, &[(1, 1), (2, -1)]).unwrap(),
            [0, 2, 1, 3]
        );
        assert!(draw_order_from_changes(4, &[]).unwrap().is_empty());
        assert!(draw_order_from_changes(2, &[(0, 5)]).is_err());
    }
}
