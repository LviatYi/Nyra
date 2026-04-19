mod app;
mod capture;
mod measure;
mod overlay_shell;
mod perception;
mod platform;
mod ui_frontend;
pub mod overlay;
pub mod job;

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("Nyra overlay currently targets Windows first.");
    std::process::exit(1);
}

#[cfg(target_os = "windows")]
fn main() {
    if let Err(error) = app::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
