use crate::overlay::overlay_app::NyraOverlayApp;
use crate::perception::perception_service::service::PerceptionService;
use eframe::egui::ViewportBuilder;
use eframe::{AppCreator, NativeOptions};
use std::error::Error;

pub fn launch_overlay(
    title: &'static str,
    perception_service: PerceptionService,
) -> Result<(), Box<dyn Error>> {
    let app_creator: AppCreator<'_> = Box::new(move |cc| {
        Ok::<Box<dyn eframe::App>, Box<dyn Error + Send + Sync>>(Box::new(NyraOverlayApp::new(
            cc,
            title,
            perception_service,
        )))
    });

    let native_options = NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title(title)
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_fullscreen(true)
            .with_resizable(false),
        ..Default::default()
    };

    eframe::run_native(title, native_options, app_creator)?;
    Ok(())
}
