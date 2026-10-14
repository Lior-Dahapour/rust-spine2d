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

use crate::atlas::Atlas;
use crate::attachment::{
    BoundingBoxAttachment, ClippingAttachment, MeshAttachment, PathAttachment, PointAttachment,
    RegionAttachment,
};
use crate::attachment_loader::AttachmentLoader;
use crate::sequence::Sequence;
use crate::skin::Skin;

/// Creates attachments whose texture regions are looked up in an [`Atlas`].
pub struct AtlasAttachmentLoader<'a> {
    atlas: &'a Atlas,
}

impl<'a> AtlasAttachmentLoader<'a> {
    pub fn new(atlas: &'a Atlas) -> Self {
        AtlasAttachmentLoader { atlas }
    }

    /// Fills the sequence's regions from the atlas by `path` (plus frame suffixes).
    fn find_regions(&self, path: &str, sequence: &mut Sequence) {
        for i in 0..sequence.regions.len() {
            let frame_path = sequence.path(path, i as i32);
            sequence.regions[i] = self
                .atlas
                .find_region(&frame_path)
                .map(|r| r.region.clone());
        }
    }
}

impl AttachmentLoader for AtlasAttachmentLoader<'_> {
    fn new_region_attachment(
        &self,
        _skin: &Skin,
        _placeholder: &str,
        name: &str,
        path: &str,
        mut sequence: Sequence,
    ) -> Option<RegionAttachment> {
        self.find_regions(path, &mut sequence);
        Some(RegionAttachment::new(name, sequence))
    }

    fn new_mesh_attachment(
        &self,
        _skin: &Skin,
        _placeholder: &str,
        name: &str,
        path: &str,
        mut sequence: Sequence,
    ) -> Option<MeshAttachment> {
        self.find_regions(path, &mut sequence);
        Some(MeshAttachment::new(name, sequence))
    }

    fn new_bounding_box_attachment(
        &self,
        _skin: &Skin,
        _placeholder: &str,
        name: &str,
    ) -> Option<BoundingBoxAttachment> {
        Some(BoundingBoxAttachment::new(name))
    }

    fn new_path_attachment(
        &self,
        _skin: &Skin,
        _placeholder: &str,
        name: &str,
    ) -> Option<PathAttachment> {
        Some(PathAttachment::new(name))
    }

    fn new_point_attachment(
        &self,
        _skin: &Skin,
        _placeholder: &str,
        name: &str,
    ) -> Option<PointAttachment> {
        Some(PointAttachment::new(name))
    }

    fn new_clipping_attachment(
        &self,
        _skin: &Skin,
        _placeholder: &str,
        name: &str,
    ) -> Option<ClippingAttachment> {
        Some(ClippingAttachment::new(name))
    }
}
