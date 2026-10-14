use spine2d_core::atlas::{Atlas, AtlasPage, Format, TextureFilter, TextureLoader};

const ATLAS: &str = "\nsheet.png\nsize:256,128\nformat:RGBA8888\nfilter:Linear,Linear\nrepeat:none\npma:true\nhead\nbounds:10,20,30,40\noffsets:1,2,50,60\nrotate:true\nindex:-1\nsplit:1,2,3,4\n\
body\nxy:0,0\nsize:64,32\n";

#[test]
fn parses_pages_and_regions() {
    let atlas = Atlas::parse(ATLAS, "tex", None);
    assert_eq!(atlas.pages().len(), 1);
    let p = &atlas.pages()[0];
    assert_eq!(
        (p.name.as_str(), p.width, p.height),
        ("sheet.png", 256, 128)
    );
    assert_eq!(p.texture_path, "tex/sheet.png");
    assert_eq!(
        (p.format, p.min_filter, p.pma),
        (Format::Rgba8888, TextureFilter::Linear, true)
    );

    let head = atlas.find_region("head").unwrap();
    let i = head.region.atlas.as_ref().unwrap();
    assert_eq!((i.x, i.y, i.index), (10, 20, -1));
    assert_eq!(
        (i.offset_x, i.offset_y, i.original_width, i.original_height),
        (1.0, 2.0, 50, 60)
    );
    // Rotated 90: packed size is swapped *after* the UVs use the unswapped sizes.
    assert_eq!((i.degrees, i.rotate), (90, true));
    assert_eq!((i.packed_width, i.packed_height), (40, 30));
    assert_eq!(head.region.u, 10.0 / 256.0);
    assert_eq!(head.region.u2, (10.0 + 40.0) / 256.0);
    assert_eq!(head.region.v2, (20.0 + 30.0) / 128.0);
    assert_eq!(head.names, ["split"]);
    assert_eq!(head.values, [1.0, 2.0, 3.0, 4.0]);

    // Without orig/offsets, the original size defaults to the packed size.
    let body = atlas
        .find_region("body")
        .unwrap()
        .region
        .atlas
        .clone()
        .unwrap();
    assert_eq!((body.original_width, body.original_height), (64, 32));
    assert!(atlas.find_region("nope").is_none());
}

#[test]
fn texture_loader_can_override_page_size_before_uvs() {
    struct Fixed;
    impl TextureLoader for Fixed {
        fn load(&mut self, page: &mut AtlasPage, _path: &str) {
            page.width = 1024;
            page.height = 1024;
            page.texture = 7;
        }
        fn unload(&mut self, _texture: usize) {}
    }
    let atlas = Atlas::parse(ATLAS, "", Some(&mut Fixed));
    let body = atlas.find_region("body").unwrap();
    assert_eq!(body.region.u2, 64.0 / 1024.0);
    assert_eq!(body.region.renderer_object, 7);
    assert_eq!(atlas.pages()[0].texture_path, "sheet.png");
}

#[test]
fn header_lines_crlf_and_flip_v() {
    let text = "pma: true\r\n\r\nsheet.png\r\nsize: 100, 100\r\nr\r\nbounds: 0, 0, 50, 50\r\n";
    let mut atlas = Atlas::parse(text, "dir/", None);
    let r = atlas.find_region("r").unwrap();
    assert_eq!((r.region.u2, r.region.v2), (0.5, 0.5));
    assert_eq!(atlas.pages()[0].texture_path, "dir/sheet.png");
    atlas.flip_v();
    let r = atlas.find_region("r").unwrap();
    assert_eq!((r.region.v, r.region.v2), (1.0, 0.5));
}
