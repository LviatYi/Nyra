use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=VCPKG_ROOT");

    #[cfg(target_os = "windows")]
    {
        for root in candidate_vcpkg_roots() {
            let lib_dir = root.join("installed").join("x64-windows").join("lib");
            if lib_dir.exists() {
                println!("cargo:rustc-link-search=native={}", lib_dir.display());
                println!("cargo:warning=Using vcpkg lib path: {}", lib_dir.display());
                break;
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn candidate_vcpkg_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Ok(root) = env::var("VCPKG_ROOT") {
        roots.push(PathBuf::from(root));
    }

    roots.push(PathBuf::from(r"C:\Workspace\vcpkg"));
    roots.push(PathBuf::from(r"C:\vcpkg"));
    roots
}
