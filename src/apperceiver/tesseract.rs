use std::env;
use std::error::Error;
use std::os::raw::c_void;
use std::path::PathBuf;
use image::codecs::bmp::BmpEncoder;
use image::{ColorType, ImageEncoder};
use tesseract_rs::{TessPageSegMode, TesseractAPI};
use crate::apperceiver::text_apperceiver::TextApperceiver;
use crate::measure::measure;

#[link(name = "leptonica")]
unsafe extern "C" {
    fn pixReadMemBmp(data: *const u8, size: usize) -> *mut c_void;

    fn pixDestroy(pix: *mut *mut c_void);
}

pub struct TesseractApperceiver {
    api: TesseractAPI,
}

impl TesseractApperceiver {
    pub fn new() -> Self {
        Self {
            api: TesseractAPI::new(),
        }
    }

    pub fn new_with_init() -> Self {
        let s = Self::new();
        s.init().expect("Initializing Tesseract API failed.");
        s
    }

    pub fn init(&self) -> Result<(), Box<dyn Error>> {
        let tessdata_dir = tessdata_dir()?;

        self.api.init(&tessdata_dir, "eng")?;
        self.api.set_page_seg_mode(TessPageSegMode::PSM_SINGLE_BLOCK)?;
        self.api.set_variable(
            "tessedit_char_whitelist",
            "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_ .:/\\",
        )?;

        Ok(())
    }
}

impl TextApperceiver for TesseractApperceiver {
    fn recognize(&self, image: &image::GrayImage) -> Result<String, Box<dyn Error>> {
        let bmp_bytes = measure("encode_bmp", || encode_bmp(image))?;
        let mut pix = measure("encode_pix", || unsafe { pixReadMemBmp(bmp_bytes.as_ptr(), bmp_bytes.len()) });

        if pix.is_null() {
            return Err("Leptonica pixReadMemBmp failed to load the in-memory BMP image.".into());
        }

        measure("prepare_pix", || -> tesseract_rs::Result<()> {
            self.api.set_image_2(pix)?;
            self.api.set_source_resolution(144)
        })?;

        let text = measure(
            "tesseract_api_get_text",
            || self.api.get_utf8_text().map(|r| { r.trim().to_string() }))?;
        unsafe { pixDestroy(&mut pix) };
        Ok(text)
    }
}

fn tessdata_dir() -> Result<PathBuf, Box<dyn Error>> {
    if let Ok(dir) = env::var("TESSDATA_PREFIX") {
        return Ok(PathBuf::from(dir));
    }

    let appdata = env::var("APPDATA")
        .map_err(|_| "APPDATA environment variable is not set, and TESSDATA_PREFIX was not provided.")?;
    Ok(PathBuf::from(appdata).join("tesseract-rs").join("tessdata"))
}

fn encode_bmp(image: &image::GrayImage) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::new();
    let encoder = BmpEncoder::new(&mut bytes);
    encoder.write_image(
        image.as_raw(),
        image.width(),
        image.height(),
        ColorType::L8.into(),
    )?;
    Ok(bytes)
}