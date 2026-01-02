use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/ffi.rs");
    println!("cargo:rerun-if-changed=cpp/");
    println!("cargo:rerun-if-changed=include/");
    
    // Register custom cfg for -Zcheck-cfg compatibility
    println!("cargo:rustc-check-cfg=cfg(atom_cpp_linked)");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    
    println!("cargo:warning=atom-bridge build.rs running...");
    println!("cargo:warning=Target OS: {}", target_os);
    println!("cargo:warning=Manifest dir: {}", manifest_dir);

    // Check for pre-built C++ library
    let lib_search_paths = [
        // Relative to atom-bridge crate
        PathBuf::from("lib"),
        PathBuf::from("cpp/build/lib/Release"),
        PathBuf::from("cpp/build/Release"),
        PathBuf::from("cpp/build/lib"),
        // Relative to bevy-game crate
        PathBuf::from("../atom-bridge/lib"),
        PathBuf::from("../atom-bridge/cpp/build/lib/Release"),
        PathBuf::from("../atom-bridge/cpp/build/Release"),
        // Absolute path fallback
        PathBuf::from(format!("{}/lib", manifest_dir)),
        PathBuf::from(format!("{}/cpp/build/lib/Release", manifest_dir)),
    ];

    let lib_name = if target_os == "windows" { "atom_bridge.lib" } else { "libatom_bridge.a" };
    let mut found_lib = false;
    
    for search_path in &lib_search_paths {
        let lib_path = search_path.join(lib_name);
        
        if lib_path.exists() {
            println!("cargo:warning=Found pre-built C++ library at: {:?}", lib_path);
            println!("cargo:rustc-link-search=native={}", search_path.display());
            println!("cargo:rustc-link-lib=static=atom_bridge");
            println!("cargo:rustc-cfg=atom_cpp_linked");
            found_lib = true;
            break;
        }
    }

    // Link Vulkan if available
    if let Ok(vulkan_sdk) = env::var("VULKAN_SDK") {
        println!("cargo:warning=VULKAN_SDK: {}", vulkan_sdk);
        if target_os == "windows" {
            println!("cargo:rustc-link-search=native={}/Lib", vulkan_sdk);
            println!("cargo:rustc-link-lib=vulkan-1");
        }
    }

    // Windows system libraries
    if target_os == "windows" && found_lib {
        println!("cargo:rustc-link-lib=user32");
        println!("cargo:rustc-link-lib=gdi32");
        println!("cargo:rustc-link-lib=shell32");
    }

    if found_lib {
        println!("cargo:warning=C++ Atom library found - REAL VULKAN RENDERER ENABLED");
    } else {
        // C++ library not found - use Bevy's built-in wgpu renderer (stub mode)
        // This is fine for development and testing - the custom Vulkan renderer is optional
        println!("cargo:warning=================================================");
        println!("cargo:warning=C++ library not found - USING BEVY WGPU RENDERER");
        println!("cargo:warning=================================================");
        println!("cargo:warning=The game will run using Bevy's built-in renderer.");
        println!("cargo:warning=This is fine for gameplay testing!");
        println!("cargo:warning=");
        println!("cargo:warning=To enable the custom Vulkan renderer later:");
        println!("cargo:warning=  1. Install Vulkan SDK");
        println!("cargo:warning=  2. Run: cmake -B cpp/build -G Ninja && cmake --build cpp/build");
        println!("cargo:warning=  3. Rebuild the game");
    }

    println!("cargo:warning=atom-bridge build.rs completed");
}
