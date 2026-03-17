use crate::measure::measure;
use crate::perception::text_perceptor::TextPerceptor;
use image::codecs::bmp::BmpEncoder;
use image::{ColorType, DynamicImage, ImageEncoder};
use std::cell::RefCell;
use std::env;
use std::error::Error;
use std::os::raw::c_char;
use std::os::raw::c_void;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::Once;
use tesseract_rs::{TessPageSegMode, TesseractAPI};

#[link(name = "leptonica")]
unsafe extern "C" {
    fn pixReadMemBmp(data: *const u8, size: usize) -> *mut c_void;

    fn pixDestroy(pix: *mut *mut c_void);
    fn leptSetStderrHandler(handler: Option<extern "C" fn(*const c_char)>);
}

static LEPTONICA_STDERR_INIT: Once = Once::new();

pub struct TesseractPerceptor {
    api: TesseractAPI,
}

impl TesseractPerceptor {
    fn new() -> Self {
        Self {
            api: TesseractAPI::new(),
        }
    }

    pub fn new_with_init() -> Result<Self, Box<dyn Error>> {
        let s = Self::new();
        s.init()?;
        Ok(s)
    }

    pub fn init(&self) -> Result<(), Box<dyn Error>> {
        configure_leptonica_stderr();

        let tessdata_dir = tessdata_dir()?;

        self.api.init(&tessdata_dir, "eng")?;
        self.api
            .set_page_seg_mode(TessPageSegMode::PSM_SINGLE_BLOCK)?;
        self.api.set_variable(
            "tessedit_char_whitelist",
            "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_ .:/\\",
        )?;

        Ok(())
    }
}

impl TextPerceptor for TesseractPerceptor {
    fn recognize(&self, image: &DynamicImage) -> Result<String, Box<dyn Error>> {
        let bmp_bytes = measure("encode_bmp", || encode_bmp(image))?;
        let mut pix = measure("encode_pix", || unsafe {
            pixReadMemBmp(bmp_bytes.as_ptr(), bmp_bytes.len())
        });

        if pix.is_null() {
            return Err("Leptonica pixReadMemBmp failed to load the in-memory BMP image.".into());
        }

        measure("prepare_pix", || -> tesseract_rs::Result<()> {
            self.api.set_image_2(pix)?;
            self.api.set_source_resolution(144)
        })?;

        let text = measure("tesseract_api_get_text", || {
            self.api.get_utf8_text().map(|r| r.trim().to_string())
        })?;
        unsafe { pixDestroy(&mut pix) };
        Ok(text)
    }
}

extern "C" fn suppress_leptonica_stderr(_message: *const c_char) {}

fn configure_leptonica_stderr() {
    LEPTONICA_STDERR_INIT.call_once(|| unsafe {
        leptSetStderrHandler(Some(suppress_leptonica_stderr));
    });
}

#[derive(Default)]
pub struct TesseractPerceptorPool {
    perceptors: Mutex<Vec<TesseractPerceptor>>,
}

impl TesseractPerceptorPool {
    pub fn new() -> Self {
        Self::default()
    }

    fn acquire(&self) -> Result<PooledTesseractPerceptor<'_>, Box<dyn Error>> {
        let perceptor = self
            .perceptors
            .lock()
            .map_err(|_| "Tesseract perceptor pool mutex was poisoned.")?
            .pop();

        let perceptor = match perceptor {
            Some(perceptor) => perceptor,
            None => measure("init_tesseract_perceptor", TesseractPerceptor::new_with_init)?,
        };

        Ok(PooledTesseractPerceptor {
            pool: self,
            perceptor: RefCell::new(Some(perceptor)),
        })
    }
}

impl TextPerceptor for TesseractPerceptorPool {
    fn recognize(&self, image: &DynamicImage) -> Result<String, Box<dyn Error>> {
        let perceptor = self.acquire()?;
        perceptor.recognize(image)
    }
}

struct PooledTesseractPerceptor<'a> {
    pool: &'a TesseractPerceptorPool,
    perceptor: RefCell<Option<TesseractPerceptor>>,
}

impl TextPerceptor for PooledTesseractPerceptor<'_> {
    fn recognize(&self, image: &DynamicImage) -> Result<String, Box<dyn Error>> {
        self.perceptor
            .borrow()
            .as_ref()
            .ok_or_else(|| "Tesseract perceptor was already returned to the pool.".into())
            .and_then(|perceptor| perceptor.recognize(image))
    }
}

impl Drop for PooledTesseractPerceptor<'_> {
    fn drop(&mut self) {
        if let Ok(mut slot) = self.perceptor.try_borrow_mut() {
            if let Some(perceptor) = slot.take() {
                if let Ok(mut pool) = self.pool.perceptors.lock() {
                    pool.push(perceptor);
                }
            }
        }
    }
}

fn tessdata_dir() -> Result<PathBuf, Box<dyn Error>> {
    if let Ok(dir) = env::var("TESSDATA_PREFIX") {
        return Ok(PathBuf::from(dir));
    }

    let appdata = env::var("APPDATA").map_err(
        |_| "APPDATA environment variable is not set, and TESSDATA_PREFIX was not provided.",
    )?;
    Ok(PathBuf::from(appdata).join("tesseract-rs").join("tessdata"))
}

fn encode_bmp(image: &DynamicImage) -> Result<Vec<u8>, Box<dyn Error>> {
    let grey = image.to_luma8();
    let mut bytes = Vec::new();
    let encoder = BmpEncoder::new(&mut bytes);
    encoder.write_image(
        grey.as_raw(),
        grey.width(),
        grey.height(),
        ColorType::L8.into(),
    )?;
    Ok(bytes)
}
