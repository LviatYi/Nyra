use crate::overlay::overlay_app::NyraOverlayApp;
use crate::perception::perception_service::service::PerceptionService;
use eframe::egui::{Pos2, Vec2, ViewportBuilder};
use eframe::{AppCreator, NativeOptions};
use std::error::Error;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
    SM_YVIRTUALSCREEN,
};

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
        viewport: overlay_viewport(title),
        ..Default::default()
    };

    eframe::run_native(title, native_options, app_creator)?;
    Ok(())
}

fn overlay_viewport(title: &'static str) -> ViewportBuilder {
    let mut viewport = ViewportBuilder::default()
        .with_title(title)
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top()
        .with_resizable(false);

    #[cfg(target_os = "windows")]
    {
        let left = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) } as f32;
        let top = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) } as f32;
        let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) } as f32;
        let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) } as f32;

        viewport = viewport
            .with_position(Pos2::new(left, top))
            .with_inner_size(Vec2::new(width, height - 1.0));
    }

    viewport
}
