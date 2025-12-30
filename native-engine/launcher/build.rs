use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let profile = env::var("PROFILE").unwrap_or_else(|_| "release".to_string());
    
    let target_dir = PathBuf::from(&manifest_dir).join("target").join(&profile);
    let exe_path = target_dir.join("aaa-launcher.exe");
    let deps_dir = target_dir.join("deps");
    
    println!("cargo:warning=Build.rs pre-cleanup running...");
    println!("cargo:warning=Checking: {}", exe_path.display());
    
    let _ = Command::new("taskkill")
        .args(["/F", "/IM", "aaa-launcher.exe"])
        .output();
    let _ = Command::new("taskkill")
        .args(["/F", "/IM", "aaa-launcher-*.exe"])
        .output();
    
    thread::sleep(Duration::from_millis(500));
    
    if exe_path.exists() {
        println!("cargo:warning=Found existing exe at {}", exe_path.display());
        
        for attempt in 1..=5 {
            println!("cargo:warning=Cleanup attempt {}/5", attempt);
            
            #[cfg(windows)]
            {
                use std::os::windows::ffi::OsStrExt;
                use std::ffi::OsStr;
                
                let wide_path: Vec<u16> = OsStr::new(&exe_path)
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();
                
                unsafe {
                    #[link(name = "kernel32")]
                    extern "system" {
                        fn SetFileAttributesW(lpFileName: *const u16, dwFileAttributes: u32) -> i32;
                    }
                    SetFileAttributesW(wide_path.as_ptr(), 0x80);
                }
            }
            
            match fs::remove_file(&exe_path) {
                Ok(()) => {
                    println!("cargo:warning=Successfully deleted old exe on attempt {}", attempt);
                    break;
                }
                Err(e) => {
                    println!("cargo:warning=Delete failed ({}), trying rename...", e);
                    
                    let backup_name = format!(
                        "aaa-launcher.old.{}.{}.exe",
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_millis())
                            .unwrap_or(0),
                        attempt
                    );
                    let backup_path = target_dir.join(&backup_name);
                    
                    match fs::rename(&exe_path, &backup_path) {
                        Ok(()) => {
                            println!("cargo:warning=Successfully renamed to {}", backup_name);
                            break;
                        }
                        Err(e2) => {
                            println!("cargo:warning=Rename also failed: {}", e2);
                            
                            if attempt < 5 {
                                println!("cargo:warning=Waiting 1 second before retry...");
                                thread::sleep(Duration::from_secs(1));
                            }
                        }
                    }
                }
            }
        }
        
        if exe_path.exists() {
            println!("cargo:warning=WARNING: Could not remove exe after 5 attempts!");
            println!("cargo:warning=Trying Windows scheduled deletion...");
            
            #[cfg(windows)]
            {
                use std::os::windows::ffi::OsStrExt;
                use std::ffi::OsStr;
                
                let wide_path: Vec<u16> = OsStr::new(&exe_path)
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();
                
                unsafe {
                    #[link(name = "kernel32")]
                    extern "system" {
                        fn MoveFileExW(lpExistingFileName: *const u16, lpNewFileName: *const u16, dwFlags: u32) -> i32;
                    }
                    
                    let result = MoveFileExW(wide_path.as_ptr(), std::ptr::null(), 0x4);
                    if result != 0 {
                        println!("cargo:warning=Exe scheduled for deletion on reboot");
                    }
                }
            }
        }
    } else {
        println!("cargo:warning=No existing exe found - clean state");
    }
    
    if deps_dir.exists() {
        if let Ok(entries) = fs::read_dir(&deps_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("aaa_launcher") && name.ends_with(".exe") {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }
    
    if let Ok(entries) = fs::read_dir(&target_dir) {
        let mut old_files: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with("aaa-launcher.old.")
            })
            .collect();
        
        old_files.sort_by_key(|e| std::cmp::Reverse(e.metadata().and_then(|m| m.modified()).ok()));
        
        for old_file in old_files.into_iter().skip(2) {
            let _ = fs::remove_file(old_file.path());
        }
    }
    
    println!("cargo:warning=Build.rs pre-cleanup complete");
}
