//! Canvas materials for the four Spine blend modes, in plain, premultiplied-alpha and two-color
//! variants. They are created lazily through the `RenderingServer` and shared by every sprite.

use bevy_ecs::resource::Resource;
use godot::builtin::Rid;
use godot::classes::RenderingServer;
use godot::obj::Singleton;
use rustc_hash::FxHashMap;
use spine2d_core::BlendMode;

/// The properties of a texture and slot that decide which material draws a batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MaterialKey {
    pub blend_mode: u8,
    pub premultiplied_alpha: bool,
    pub two_color: bool,
}

impl MaterialKey {
    /// How many distinct keys there are: four blend modes, each plain or premultiplied, with or
    /// without two colors.
    pub const COUNT: usize = 16;

    pub fn new(blend_mode: BlendMode, premultiplied_alpha: bool, two_color: bool) -> Self {
        MaterialKey {
            blend_mode: blend_mode as u8,
            premultiplied_alpha,
            two_color,
        }
    }

    /// The position of this key among all [`COUNT`](Self::COUNT) of them.
    pub fn index(self) -> usize {
        usize::from(self.blend_mode) * 4
            + usize::from(self.premultiplied_alpha) * 2
            + usize::from(self.two_color)
    }

    fn blend(self) -> BlendMode {
        match self.blend_mode {
            1 => BlendMode::Additive,
            2 => BlendMode::Multiply,
            3 => BlendMode::Screen,
            _ => BlendMode::Normal,
        }
    }
}

/// The shared materials, created lazily through the `RenderingServer`. A resource of the world,
/// which frees them when it is dropped.
#[derive(Resource, Default)]
pub struct Materials {
    /// The (shader, material) pair of each key, by [`MaterialKey::index`].
    slots: [Option<(Rid, Rid)>; MaterialKey::COUNT],
}

impl Materials {
    /// The material that draws `key`, creating it on first use.
    pub fn get(&mut self, key: MaterialKey) -> Rid {
        self.slots[key.index()]
            .get_or_insert_with(|| {
                let mut rs = RenderingServer::singleton();
                let shader = rs.shader_create();
                rs.shader_set_code(shader, shader_code(key).as_str());
                let material = rs.material_create();
                rs.material_set_shader(material, shader);
                (shader, material)
            })
            .1
    }
}

impl Drop for Materials {
    fn drop(&mut self) {
        if self.slots.iter().all(Option::is_none) {
            return;
        }
        let mut rs = RenderingServer::singleton();
        for (shader, material) in self.slots.iter_mut().filter_map(Option::take) {
            rs.free_rid(material);
            rs.free_rid(shader);
        }
    }
}

/// The alpha mode of textures that scripts attach to a skeleton after it was loaded, by render
/// server handle. The textures of the skeleton's own atlas are known to the sprite and are not in
/// here. A resource of the world.
#[derive(Resource, Default)]
pub struct TextureAlpha {
    premultiplied: FxHashMap<usize, bool>,
}

impl TextureAlpha {
    /// Records whether the texture with the render server handle `handle` has premultiplied alpha.
    pub fn register(&mut self, handle: usize, premultiplied: bool) {
        self.premultiplied.insert(handle, premultiplied);
    }

    /// Whether a texture registered with [`register`](Self::register) is premultiplied; `None` if
    /// it was never registered.
    pub fn get(&self, handle: usize) -> Option<bool> {
        self.premultiplied.get(&handle).copied()
    }
}

/// A premultiplied pixel never has RGB above its alpha. Godot's default texture import ("Fix Alpha
/// Border") writes neighbouring colors into fully transparent pixels, which would otherwise be
/// added to the screen as speckles and fringes.
const CLAMP_PREMULTIPLIED: &str = "t.rgb = min(t.rgb, vec3(t.a));";

/// Godot has no Screen blend, so it falls back to additive (the official plugin maps it to
/// subtract, which is the wrong direction).
fn render_mode(blend: BlendMode, premultiplied_alpha: bool) -> &'static str {
    match blend {
        BlendMode::Normal if premultiplied_alpha => "blend_premul_alpha",
        BlendMode::Normal => "blend_mix",
        BlendMode::Additive | BlendMode::Screen => "blend_add",
        BlendMode::Multiply => "blend_mul",
    }
}

