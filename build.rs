use std::{
    collections::BTreeMap,
    env,
    fmt::Write,
    fs,
    path::{Path, PathBuf},
};

const SHARED_DATA: &[&str] = &[
    "ALS/Animations/als_notify_manifest.json",
    "ALS/Character/als_character_curves.runtime.json",
    "ALS/Character/overlay_actor.runtime.json",
    "ALS/Data/Footstep/footprint.runtime.json",
    "ALS/Data/als_gameplay_tags.json",
    "ALSCamera/als_camera_curves.runtime.json",
];

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=src/animation_graph_ext/acl_decode.cpp");
    println!("cargo::rerun-if-changed=vendor/acl/includes");
    println!("cargo::rerun-if-changed=vendor/rtm/includes");
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .include("vendor/acl/includes")
        .include("vendor/rtm/includes")
        .file("src/animation_graph_ext/acl_decode.cpp")
        .compile("als_acl_decode");

    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("assets");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let mut files = BTreeMap::new();
    for name in ["ALS", "ALSCamera", "ALSRuntime"] {
        println!("cargo::rerun-if-changed=assets/{name}");
        collect_assets(&root, &root.join(name), &mut files);
    }
    for &path in SHARED_DATA {
        let source = root.join(path);
        assert!(source.is_file(), "Missing packaged ALS data: {path}");
        files.insert(path.to_owned(), source);
    }

    let mut table = String::from("&[\n");
    for (index, (path, source)) in files.iter().enumerate() {
        let compiled = if path.ends_with(".ron") {
            let text = fs::read_to_string(source).unwrap();
            let rewritten = embedded_references(&text, &files);
            if rewritten != text {
                let destination = output.join(format!("als-{index}.ron"));
                fs::write(&destination, rewritten).unwrap();
                destination
            } else {
                source.clone()
            }
        } else {
            source.clone()
        };
        writeln!(
            table,
            "    ({path:?}, {compiled:?}, include_bytes!({compiled:?})),"
        )
        .unwrap();
    }
    table.push_str("]\n");
    fs::write(output.join("als_assets.rs"), table).unwrap();
}

fn collect_assets(root: &Path, folder: &Path, files: &mut BTreeMap<String, PathBuf>) {
    for entry in fs::read_dir(folder).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if entry.file_type().unwrap().is_dir() {
            collect_assets(root, &path, files);
        } else if matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("ron" | "glb" | "tga")
        ) || path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".notify.json") || name.ends_with(".montage.json"))
        {
            let key = path
                .strip_prefix(root)
                .unwrap()
                .components()
                .map(|part| part.as_os_str().to_str().unwrap())
                .collect::<Vec<_>>()
                .join("/");
            files.insert(key, path);
        }
    }
}

fn embedded_references(text: &str, files: &BTreeMap<String, PathBuf>) -> String {
    // Only rewrite installed asset paths. Source names and RON type paths stay intact.
    let mut parts = text.split('"');
    let mut result = String::with_capacity(text.len());
    result.push_str(parts.next().unwrap_or_default());
    for part in parts {
        result.push('"');
        let path = part.split('#').next().unwrap_or_default();
        if files.contains_key(path) {
            result.push_str("embedded://");
        }
        result.push_str(part);
    }
    result
}
