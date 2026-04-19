use crate::overlay_shell::launch_overlay;
use crate::perception::perception_service::service::PerceptionService;
use std::error::Error;
use tracing::level_filters::LevelFilter;

const APP_TITLE: &str = "Nyra";

pub fn bootstrap() {
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::INFO)
        .with_target(false)
        .try_init()
        .ok();
}

pub fn run() -> Result<(), Box<dyn Error>> {
    bootstrap();
    let perception_service = PerceptionService::default();
    launch_overlay(APP_TITLE, perception_service)
}
