// Splits JSON load time into: file read, serde_json parse, and the rest (our loader).
use spine2d_core::*;
use std::time::Instant;
struct Headless;
impl TextureLoader for Headless {
    fn load(&mut self, page: &mut AtlasPage, _p: &str) {
        page.width = 1024;
        page.height = 1024;
    }
    fn unload(&mut self, _t: usize) {}
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let atlas = Atlas::from_file(&a[2], Some(&mut Headless)).unwrap();
    let loader = AtlasAttachmentLoader::new(&atlas);
    let n = 40;
    let text = std::fs::read_to_string(&a[1]).unwrap();
    let t = Instant::now();
    for _ in 0..n {
        std::hint::black_box(json::parse(&text).unwrap());
    }
    let parse = t.elapsed().as_secs_f64() * 1e6 / n as f64;
    let t = Instant::now();
    for _ in 0..n {
        std::hint::black_box(
            SkeletonJson::new(&loader)
                .read_skeleton_data(&text)
                .unwrap(),
        );
    }
    let full = t.elapsed().as_secs_f64() * 1e6 / n as f64;
    println!(
        "size={}KB parse_only={parse:.0}us full={full:.0}us -> serde_json parse is {:.0}% of load",
        text.len() / 1024,
        parse / full * 100.0
    );
}
