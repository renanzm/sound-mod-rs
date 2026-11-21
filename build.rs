//! Grava o icone e as informacoes de versao dentro do SoundMod.exe.
//!
//! Sem rc.exe/windres: monta o arquivo de recursos (.res) aqui mesmo e passa para o linker
//! (rust-lld e link.exe aceitam .res direto). O desenho do icone vem de src/icon_raster.rs.

#[allow(dead_code)]
#[path = "src/icon_raster.rs"]
mod icon_raster;

use std::path::PathBuf;

const RT_ICON: u16 = 3;
const RT_VERSION: u16 = 16;
const RT_GROUP_ICON: u16 = 14;
const SIZES: [usize; 6] = [16, 24, 32, 48, 64, 256];

fn main() {
    println!("cargo:rerun-if-changed=src/icon_raster.rs");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = Vec::new();
    entry(&mut res, 0, 0, &[]); // o formato .res comeca com uma entrada vazia
    let mut group = vec![0, 0, 1, 0, SIZES.len() as u8, 0]; // GRPICONDIR: reservado, tipo=1, quantidade
    for (i, &n) in SIZES.iter().enumerate() {
        let id = i as u16 + 1;
        let image = dib(n, &icon_raster::rgba(n, icon_raster::GREEN));
        entry(&mut res, RT_ICON, id, &image);
        let side = if n >= 256 { 0 } else { n as u8 };
        group.extend_from_slice(&[side, side, 0, 0]);
        group.extend_from_slice(&1u16.to_le_bytes()); // planos
        group.extend_from_slice(&32u16.to_le_bytes()); // bits por pixel
        group.extend_from_slice(&(image.len() as u32).to_le_bytes());
        group.extend_from_slice(&id.to_le_bytes());
    }
    entry(&mut res, RT_GROUP_ICON, 1, &group);
    entry(&mut res, RT_VERSION, 1, &version_info());

    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("soundmod.res");
    std::fs::write(&out, res).expect("gravar .res");
    println!("cargo:rustc-link-arg-bins={}", out.display());
}

/// Uma entrada do .res com tipo e nome numericos.
fn entry(out: &mut Vec<u8>, kind: u16, name: u16, data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&32u32.to_le_bytes()); // tamanho do cabecalho
    out.extend_from_slice(&[0xFF, 0xFF]);
    out.extend_from_slice(&kind.to_le_bytes());
    out.extend_from_slice(&[0xFF, 0xFF]);
    out.extend_from_slice(&name.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // DataVersion
    let flags: u16 = if kind == 0 { 0 } else { 0x1030 }; // MOVEABLE | PURE | DISCARDABLE
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&(if kind == 0 { 0u16 } else { 0x0409 }).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // Version
    out.extend_from_slice(&0u32.to_le_bytes()); // Characteristics
    out.extend_from_slice(data);
    while out.len() % 4 != 0 {
        out.push(0);
    }
}

/// Imagem de icone em DIB 32 bits (BGRA de baixo para cima + mascara AND vazia).
fn dib(n: usize, rgba: &[u8]) -> Vec<u8> {
    let mask_row = n.div_ceil(32) * 4;
    let mut v = Vec::with_capacity(40 + n * n * 4 + mask_row * n);
    for x in [40u32, n as u32, 2 * n as u32] {
        v.extend_from_slice(&x.to_le_bytes());
    }
    v.extend_from_slice(&1u16.to_le_bytes()); // planos
    v.extend_from_slice(&32u16.to_le_bytes());
    v.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    v.extend_from_slice(&((n * n * 4 + mask_row * n) as u32).to_le_bytes());
    v.extend_from_slice(&[0; 16]); // resolucao e paleta
    for y in (0..n).rev() {
        for x in 0..n {
            let p = &rgba[(y * n + x) * 4..][..4];
            v.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
        }
    }
    v.extend(std::iter::repeat_n(0u8, mask_row * n));
    v
}

// ---- VS_VERSIONINFO (o que o Explorer e o Gerenciador de Tarefas mostram) ----

fn utf16z(s: &str) -> Vec<u8> {
    s.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect()
}

fn pad4(v: &mut Vec<u8>) {
    while v.len() % 4 != 0 {
        v.push(0);
    }
}

/// Bloco generico: wLength, wValueLength, wType, szKey, padding, valor, padding, filhos.
fn block(key: &str, value: &[u8], value_len: u16, text: bool, children: &[Vec<u8>]) -> Vec<u8> {
    let mut v = vec![0, 0];
    v.extend_from_slice(&value_len.to_le_bytes());
    v.extend_from_slice(&(text as u16).to_le_bytes());
    v.extend_from_slice(&utf16z(key));
    pad4(&mut v);
    v.extend_from_slice(value);
    for c in children {
        pad4(&mut v);
        v.extend_from_slice(c);
    }
    let len = v.len() as u16;
    v[0..2].copy_from_slice(&len.to_le_bytes());
    v
}

fn version_info() -> Vec<u8> {
    let ver = std::env::var("CARGO_PKG_VERSION").unwrap();
    let mut parts = ver.split('.').map(|p| p.parse::<u16>().unwrap_or(0)).chain(std::iter::repeat(0));
    let (a, b, c) = (parts.next().unwrap(), parts.next().unwrap(), parts.next().unwrap());
    let ms = (a as u32) << 16 | b as u32;
    let ls = (c as u32) << 16;
    let mut fixed = Vec::new();
    for x in [0xFEEF04BDu32, 0x0001_0000, ms, ls, ms, ls, 0x3F, 0, 0x0004_0004, 1, 0, 0, 0] {
        fixed.extend_from_slice(&x.to_le_bytes());
    }
    let string = |k: &str, val: &str| {
        let data = utf16z(val);
        block(k, &data, (data.len() / 2) as u16, true, &[])
    };
    let strings = [
        string("CompanyName", "renanzm"),
        string("FileDescription", "Sound Mod"),
        string("FileVersion", &ver),
        string("InternalName", "SoundMod"),
        string("LegalCopyright", "MIT License"),
        string("OriginalFilename", "SoundMod.exe"),
        string("ProductName", "Sound Mod"),
        string("ProductVersion", &ver),
    ];
    let table = block("040904B0", &[], 0, true, &strings);
    let sfi = block("StringFileInfo", &[], 0, true, &[table]);
    let translation = block("Translation", &[0x09, 0x04, 0xB0, 0x04], 4, false, &[]);
    let vfi = block("VarFileInfo", &[], 0, true, &[translation]);
    block("VS_VERSION_INFO", &fixed, fixed.len() as u16, false, &[sfi, vfi])
}
