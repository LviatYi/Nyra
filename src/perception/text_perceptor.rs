use std::error::Error;

pub trait TextPerceptor {
    fn recognize(&self, grey_image: &image::DynamicImage) -> Result<String, Box<dyn Error>>;
}
