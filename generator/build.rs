//! Embeds assets/icons/Adwaita-Red into the binary, so `--folders` needs no
//! files at run time. Generates `$OUT_DIR/adwaita_red.rs`:
//! `static ADWAITA_RED: &[(&str, &[u8])]` = (path relative to the theme, contents),
//! sorted by path.

use std::fs;
use std::path::{Path, PathBuf};

const ICONS: &str = "assets/icons/Adwaita-Red";

fn collect(dir: &Path, rel: &str, out: &mut Vec<(String, PathBuf)>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.expect("readable directory entry"))
        .collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name().into_string().expect("UTF-8 file name");
        let rel = if rel.is_empty() { name } else { format!("{rel}/{name}") };
        let kind = entry.file_type().expect("file type");
        assert!(!kind.is_symlink(), "{ICONS}/{rel}: symlinks are not allowed in the icon assets");
        if kind.is_dir() {
            collect(&entry.path(), &rel, out);
        } else {
            out.push((rel, entry.path()));
        }
    }
}

fn main() {
    println!("cargo::rerun-if-changed={ICONS}");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(ICONS);
    let mut files = Vec::new();
    collect(&root, "", &mut files);
    assert!(!files.is_empty(), "{ICONS} is empty");

    let mut code = String::from("static ADWAITA_RED: &[(&str, &[u8])] = &[\n");
    for (rel, path) in &files {
        code += &format!("    ({rel:?}, include_bytes!({:?})),\n", path.display().to_string());
    }
    code += "];\n";
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR is set by cargo"));
    fs::write(out.join("adwaita_red.rs"), code).expect("write generated code");
}
