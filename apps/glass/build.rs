// Links Mutter's Clutter and Cogl, whose names carry GNOME's major version (mutter-clutter-51), found through pkg-config;
// GNOME Shell has them loaded already, so the library only names them, and the path is recorded for the tests.
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let list = Command::new("pkg-config").arg("--list-package-names").output();
    let names = match list {
        Ok(out) => String::from_utf8_lossy(&out.stdout).into_owned(),
        Err(_) => String::new(),
    };
    let mut clutter: Vec<&str> = names.lines().filter(|n| n.starts_with("mutter-clutter-")).collect();
    clutter.sort();
    let Some(clutter) = clutter.last().copied() else {
        println!("cargo:warning=mutter-devel is not installed: amethystora-glass is built without Mutter's libraries");
        return;
    };
    let version = clutter.trim_start_matches("mutter-clutter-");
    let cogl = format!("mutter-cogl-{version}");
    if let Ok(out) = Command::new("pkg-config").args(["--libs", clutter, &cogl]).output() {
        for arg in String::from_utf8_lossy(&out.stdout).split_whitespace() {
            if let Some(dir) = arg.strip_prefix("-L") {
                println!("cargo:rustc-link-search=native={dir}");
                println!("cargo:rustc-link-arg=-Wl,-rpath,{dir}");
            } else if let Some(lib) = arg.strip_prefix("-l") {
                println!("cargo:rustc-link-lib=dylib={lib}");
            }
        }
    }
}
