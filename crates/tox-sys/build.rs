use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // docs.rs only runs rustdoc: no native link, no local c-toxcore required
    if env::var("DOCS_RS").is_ok() {
        return;
    }

    println!("cargo:rerun-if-env-changed=TOXCORE_PREFIX");
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_PATH");

    // 1) system install via pkg-config (package name: toxcore)
    if let Some((libdir, _flags)) = pkg_config_toxcore() {
        println!("cargo:rustc-link-search=native={}", libdir.display());
        println!("cargo:rustc-link-lib=toxcore");
        // keep rpath so the binary finds a non-standard lib dir if needed
        if !libdir.starts_with("/usr/lib") && !libdir.starts_with("/lib") {
            println!("cargo:rustc-link-arg=-Wl,-rpath,{}", libdir.display());
        }
        return;
    }

    // 2) explicit prefix (TOXCORE_PREFIX or repo ./local)
    let prefix = env::var("TOXCORE_PREFIX")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../local"));

    let prefix = prefix.canonicalize().unwrap_or(prefix);
    let libdir = prefix.join("lib");
    let incdir = prefix.join("include");

    if !libdir.join("libtoxcore.so").exists() && !libdir.join("libtoxcore.a").exists() {
        panic!(
            "c-toxcore not found.\n\
             \n\
             tried pkg-config module `toxcore` and prefix {}.\n\
             \n\
             install the system package first, then retry `cargo install tox-tui`:\n\
             \n\
             Debian/Ubuntu:  sudo apt install libtoxcore-dev pkg-config\n\
             Fedora:         sudo dnf install c-toxcore-devel pkgconf-pkg-config\n\
             openSUSE:       sudo zypper install c-toxcore-devel pkg-config\n\
             Alpine:         apk add toxcore-dev pkgconf\n\
             Arch:           sudo pacman -S c-toxcore pkgconf\n\
             \n\
             or build c-toxcore yourself:\n\
             cmake -S vendor/c-toxcore -B vendor/c-toxcore/build -DCMAKE_INSTALL_PREFIX=$PWD/local -DBUILD_TOXAV=OFF\n\
             cmake --build vendor/c-toxcore/build -j\n\
             cmake --install vendor/c-toxcore/build\n\
             \n\
             then: TOXCORE_PREFIX=$PWD/local cargo install tox-tui",
            prefix.display()
        );
    }

    println!("cargo:rustc-link-search=native={}", libdir.display());
    println!("cargo:rustc-link-lib=toxcore");
    println!(
        "cargo:rerun-if-changed={}",
        incdir.join("tox/tox.h").display()
    );
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", libdir.display());
}

// probe `pkg-config --libs toxcore` without a build-dependency
fn pkg_config_toxcore() -> Option<(PathBuf, String)> {
    let ok = Command::new("pkg-config")
        .args(["--exists", "toxcore"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        return None;
    }
    let out = Command::new("pkg-config")
        .args(["--libs", "toxcore"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let flags = String::from_utf8_lossy(&out.stdout).trim().to_string();
    // find -L dir (first one wins)
    let libdir = flags
        .split_whitespace()
        .find_map(|tok| tok.strip_prefix("-L").map(PathBuf::from))?;
    // sanity: a libtoxcore should be findable there (or on default linker path)
    let has_lib = libdir.join("libtoxcore.so").exists()
        || libdir.join("libtoxcore.a").exists()
        || libdir.join("libtoxcore.dylib").exists();
    if !has_lib {
        // still accept: multiarch paths sometimes hide the .so name
        // but require the dir exists at least
        if !libdir.exists() {
            return None;
        }
    }
    Some((libdir, flags))
}
