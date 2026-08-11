//! Bundle `src/frontend/app.ts` → `static/app.js` with esbuild (no Node/pnpm).
//!
//! Downloads a pinned `@esbuild/<platform>` binary from the npm registry into
//! `OUT_DIR` on first use. Override with `ESBUILD_BIN` to point at a local binary.

use std::env;
use std::fs::{self, File};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const ESBUILD_VERSION: &str = "0.25.12";

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let frontend_dir = manifest_dir.join("src/frontend");
    let entry = frontend_dir.join("app.ts");
    let outfile = manifest_dir.join("static/app.js");

    rerun_if_changed_dir(&frontend_dir);
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=ESBUILD_BIN");
    println!("cargo:rerun-if-env-changed=ESBUILD_VERSION");

    if !entry.is_file() {
        panic!("frontend entry missing: {}", entry.display());
    }

    let esbuild = resolve_esbuild();
    if let Some(parent) = outfile.parent() {
        fs::create_dir_all(parent).expect("create static/");
    }

    let status = Command::new(&esbuild)
        .arg(entry.as_os_str())
        .arg("--bundle")
        .arg("--format=iife")
        .arg("--target=es2022")
        .arg(format!("--outfile={}", outfile.display()))
        .status()
        .unwrap_or_else(|err| panic!("failed to spawn {}: {err}", esbuild.display()));

    if !status.success() {
        panic!("esbuild failed with {status}");
    }
}

fn resolve_esbuild() -> PathBuf {
    if let Ok(path) = env::var("ESBUILD_BIN") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return path;
        }
        panic!("ESBUILD_BIN does not exist: {}", path.display());
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let cache_dir = out_dir.join(format!("esbuild-{ESBUILD_VERSION}"));
    let bin_name = if cfg!(windows) {
        "esbuild.exe"
    } else {
        "esbuild"
    };
    let bin_path = cache_dir.join(bin_name);
    if bin_path.is_file() {
        return bin_path;
    }

    download_esbuild(&cache_dir, &bin_path);
    bin_path
}

fn platform_package() -> (&'static str, &'static str) {
    // npm package suffix + path inside the tarball (under package/)
    match (env::consts::OS, env::consts::ARCH) {
        ("windows", "x86_64") => ("win32-x64", "esbuild.exe"),
        ("windows", "aarch64") => ("win32-arm64", "esbuild.exe"),
        ("linux", "x86_64") => ("linux-x64", "bin/esbuild"),
        ("linux", "aarch64") => ("linux-arm64", "bin/esbuild"),
        ("macos", "x86_64") => ("darwin-x64", "bin/esbuild"),
        ("macos", "aarch64") => ("darwin-arm64", "bin/esbuild"),
        (os, arch) => panic!("unsupported host for esbuild download: {os}/{arch}"),
    }
}

fn download_esbuild(cache_dir: &Path, bin_path: &Path) {
    let (pkg_suffix, subpath) = platform_package();
    let url = format!(
        "https://registry.npmjs.org/@esbuild/{pkg_suffix}/-/{pkg_suffix}-{ESBUILD_VERSION}.tgz"
    );
    eprintln!("downloading esbuild {ESBUILD_VERSION} ({pkg_suffix})…");

    let response = ureq::get(&url)
        .call()
        .unwrap_or_else(|err| panic!("download {url}: {err}"));
    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .unwrap_or_else(|err| panic!("read {url}: {err}"));

    let decoder = flate2::read::GzDecoder::new(Cursor::new(bytes));
    let mut archive = tar::Archive::new(decoder);
    let wanted = format!("package/{subpath}");

    fs::create_dir_all(cache_dir).expect("create esbuild cache dir");
    let mut found = false;
    for entry in archive.entries().expect("tar entries") {
        let mut entry = entry.expect("tar entry");
        let path = entry.path().expect("tar path").into_owned();
        if path.to_string_lossy() != wanted {
            continue;
        }
        let mut out = File::create(bin_path).expect("create esbuild binary");
        std::io::copy(&mut entry, &mut out).expect("extract esbuild binary");
        out.flush().ok();
        found = true;
        break;
    }
    if !found {
        panic!("esbuild binary {wanted} not found in {url}");
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(bin_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(bin_path, perms).expect("chmod esbuild");
    }
}

fn rerun_if_changed_dir(dir: &Path) {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        println!("cargo:rerun-if-changed={}", path.display());
        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
}
