#![cfg(target_os = "windows")]

use image::imageops::{FilterType, crop_imm, resize};
use image::{DynamicImage, GrayImage, Luma};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};
use xcap::{Monitor, Window};

#[derive(Debug)]
struct WindowCapture {
    title: String,
    screenshot_path: PathBuf,
}

fn get_default_tessdata_dir() -> PathBuf {
    PathBuf::from(env::var("APPDATA").expect("APPDATA environment variable not set"))
        .join("tesseract-rs")
        .join("tessdata")
}

fn get_tessdata_dir() -> PathBuf {
    env::var("TESSDATA_PREFIX")
        .map(PathBuf::from)
        .unwrap_or_else(|_| get_default_tessdata_dir())
}

fn get_tesseract_executable() -> PathBuf {
    let appdata = env::var("APPDATA").expect("APPDATA environment variable not set");
    PathBuf::from(appdata)
        .join("tesseract-rs")
        .join("tesseract")
        .join("bin")
        .join("tesseract.exe")
}

fn capture_rustrover_window(output_path: &Path) -> Result<WindowCapture, Box<dyn Error>> {
    let target = Window::all()?
        .into_iter()
        .filter_map(|window| {
            let title = window.title().ok()?;
            let minimized = window.is_minimized().ok()?;
            Some((window, title, minimized))
        })
        .find(|(_, title, minimized)| !minimized && title.to_lowercase().contains("nyra"))
        .ok_or("No visible window title containing 'nyra' was found.")?;

    let (window, title, _) = target;
    let window_x = window.x()?;
    let window_y = window.y()?;
    let window_width = window.width()?;
    let window_height = window.height()?;
    let monitor = Monitor::from_point(window_x, window_y)?;
    let region_x = (window_x - monitor.x()?).max(0) as u32;
    let region_y = (window_y - monitor.y()?).max(0) as u32;
    let available_width = monitor.width()?.saturating_sub(region_x);
    let available_height = monitor.height()?.saturating_sub(region_y);
    let region_width = window_width.min(available_width).max(1);
    let region_height = window_height.min(available_height).max(1);
    let image = monitor.capture_region(region_x, region_y, region_width, region_height)?;
    image.save(output_path)?;

    Ok(WindowCapture {
        title,
        screenshot_path: output_path.to_path_buf(),
    })
}

fn crop_title_region(image: &DynamicImage) -> DynamicImage {
    let width = image.width();
    let height = image.height();
    let crop_width = width.clamp(1, 420);
    let crop_height = height.clamp(1, 220);
    DynamicImage::ImageRgba8(crop_imm(image, 0, 0, crop_width, crop_height).to_image())
}

fn threshold_image(image: &GrayImage, threshold: u8) -> GrayImage {
    let mut out = image.clone();
    for pixel in out.pixels_mut() {
        let value = if pixel[0] >= threshold { 0 } else { 255 };
        *pixel = Luma([value]);
    }
    out
}

fn save_preprocessed_title_region(image_path: &Path, output_path: &Path) -> Result<(), Box<dyn Error>> {
    let image = image::open(image_path)?;
    let title_region = crop_title_region(&image);
    let grayscale = title_region.to_luma8();
    let thresholded = threshold_image(&grayscale, 96);
    let enlarged = resize(
        &thresholded,
        thresholded.width() * 8,
        thresholded.height() * 8,
        FilterType::Nearest,
    );
    enlarged.save(output_path)?;
    Ok(())
}

fn recognize_text(image_path: &Path) -> Result<String, Box<dyn Error>> {
    let tessdata_dir = get_tessdata_dir();
    let tesseract = get_tesseract_executable();
    let output = Command::new(tesseract)
        .arg(image_path)
        .arg("stdout")
        .arg("--tessdata-dir")
        .arg(&tessdata_dir)
        .args(["--psm", "6"])
        .args(["-l", "eng"])
        .args([
            "-c",
            "tessedit_char_whitelist=abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_ .",
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tesseract.exe failed: {}", stderr.trim()).into());
    }

    Ok(String::from_utf8(output.stdout)?)
}

#[test]
#[ignore = "Requires a visible RustRover window whose title contains 'nyra'"]
fn ocr_detects_nyra_in_rustrover_titlebar() -> Result<(), Box<dyn Error>> {
    let output_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("ocr-mvp");
    fs::create_dir_all(&output_dir)?;

    let screenshot_path = output_dir.join("rustrover-nyra-window.png");
    let preprocessed_path = output_dir.join("rustrover-nyra-titlebar-preprocessed.bmp");
    let capture = capture_rustrover_window(&screenshot_path)?;
    save_preprocessed_title_region(&capture.screenshot_path, &preprocessed_path)?;
    let text = recognize_text(&preprocessed_path)?;
    let normalized = text.to_lowercase();

    assert!(
        normalized.contains("nyra"),
        "OCR did not detect 'nyra'. window_title={:?}, ocr_text={:?}, screenshot={}, preprocessed={}",
        capture.title,
        text,
        capture.screenshot_path.display(),
        preprocessed_path.display()
    );

    Ok(())
}
