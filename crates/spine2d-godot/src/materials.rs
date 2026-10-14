//! Canvas materials for the four Spine blend modes, in plain, premultiplied-alpha and two-color
//! variants. They are created lazily through the `RenderingServer` and shared by every sprite.

use godot::builtin::Rid;
use godot::classes::RenderingServer;
use godot::obj::Singleton;
use spine2d_core::BlendMode;
use std::cell::RefCell;
use std::collections::HashMap;

/// The properties of a texture and slot that decide which material draws a batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MaterialKey {
    pub blend_mode: u8,
    pub premultiplied_alpha: bool,
    pub two_color: bool,
}

impl MaterialKey {
    pub fn new(blend_mode: BlendMode, premultiplied_alpha: bool, two_color: bool) -> Self {
        MaterialKey {
            blend_mode: blend_mode as u8,
            premultiplied_alpha,
            two_color,
        }
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

thread_local! {
    // Godot objects may only be used from the main thread, which is also the only thread that
    // renders, so a thread-local cache is enough.
    static MATERIALS: RefCell<HashMap<MaterialKey, (Rid, Rid)>> = RefCell::new(HashMap::new());
}

/// The material that draws `key`, creating it on first use.
pub fn material(key: MaterialKey) -> Rid {
    MATERIALS
        .with(|cache| {
            *cache.borrow_mut().entry(key).or_insert_with(|| {
                let mut rs = RenderingServer::singleton();
                let shader = rs.shader_create();
                rs.shader_set_code(shader, shader_code(key).as_str());
                let material = rs.material_create();
                rs.material_set_shader(material, shader);
                (shader, material)
            })
        })
        .1
}

thread_local! {
    // The alpha mode of textures that scripts attach to a skeleton after it was loaded. The
    // textures of the skeleton's own atlas are known to the sprite and are not in here.
    static TEXTURE_ALPHA: RefCell<HashMap<usize, bool>> = RefCell::new(HashMap::new());
}

/// Records whether the texture with the render server handle `handle` has premultiplied alpha.
pub fn register_texture_alpha(handle: usize, premultiplied: bool) {
    TEXTURE_ALPHA.with(|alpha| alpha.borrow_mut().insert(handle, premultiplied));
}

/// Whether a texture registered with [`register_texture_alpha`] is premultiplied; `None` if it was
/// never registered.
pub fn registered_texture_alpha(handle: usize) -> Option<bool> {
    TEXTURE_ALPHA.with(|alpha| alpha.borrow().get(&handle).copied())
}

/// Frees every cached material. Call when the extension unloads.
pub fn free_all() {
    MATERIALS.with(|cache| {
        let mut rs = RenderingServer::singleton();
        for (_, (shader, material)) in cache.borrow_mut().drain() {
            rs.free_rid(material);
            rs.free_rid(shader);
        }
    });
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
