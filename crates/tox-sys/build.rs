use std::env;
use std::path::PathBuf;

fn main() {
    let prefix = env::var("TOXCORE_PREFIX")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../local"));

    let prefix = prefix.canonicalize().unwrap_or(prefix);
    let libdir = prefix.join("lib");
    let incdir = prefix.join("include");

    if !libdir.join("libtoxcore.so").exists() && !libdir.join("libtoxcore.a").exists() {
        panic!(
            "c-toxcore not found at {}. Build it first:\n  \
             cmake -S vendor/c-toxcore -B vendor/c-toxcore/build -DCMAKE_INSTALL_PREFIX=$PWD/local -DBUILD_TOXAV=OFF\n  \
             cmake --build vendor/c-toxcore/build -j\n  \
             cmake --install vendor/c-toxcore/build\n\
             or set TOXCORE_PREFIX to an existing install.",
            prefix.display()
        );
    }

    println!("cargo:rustc-link-search=native={}", libdir.display());
    println!("cargo:rustc-link-lib=toxcore");
    println!("cargo:rerun-if-env-changed=TOXCORE_PREFIX");
    println!(
        "cargo:rerun-if-changed={}",
        incdir.join("tox/tox.h").display()
    );

    // Runtime loader path for the final binary (emitted here so tox-tui picks it up
    // when it depends on this crate's link-search; also set explicitly in tox-tui).
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", libdir.display());
}
