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

//! Texture atlas parsing (the libgdx `.atlas` text format).

use crate::texture_region::{AtlasRegionInfo, TextureRegion};
use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Format {
    Alpha,
    Intensity,
    LuminanceAlpha,
    Rgb565,
    Rgba4444,
    Rgb888,
    #[default]
    Rgba8888,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextureFilter {
    Unknown,
    #[default]
    Nearest,
    Linear,
    MipMap,
    MipMapNearestNearest,
    MipMapLinearNearest,
    MipMapNearestLinear,
    MipMapLinearLinear,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextureWrap {
    MirroredRepeat,
    #[default]
    ClampToEdge,
    Repeat,
}

const FORMAT_NAMES: [&str; 8] = [
    "",
    "Alpha",
    "Intensity",
    "LuminanceAlpha",
    "RGB565",
    "RGBA4444",
    "RGB888",
    "RGBA8888",
];
const FILTER_NAMES: [&str; 8] = [
    "",
    "Nearest",
    "Linear",
    "MipMap",
    "MipMapNearestNearest",
    "MipMapLinearNearest",
    "MipMapNearestLinear",
    "MipMapLinearLinear",
];

/// Unknown names map to index 0, like the reference.
fn index_of(names: &[&str], name: &str) -> usize {
    names.iter().position(|n| *n == name).unwrap_or(0)
}

fn format_from_index(i: usize) -> Format {
    [
        Format::Alpha, // index 0 is "unknown"; the reference casts it straight to the enum
        Format::Alpha,
        Format::Intensity,
        Format::LuminanceAlpha,
        Format::Rgb565,
        Format::Rgba4444,
        Format::Rgb888,
        Format::Rgba8888,
    ][i]
}

fn filter_from_index(i: usize) -> TextureFilter {
    [
        TextureFilter::Unknown,
        TextureFilter::Nearest,
        TextureFilter::Linear,
        TextureFilter::MipMap,
        TextureFilter::MipMapNearestNearest,
        TextureFilter::MipMapLinearNearest,
        TextureFilter::MipMapNearestLinear,
        TextureFilter::MipMapLinearLinear,
    ][i]
}

/// A texture file that atlas regions are packed into.
#[derive(Clone, Debug, Default)]
pub struct AtlasPage {
    pub name: SmolStr,
    pub texture_path: SmolStr,
    pub format: Format,
    pub min_filter: TextureFilter,
    pub mag_filter: TextureFilter,
    pub u_wrap: TextureWrap,
    pub v_wrap: TextureWrap,
    pub width: i32,
    pub height: i32,
    /// True if the texture has premultiplied alpha.
    pub pma: bool,
    pub index: usize,
    /// Opaque renderer texture handle set by the [`TextureLoader`] (0 = none).
    pub texture: usize,
}

/// A region of an atlas page.
#[derive(Clone, Debug)]
pub struct AtlasRegion {
    pub region: Arc<TextureRegion>,
    /// Index into [`Atlas::pages`].
    pub page: usize,
    /// Names and values of region entries the parser doesn't interpret (eg `split`, `pad`).
    pub names: Vec<SmolStr>,
    pub values: Vec<f32>,
}

impl AtlasRegion {
    pub fn name(&self) -> &str {
        self.region.atlas.as_ref().map_or("", |a| a.name.as_str())
    }
}

/// Loads the texture for an atlas page. The loader may change the page's `width` and `height`
/// (eg to the real texture size); region UVs are computed afterwards from the final values.
pub trait TextureLoader {
    fn load(&mut self, page: &mut AtlasPage, path: &str);
    fn unload(&mut self, texture: usize);
}

#[derive(Clone, Debug, Default)]
pub struct Atlas {
    pages: Vec<AtlasPage>,
    regions: Vec<AtlasRegion>,
    by_name: FxHashMap<SmolStr, usize>,
}

impl Atlas {
    /// Parses atlas text. `dir` is prepended to page names to form texture paths. If `loader` is
    /// given it is called for every page.
    pub fn parse(text: &str, dir: &str, loader: Option<&mut dyn TextureLoader>) -> Atlas {
        let mut atlas = Atlas::default();
        atlas.load(text, dir, loader);
        atlas.by_name.reserve(atlas.regions.len());
        for (i, r) in atlas.regions.iter().enumerate() {
            atlas.by_name.entry(r.name().into()).or_insert(i);
        }
        atlas
    }

    /// Reads and parses an atlas file; the texture directory is the file's directory.
    pub fn from_file(path: &str, loader: Option<&mut dyn TextureLoader>) -> std::io::Result<Atlas> {
        let text = std::fs::read_to_string(path)?;
        let last_slash = path.rfind(['/', '\\']);
        let dir = match last_slash {
            Some(0) => &path[..1], // never drop a starting slash
            Some(i) => &path[..i],
            None => "",
        };
        Ok(Atlas::parse(&text, dir, loader))
    }

    pub fn pages(&self) -> &[AtlasPage] {
        &self.pages
    }

    pub fn regions(&self) -> &[AtlasRegion] {
        &self.regions
    }

    /// The first region with the given name.
    pub fn find_region(&self, name: &str) -> Option<&AtlasRegion> {
        self.by_name.get(name).map(|&i| &self.regions[i])
    }

    /// Flips the V coordinates of all regions, for renderers with a flipped texture origin.
    pub fn flip_v(&mut self) {
        for r in &mut self.regions {
            let region = Arc::make_mut(&mut r.region);
            region.v = 1.0 - region.v;
            region.v2 = 1.0 - region.v2;
        }
    }

    /// Unloads the textures of all pages.
    pub fn unload(&self, loader: &mut dyn TextureLoader) {
        for page in &self.pages {
            loader.unload(page.texture);
        }
    }

    fn load(&mut self, text: &str, dir: &str, mut loader: Option<&mut dyn TextureLoader>) {
        let needs_slash = !dir.is_empty() && !dir.ends_with('/') && !dir.ends_with('\\');
        let mut reader = Reader { text, pos: 0 };

        // Leading "key: value" header lines are skipped.
        let mut line = reader.read_line();
        while let Some(l) = line {
            if read_entry(l).is_none() {
                break;
            }
            line = reader.read_line();
        }

        let mut page_index: Option<usize> = None;
        while let Some(l) = line {
            if l.is_empty() {
                page_index = None;
                line = reader.read_line();
            } else if page_index.is_none() {
                let mut page = AtlasPage {
                    name: l.into(),
                    ..Default::default()
                };
                let path = format!("{dir}{}{l}", if needs_slash { "/" } else { "" });
                loop {
                    line = reader.read_line();
                    let Some(entry) = line.and_then(read_entry) else {
                        break;
                    };
                    let v = &entry.values;
                    match entry.key {
                        "size" => {
                            page.width = to_int(v[0]);
                            page.height = to_int(v[1]);
                        }
                        "format" => page.format = format_from_index(index_of(&FORMAT_NAMES, v[0])),
                        "filter" => {
                            page.min_filter = filter_from_index(index_of(&FILTER_NAMES, v[0]));
                            page.mag_filter = filter_from_index(index_of(&FILTER_NAMES, v[1]));
                        }
                        "repeat" => {
                            page.u_wrap = if v[0].contains('x') {
                                TextureWrap::Repeat
                            } else {
                                TextureWrap::ClampToEdge
                            };
                            page.v_wrap = if v[0].contains('y') {
                                TextureWrap::Repeat
                            } else {
                                TextureWrap::ClampToEdge
                            };
                        }
                        "pma" => page.pma = v[0] == "true",
                        _ => {}
                    }
                }
                page.index = self.pages.len();
                if let Some(loader) = loader.as_deref_mut() {
                    loader.load(&mut page, &path);
                }
                page.texture_path = path.into();
                self.pages.push(page);
                page_index = Some(self.pages.len() - 1);
            } else {
                let pi = page_index.expect("checked above");
                let region = self.read_region(&mut reader, &mut line, l, pi);
                self.regions.push(region);
            }
        }
    }

    fn read_region<'a>(
        &self,
        reader: &mut Reader<'a>,
        line: &mut Option<&'a str>,
        name: &str,
        page_index: usize,
    ) -> AtlasRegion {
        let page = &self.pages[page_index];
        let mut info = AtlasRegionInfo {
            name: name.into(),
            page_width: page.width,
            page_height: page.height,
            ..Default::default()
        };
        let mut names = Vec::new();
        let mut values = Vec::new();
        loop {
            *line = reader.read_line();
            let Some(entry) = line.and_then(read_entry) else {
                break;
            };
            let v = &entry.values;
            match entry.key {
                "xy" => {
                    info.x = to_int(v[0]);
                    info.y = to_int(v[1]);
                }
                "size" => {
                    info.packed_width = to_int(v[0]);
                    info.packed_height = to_int(v[1]);
                }
                "bounds" => {
                    info.x = to_int(v[0]);
                    info.y = to_int(v[1]);
                    info.packed_width = to_int(v[2]);
                    info.packed_height = to_int(v[3]);
                }
                "offset" => {
                    info.offset_x = to_int(v[0]) as f32;
                    info.offset_y = to_int(v[1]) as f32;
                }
                "orig" => {
                    info.original_width = to_int(v[0]);
                    info.original_height = to_int(v[1]);
                }
                "offsets" => {
                    info.offset_x = to_int(v[0]) as f32;
                    info.offset_y = to_int(v[1]) as f32;
                    info.original_width = to_int(v[2]);
                    info.original_height = to_int(v[3]);
                }
                "rotate" => {
                    if v[0] == "true" {
                        info.degrees = 90;
                    } else if v[0] != "false" {
                        info.degrees = to_int(v[0]);
                    }
                    info.rotate = info.degrees == 90;
                }
                "index" => info.index = to_int(v[0]),
                key => {
                    names.push(key.into());
                    for s in v.iter().take(entry.count) {
                        values.push(to_int(s) as f32);
                    }
                }
            }
        }
        if info.original_width == 0 && info.original_height == 0 {
            info.original_width = info.packed_width;
            info.original_height = info.packed_height;
        }

        let (pw, ph) = (page.width as f32, page.height as f32);
        let u = info.x as f32 / pw;
        let v = info.y as f32 / ph;
        let (u2, v2) = if info.degrees == 90 {
            (
                (info.x + info.packed_height) as f32 / pw,
                (info.y + info.packed_width) as f32 / ph,
            )
        } else {
            (
                (info.x + info.packed_width) as f32 / pw,
                (info.y + info.packed_height) as f32 / ph,
            )
        };
        let region_width = ((u2 - u) * page.width as f32) as i32;
        let region_height = ((v2 - v) * page.height as f32) as i32;
        if info.degrees == 90 {
            std::mem::swap(&mut info.packed_width, &mut info.packed_height);
        }
        AtlasRegion {
            region: Arc::new(TextureRegion {
                renderer_object: page.texture,
                u,
                v,
                u2,
                v2,
                region_width: region_width.abs(),
                region_height: region_height.abs(),
                atlas: Some(info),
            }),
            page: page_index,
            names,
            values,
        }
    }
}

/// C `isspace`.
fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\x0b' | '\x0c' | '\r')
}

