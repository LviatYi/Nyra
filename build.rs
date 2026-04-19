use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=scripts/setup-dev.ps1");
    println!("cargo:rerun-if-changed=scripts/build-ocr-release.ps1");

    #[cfg(target_os = "windows")]
    {
        let required_pairs = [
            (
                r".ocr-release\leptonica\lib\leptonica-1.84.1.lib",
                r"%APPDATA%\tesseract-rs\leptonica\lib\leptonica.lib",
            ),
            (
                r".ocr-release\tesseract\lib\tesseract53.lib",
                r"%APPDATA%\tesseract-rs\tesseract\lib\tesseract.lib",
            ),
            (
                r".ocr-release\tessdata\eng.traineddata",
                r"%APPDATA%\tesseract-rs\tessdata\eng.traineddata",
            ),
        ];

        let appdata = std::env::var("APPDATA").ok();
        let missing: Vec<_> = required_pairs
            .iter()
            .filter(|(workspace_path, appdata_path)| {
                let workspace_exists = Path::new(workspace_path).exists();
                let appdata_exists = appdata
                    .as_ref()
                    .map(|root| {
                        let resolved = appdata_path.replacen("%APPDATA%", root, 1);
                        Path::new(&resolved).exists()
                    })
                    .unwrap_or(false);
                !workspace_exists && !appdata_exists
            })
            .map(|(workspace_path, _)| *workspace_path)
            .collect();

        if !missing.is_empty() {
            println!("cargo:warning=OCR release dependencies are missing:");
            for path in missing {
                println!("cargo:warning=  - {path}");
            }
            println!(
                "cargo:warning=Run .\\scripts\\setup-dev.ps1 before building if the OCR toolchain is not initialized on this machine."
            );
        }
    }
}
