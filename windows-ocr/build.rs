//! Embed the generated ICO as Windows resources without a resource-compiler dependency.
use std::{env, fs, path::PathBuf};

fn word(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
fn dword(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
fn resource(output: &mut Vec<u8>, kind: u16, id: u16, data: &[u8]) {
    dword(output, data.len() as u32);
    dword(output, 32);
    for value in [0xffff, kind, 0xffff, id] {
        word(output, value);
    }
    dword(output, 0); // DataVersion
    word(output, if kind == 0 { 0 } else { 0x1030 }); // MOVEABLE | PURE | DISCARDABLE
    word(output, 0); // Neutral language
    dword(output, 0);
    dword(output, 0);
    output.extend_from_slice(data);
    output.resize((output.len() + 3) & !3, 0);
}
fn main() {
    println!("cargo:rerun-if-changed=assets/app.ico");
    let mut resources = Vec::new();
    resource(&mut resources, 0, 0, &[]);
    let mut next_id = 1;
    append_icon(&mut resources, "assets/app.ico", 1, &mut next_id);
    for (index, name) in [
        "play", "pause", "add", "delete", "clear", "search", "confirm", "adjust", "save",
    ]
    .iter()
    .enumerate()
    {
        append_icon(
            &mut resources,
            &format!("assets/buttons/{name}.ico"),
            10 + index as u16,
            &mut next_id,
        );
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let path = out.join("app-icon.res");
    fs::write(&path, resources).expect("write application icon resources");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bin=pokemmo-hint={}", path.display());
    } else {
        // GNU linkers require conversion to COFF; the MSVC path needs no extra tools.
        let object = out.join("app-icon.o");
        let compiler = env::var_os("WINDRES").unwrap_or_else(|| "windres".into());
        let status = std::process::Command::new(compiler)
            .args(["-J", "res", "-O", "coff", "-i"])
            .arg(&path)
            .arg("-o")
            .arg(&object)
            .status()
            .expect("run windres for GNU Windows target");
        assert!(status.success(), "windres failed");
        println!("cargo:rustc-link-arg-bin=pokemmo-hint={}", object.display());
    }
}

fn append_icon(resources: &mut Vec<u8>, path: &str, group_id: u16, next_id: &mut u16) {
    println!("cargo:rerun-if-changed={path}");
    let icon = fs::read(path).expect("read icon");
    assert_eq!(&icon[..4], &[0, 0, 1, 0], "expected ICO header");
    let count = u16::from_le_bytes([icon[4], icon[5]]);
    let mut group = icon[..6].to_vec();
    for index in 0..count {
        let start = 6 + index as usize * 16;
        let entry = &icon[start..start + 16];
        let length = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
        let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
        resource(resources, 3, *next_id, &icon[offset..offset + length]);
        group.extend_from_slice(&entry[..12]);
        word(&mut group, *next_id);
        *next_id += 1;
    }
    resource(resources, 14, group_id, &group);
}
