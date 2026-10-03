#![deny(warnings)]
#![forbid(unsafe_code)]

fn main() {
    let main_source: &'static str = include_str!("../../src/main.rs");
    for required_token in [
        "synchronous_pipeline_compilation: true",
        "TEXTURE_BINDING_ARRAY",
        "BUFFER_BINDING_ARRAY",
        "NoFrustumCulling",
        "FrameCountPlugin",
        "mesh.0 = mesh.0.clone()",
        "assets/000.ktx2",
        "basisu::Transcoder::new",
    ] {
        assert!(
            main_source.contains(required_token),
            "required token missing in src/main.rs: {}",
            required_token
        );
    }
    println!("verified: all required tokens present");
}