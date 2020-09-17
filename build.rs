/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::env;
use std::path::PathBuf;

fn run_bindgen(include_paths: &[PathBuf], defines: &[&str]) {
    println!("cargo:rerun-if-changed=wrapper.h");

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let mut config = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header("wrapper.h")
        .default_enum_style(bindgen::EnumVariation::NewType { is_bitfield: false })
        .whitelist_type("XML_.*")
        .size_t_is_usize(true)
        .whitelist_var("XML_.*")
        .whitelist_function("XML_.*")
        .blacklist_item("XML_ENABLE_VISIBILITY")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks));

    if cfg!(not(feature = "attr_info")) {
        config = config.blacklist_type("XML_AttrInfo")
            .blacklist_function("XML_GetAttributeInfo");
    }

    if cfg!(feature = "no_std") {
        config = config.use_core().ctypes_prefix("::libc");
    }

    for path in include_paths {
        config = config.clang_arg(format!("-I{}", path.display()));
    }

    for def in defines {
        config = config.clang_arg(format!("-D{}", def));
    }

    // Finish the builder and generate the bindings.
    let bindings = config
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}

fn main() {
    if cfg!(feature = "ushort") && cfg!(feature = "wchar_t") {
        panic!("Can't use EXPAT_CHAR_TYPE=ushort and EXPAT_CHAR_TYPE=wchar_t at the same time");
    }
    if cfg!(feature = "static") && cfg!(feature = "dynamic") {
        panic!("Can't use static and dynamic at the same time");
    }
    if cfg!(feature = "wchar_t") {
        env::var("CARGO_CFG_WINDOWS").expect("EXPAT_CHAR_TYPE=wchar_t only works on Windows");
    }
    let target = env::var("TARGET").unwrap();

    if cfg!(not(feature = "bundled")) && !target.contains("android") {
        if let Ok(config) = pkg_config::Config::new().probe("expat") {
            let defines: Vec<_> = config.defines.keys().map(|k| k.as_str()).collect();

            run_bindgen(&config.include_paths, &defines);

            return;
        }
    }

    let mut config = cmake::Config::new("libexpat/expat");

    let mut cfg = if target.contains("android") {
        let ndk_root = env::var("ANDROID_NDK_ROOT")
            .or(env::var("ANDROID_NDK_HOME"))
            .expect("`$ANDROID_NDK_ROOT` or `$ANDROID_NDK_ROOT` is not set.");
        let config = config.define(
            "CMAKE_TOOLCHAIN_FILE",
            format!("{}/build/cmake/android.toolchain.cmake", ndk_root),
        );
        if target.starts_with("aarch64") {
            config.define("ANDROID_ABI", "arm64-v8a")
        } else if target.starts_with("armv7") {
            config.define("ANDROID_ABI", "armeabi-v7a")
        } else if target.starts_with("i686") {
            config.define("ANDROID_ABI", "x86")
        } else if target.starts_with("x86_64") {
            config.define("ANDROID_ABI", "x86_64")
        } else {
            config
        }
    } else {
        &mut config
    };

    let mut defines = vec![];

    cfg = cfg.define(
        "EXPAT_SHARED_LIBS",
        if cfg!(feature = "dynamic") {
            "1"
        } else {
            defines.push("XML_STATIC");
            "0"
        },
    );

    cfg = cfg.define(
        "EXPAT_LARGE_SIZE",
        if cfg!(feature = "large_size") {
            defines.push("XML_LARGE_SIZE");
            "1"
        } else {
            "0"
        },
    );

    cfg = cfg.define(
        "EXPAT_MIN_SIZE",
        if cfg!(feature = "min_size") {
            defines.push("XML_MIN_SIZE");
            "1"
        } else {
            "0"
        },
    );

    cfg = cfg.define(
        "EXPAT_ATTR_INFO",
        if cfg!(feature = "attr_info") {
            defines.push("XML_ATTR_INFO");
            "1"
        } else {
            "0"
        },
    );

    cfg = cfg.define(
        "EXPAT_DTD",
        if cfg!(feature = "dtd") {
            defines.push("XML_DTD");
            "1"
        } else {
            "0"
        },
    );

    cfg = cfg.define(
        "EXPAT_NS",
        if cfg!(feature = "ns") {
            defines.push("XML_NS");
            "1"
        } else {
            "0"
        },
    );

    if env::var("CARGO_CFG_UNIX").is_ok() {
        cfg = cfg.define(
            "EXPAT_DEV_URANDOM",
            if cfg!(feature = "urandom") {
                defines.push("XML_DEV_URANDOM");
                "1"
            } else {
                "0"
            },
        );
    }

    cfg = cfg.define(
        "EXPAT_CHAR_TYPE",
        if cfg!(feature = "ushort") {
            defines.push("XML_UNICODE");
            "ushort"
        } else if cfg!(feature = "wchar_t") {
            defines.push("XML_UNICODE");
            defines.push("XML_UNICODE_WCHAR_T");
            "wchar_t"
        } else {
            "char"
        },
    );

    let msvc = target.contains("msvc");

    cfg = cfg.define(
        "EXPAT_MSVC_STATIC_CRT",
        if msvc
            && env::var("CARGO_CFG_TARGET_FEATURE")
                .unwrap()
                .contains("crt-static")
        {
            "1"
        } else {
            "0"
        },
    );

    let dst = cfg
        .define("EXPAT_BUILD_TOOLS", "0")
        .define("EXPAT_BUILD_EXAMPLES", "0")
        .define("EXPAT_BUILD_TESTS", "0")
        .define("EXPAT_BUILD_DOCS", "0")
        .define("CMAKE_DEBUG_POSTFIX", "")
        .define("CMAKE_RELEASE_POSTFIX", "")
        .define("CMAKE_MINSIZEREL_POSTFIX", "")
        .define("CMAKE_RELWITHDEBINFO_POSTFIX", "")
        .build();

    let mut include = dst.clone();
    include.push("include");

    println!("cargo:include={}", include.display());

    run_bindgen(&[include], &defines);

    let mut lib = dst.clone();
    lib.push("lib");

    let lib_name = if msvc { "libexpat" } else { "expat" };

    println!("cargo:rustc-link-search=native={}", lib.display());
    println!(
        "cargo:rustc-link-lib={}={}",
        if cfg!(feature = "dynamic") {
            "dylib"
        } else {
            "static"
        },
        lib_name
    );
}