/// Strips leading whitespace and trailing carriage returns (but not trailing spaces), like the
/// reference.
fn trim(s: &str) -> &str {
    s.trim_start_matches(is_space).trim_end_matches('\r')
}

struct Reader<'a> {
    text: &'a str,
    pos: usize,
}

impl<'a> Reader<'a> {
    fn read_line(&mut self) -> Option<&'a str> {
        if self.pos >= self.text.len() {
            return None;
        }
        let rest = &self.text[self.pos..];
        let end = rest.find('\n').unwrap_or(rest.len());
        self.pos += (end + 1).min(rest.len());
        Some(trim(&rest[..end]))
    }
}

struct Entry<'a> {
    key: &'a str,
    /// Up to four comma-separated values; unused slots are empty.
    values: [&'a str; 4],
    count: usize,
}

/// Parses `key: v1, v2, v3, v4`; `None` if the line is empty or has no colon.
fn read_entry(line: &str) -> Option<Entry<'_>> {
    let line = trim(line);
    if line.is_empty() {
        return None;
    }
    let colon = line.find(':')?;
    let key = trim(&line[..colon]);
    let mut values = [""; 4];
    let mut last = colon + 1;
    let mut count = 0;
    loop {
        count += 1;
        match line[last..].find(',') {
            None => {
                values[count - 1] = trim(&line[last..]);
                return Some(Entry { key, values, count });
            }
            Some(rel) => {
                values[count - 1] = trim(&line[last..last + rel]);
                last += rel + 1;
                if count == 4 {
                    return Some(Entry { key, values, count });
                }
            }
        }
    }
}

/// C `strtol(s, NULL, 10)`: leading whitespace, optional sign, digits; 0 if none.
fn to_int(s: &str) -> i32 {
    let s = s.trim_start_matches(is_space);
    let (neg, digits) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let n: i64 = digits
        .bytes()
        .take_while(u8::is_ascii_digit)
        .fold(0i64, |a, d| {
            a.saturating_mul(10).saturating_add((d - b'0') as i64)
        });
    (if neg { -n } else { n }) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_int_matches_strtol() {
        assert_eq!(to_int(" 42"), 42);
        assert_eq!(to_int("-7abc"), -7);
        assert_eq!(to_int("x"), 0);
        assert_eq!(to_int("+5"), 5);
    }
}
