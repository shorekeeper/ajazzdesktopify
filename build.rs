fn main() {
    println!("cargo:rerun-if-changed=src/shaders/ui.vert");
    println!("cargo:rerun-if-changed=src/shaders/ui.frag");
    let out = std::env::var("OUT_DIR").unwrap();
    compile("src/shaders/ui.vert", &format!("{out}/ui.vert.spv"), "vertex");
    compile("src/shaders/ui.frag", &format!("{out}/ui.frag.spv"), "fragment");
}

fn compile(src: &str, dst: &str, stage: &str) {
    let glslc = std::env::var("VULKAN_SDK")
        .map(|sdk| {
            let p = format!("{sdk}/Bin/glslc.exe");
            if std::path::Path::new(&p).exists() { p } else { "glslc".into() }
        })
        .unwrap_or_else(|_| "glslc".into());

    let status = std::process::Command::new(&glslc)
        .args([
            &format!("-fshader-stage={stage}"),
            src,
            "-o", dst,
        ])
        .status()
        .unwrap_or_else(|e| panic!(
            "Cannot run glslc ({glslc}): {e}\n\
             Install Vulkan SDK or add glslc to PATH."
        ));

    assert!(status.success(), "Shader compilation failed: {src}");
}