/// Source of the canvas shader for `key`.
pub fn shader_code(key: MaterialKey) -> String {
    let blend = key.blend();
    let mode = render_mode(blend, key.premultiplied_alpha);
    let additive = matches!(blend, BlendMode::Additive | BlendMode::Screen);
    let normal_pma = key.premultiplied_alpha && blend == BlendMode::Normal;
    let header = format!("shader_type canvas_item;\nrender_mode {mode};\n");

    if key.two_color {
        let (vertex, tail) = if key.premultiplied_alpha {
            (
                "light_color = vec4(CUSTOM1.rgb * CUSTOM1.a, CUSTOM1.a);\n\t\tdark_color = CUSTOM0.rgb * CUSTOM1.a;",
                if additive {
                    "COLOR = vec4((t.a - t.rgb) * dark_color + t.rgb * light_color.rgb, 1.0);"
                } else {
                    "COLOR = vec4((t.a - t.rgb) * dark_color + t.rgb * light_color.rgb, t.a * light_color.a);"
                },
            )
        } else {
            (
                "light_color = CUSTOM1;\n\t\tdark_color = CUSTOM0.rgb;",
                "COLOR = vec4(t.rgb * light_color.rgb + (vec3(1.0) - t.rgb) * dark_color, t.a * light_color.a);",
            )
        };
        return format!(
            "{header}varying vec4 light_color;\nvarying vec3 dark_color;\nvoid vertex() {{\n\t\t{vertex}\n}}\nvoid fragment() {{\n\tvec4 t = texture(TEXTURE, UV);\n\t{clamp}{tail}\n}}\n",
            clamp = if key.premultiplied_alpha { format!("{CLAMP_PREMULTIPLIED}\n\t") } else { String::new() }
        );
    }
    if !key.premultiplied_alpha {
        return header;
    }
    // In a canvas fragment shader COLOR already holds vertex color * texture, so the plain vertex
    // tint is captured in vertex() (where Godot has applied the node's modulate) and used here.
    let body = if normal_pma {
        "COLOR = t * vec4(tint.rgb * tint.a, tint.a);"
    } else if additive {
        "COLOR = vec4(t.rgb * tint.rgb * tint.a, 1.0);"
    } else {
        "t.rgb = t.a > 0.0 ? t.rgb / t.a : vec3(0.0);\n\tCOLOR = t * tint;"
    };
    format!(
        "{header}varying vec4 tint;\nvoid vertex() {{\n\ttint = COLOR;\n}}\nvoid fragment() {{\n\tvec4 t = texture(TEXTURE, UV);\n\t{CLAMP_PREMULTIPLIED}\n\t{body}\n}}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_materials_need_no_custom_code() {
        let code = shader_code(MaterialKey::new(BlendMode::Normal, false, false));
        assert!(code.contains("blend_mix") && !code.contains("fragment"));
    }

    #[test]
    fn premultiplied_normal_uses_premul_blend() {
        let code = shader_code(MaterialKey::new(BlendMode::Normal, true, false));
        assert!(
            code.contains("blend_premul_alpha")
                && code.contains("tint.a")
                && !code.contains("COLOR.a")
        );
    }

    #[test]
    fn two_color_reads_custom_attributes() {
        let code = shader_code(MaterialKey::new(BlendMode::Additive, false, true));
        assert!(code.contains("blend_add") && code.contains("CUSTOM0") && code.contains("CUSTOM1"));
    }

    #[test]
    fn screen_falls_back_to_additive() {
        assert!(
            shader_code(MaterialKey::new(BlendMode::Screen, false, false)).contains("blend_add")
        );
    }

    #[test]
    fn every_key_has_its_own_slot() {
        let mut seen = [false; MaterialKey::COUNT];
        for mode in [
            BlendMode::Normal,
            BlendMode::Additive,
            BlendMode::Multiply,
            BlendMode::Screen,
        ] {
            for premultiplied in [false, true] {
                for two_color in [false, true] {
                    let index = MaterialKey::new(mode, premultiplied, two_color).index();
                    assert!(!seen[index], "slot {index} used twice");
                    seen[index] = true;
                }
            }
        }
        assert!(seen.iter().all(|used| *used));
    }

    #[test]
    fn texture_alpha_remembers_what_was_registered() {
        let mut alpha = TextureAlpha::default();
        alpha.register(7, true);
        alpha.register(9, false);

        assert_eq!(alpha.get(7), Some(true));
        assert_eq!(alpha.get(9), Some(false));
        assert_eq!(alpha.get(8), None);
    }

    #[test]
    fn key_round_trips_blend_mode() {
        for mode in [
            BlendMode::Normal,
            BlendMode::Additive,
            BlendMode::Multiply,
            BlendMode::Screen,
        ] {
            assert_eq!(MaterialKey::new(mode, false, false).blend(), mode);
        }
    }
}
