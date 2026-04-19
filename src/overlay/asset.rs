use eframe::epaint::FontFamily;
use eframe::epaint::text::{FontData, FontDefinitions};
use egui::Context;
use std::fs;
use std::path::PathBuf;

pub fn install_fonts(ctx: &Context) {
    let mut definitions = FontDefinitions::default();

    for candidate in candidate_font_paths() {
        if let Ok(bytes) = fs::read(&candidate) {
            definitions
                .font_data
                .insert("nyra-cjk".to_string(), FontData::from_owned(bytes).into());

            if let Some(family) = definitions.families.get_mut(&FontFamily::Proportional) {
                family.insert(0, "nyra-cjk".to_string());
            }
            if let Some(family) = definitions.families.get_mut(&FontFamily::Monospace) {
                family.push("nyra-cjk".to_string());
            }

            ctx.set_fonts(definitions);
            return;
        }
    }
}

fn candidate_font_paths() -> Vec<PathBuf> {
    //TODO_LviatYi: use other font assets.
    vec![PathBuf::from(r"C:\Windows\Fonts\msyh.ttc")]
}
