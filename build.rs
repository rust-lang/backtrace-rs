use std::env;
use std::path::Path;

// Must be public so the build script of `std` can call it.
pub fn main() {
    match env::var("CARGO_CFG_TARGET_OS").unwrap_or_default().as_str() {
        "android" => build_android(),
        _ => {}
    }
}

#[cfg(not(any(android_api_at_least_0, android_api_at_least_21)))]
fn android_version_from_c_headers() -> Option<u32> {
    extern crate cc;

    // Used to detect the value of the `__ANDROID_API__`
    // builtin #define
    const MARKER: &str = "BACKTRACE_RS_ANDROID_APIVERSION";
    const ANDROID_API_C: &str = "
BACKTRACE_RS_ANDROID_APIVERSION __ANDROID_API__
";

    // Create `android-api.c` on demand.
    // Required to support calling this from the `std` build script.
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let android_api_c = Path::new(&out_dir).join("android-api.c");
    std::fs::write(&android_api_c, ANDROID_API_C).unwrap();

    let expansion = match cc::Build::new().file(&android_api_c).try_expand() {
        Ok(result) => result,
        Err(e) => {
            eprintln!(
                "warning: android version detection failed while running C compiler: {}",
                e
            );
            return None;
        }
    };
    let expansion = match std::str::from_utf8(&expansion) {
        Ok(s) => s,
        Err(_) => return None,
    };
    eprintln!("expanded android version detection:\n{}", expansion);
    let i = match expansion.find(MARKER) {
        Some(i) => i,
        None => return None,
    };
    let version = match expansion[i + MARKER.len() + 1..].split_whitespace().next() {
        Some(s) => s,
        None => return None,
    };
    match version.parse::<u32>() {
        Ok(n) => Some(n),
        Err(_) => None,
    }
}

/// Sets cfgs that depend on the Android API level.
///
/// This depends on the use of a C preprocessor to find the API level in system
/// headers. For build systems that do not want to use a C processor inside the
/// execution of build scripts, the build system can specify the API level
/// through a `--cfg` flag. If any is set, the C preprocessor will not be used:
/// * android_api_at_least_0: No minimum API level is guaranteed.
/// * android_api_at_least_21: The API level will be at least 21.
fn build_android() {
    let at_least_21;
    #[cfg(android_api_at_least_0)]
    {
        at_least_21 = false;
    }
    #[cfg(android_api_at_least_21)]
    {
        at_least_21 = true;
    }
    #[cfg(not(any(android_api_at_least_0, android_api_at_least_21)))]
    {
        at_least_21 = {
            let version = android_version_from_c_headers().unwrap_or_default();
            version >= 21
        }
    };

    if at_least_21 {
        println!("cargo:rustc-cfg=feature=\"dl_iterate_phdr\"");
    }
}
