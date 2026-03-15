pub trait TextApperceiver {
    fn recognize(&self, grey_image: &image::GrayImage) -> Result<String, Box<dyn std::error::Error>>;
}