extern crate gcc;
extern crate pkg_config;

use std::{env, fs, io::Write, path::Path, process::Command};

fn main() {
    dump_git_version();
    build_wayland_glib_interface();
}

/// Writes the current git hash to a file that is read by Way Cooler
///
/// If this is a release build, the file will be empty.
fn dump_git_version() {
    let out_dir = env::var("OUT_DIR").expect("Could not find out directory!");
    let dest_path = Path::new(&out_dir).join("git-version.txt");
    let mut f = fs::File::create(&dest_path).expect("Could not write git version to out directory");
    if let Some(git_version) = git_version() {
        f.write_all(git_version.as_ref())
            .expect("Could not write to git version file");
    }
}

/// Gets the current hash of HEAD. Returns None if for some reason
/// that could not be retrieved (e.g not in a git repository)
fn git_version() -> Option<String> {
    if !in_release_commit() {
        Command::new("git")
            .arg("rev-parse")
            .arg("HEAD")
            .output()
            .ok()
            .map(|output| output.stdout)
            .map(|hash| String::from_utf8_lossy(&hash).trim().into())
    } else {
        None
    }
}

/// Determines if the current HEAD is tagged with a release
fn in_release_commit() -> bool {
    let result = Command::new("git")
        .arg("describe")
        .arg("--exact-match")
        .arg("--tags")
        .arg("HEAD")
        .output()
        .unwrap();
    result.status.success()
}

/// Build the wayland-glib interface as a static library
fn build_wayland_glib_interface() {
    let lib = pkg_config::Config::new().probe("glib-2.0").unwrap();
    let mut builder = gcc::Build::new();

    for i in &lib.include_paths {
        builder.include(i);
    }

    builder
        .file("src/wayland_glib_interface.c")
        .compile("wayland_glib_interface");
}
