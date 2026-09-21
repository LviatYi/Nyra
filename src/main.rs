mod job;
mod reaction;
mod sensory;
mod settings_default_values;

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use crate::job::{JobConfig, Tips, is_valid_tip_color};
use crate::reaction::{GlobalHotkeys, ReactionConfig, ReactionPlugin, ReactionRunner, RunState};
use crate::sensory::SensoryPlugin;
use crate::settings_default_values::{WINDOW_HEIGHT, WINDOW_WIDTH};
use bevy::{
    prelude::*,
    render::{
        RenderPlugin,
        settings::{Backends, WgpuSettings},
    },
    window::{CompositeAlphaMode, WindowLevel, WindowResolution},
};

#[derive(serde::Deserialize)]
struct AppConfig {
    #[serde(flatten)]
    tips: Tips,
    #[serde(default)]
    reaction: ReactionConfig,
}

fn main() -> ExitCode {
    if env::args_os()
        .skip(1)
        .any(|arg| arg == "--help" || arg == "-h")
    {
        println!(
            "Usage: nyra.exe [config.json] [--run-script script.ts]\nScript paths are resolved relative to the configuration file directory; --run-script only executes the script without launching the floating window."
        );
        return ExitCode::SUCCESS;
    }
    match launch() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Nyra failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn launch() -> Result<(), String> {
    configure_dpi_awareness()?;

    let mut args = env::args_os().skip(1);
    let mut config_path = None;
    let mut script = None;
    while let Some(arg) = args.next() {
        if arg == "--run-script" && script.is_none() {
            script = Some(PathBuf::from(
                args.next().ok_or("--run-script missing script path")?,
            ));
        } else if arg.to_string_lossy().starts_with('-') || config_path.is_some() {
            return Err(format!(
                "Unknown argument: {}；use --help to see usage",
                arg.to_string_lossy()
            ));
        } else {
            config_path = Some(PathBuf::from(arg));
        }
    }
    let config_path = config_path.unwrap_or_else(|| PathBuf::from("config.json"));
    let config = load_config(&config_path)?;
    if let Some(script) = script {
        let mut runner = ReactionRunner::new(
            &config.reaction,
            &config_path,
            std::iter::once(script.clone()),
        )?;
        let run_id = runner.start(&script)?;
        println!("[Reaction {run_id}] RUNNING SCRIPT {}", script.display());
        while runner.is_busy() {
            runner.poll();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let status = runner.status();
        match status.state {
            RunState::Completed => println!("[Reaction {}] COMPLETED", status.run_id.unwrap()),
            RunState::ScriptFailed(_) => {}
            RunState::Failed(error) => return Err(format!("[Reaction {run_id}] {error}")),
            _ => return Err(format!("[Reaction {run_id}] Runner did not exit normally")),
        }
    } else {
        let scripts = config
            .tips
            .tips
            .iter()
            .filter_map(|tip| {
                tip.reaction
                    .as_ref()
                    .map(|reaction| reaction.script.clone())
            })
            .collect::<Vec<_>>();
        let runner = ReactionRunner::new(&config.reaction, &config_path, scripts)?;
        let hotkeys = GlobalHotkeys::new()?;
        configure_render_environment();
        run(config.tips, runner, hotkeys);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn configure_dpi_awareness() -> Result<(), String> {
    use windows_sys::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, DPI_AWARENESS_PER_MONITOR_AWARE,
        GetAwarenessFromDpiAwarenessContext, GetThreadDpiAwarenessContext,
        SetProcessDpiAwarenessContext,
    };

    unsafe {
        if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) != 0 {
            return Ok(());
        }

        let error = std::io::Error::last_os_error();
        let awareness = GetAwarenessFromDpiAwarenessContext(GetThreadDpiAwarenessContext());
        if awareness == DPI_AWARENESS_PER_MONITOR_AWARE {
            Ok(())
        } else {
            Err(format!(
                "Unable to enable physical screen coordinates with Per-Monitor V2 DPI awareness: {error}"
            ))
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn configure_dpi_awareness() -> Result<(), String> {
    Ok(())
}

fn configure_render_environment() {
    #[cfg(target_os = "windows")]
    {
        // Bevy 0.19 does not expose wgpu's DX12 presentation-system setting.
        // The default HWND swap chain only supports opaque composition, so a
        // transparent window using premultiplied alpha needs DirectComposition's
        // Visual presentation path instead. wgpu reads this variable while its
        // instance is created, which is why this function must run before Bevy.
        //
        // Rust 2024 marks process-environment mutation as unsafe for platforms
        // where concurrent access can cause undefined behavior. This block is
        // compiled only on Windows, where `set_var` is documented as always safe.
        unsafe {
            env::set_var("WGPU_DX12_PRESENTATION_SYSTEM", "visual");
        }
    }
}

fn load_config(path: &Path) -> Result<AppConfig, String> {
    let source = fs::read_to_string(path).map_err(|error| {
        format!(
            "Failed to read configuration file {}: {error}",
            path.display()
        )
    })?;
    let config: AppConfig = serde_json::from_str(&source).map_err(|error| {
        format!(
            "Failed to parse configuration file {}: {error}",
            path.display()
        )
    })?;
    validate_config(&config.tips)?;
    Ok(config)
}

fn validate_config(config: &Tips) -> Result<(), String> {
    if config.tips.is_empty() {
        return Err("Tips in the configuration cannot be empty".into());
    }

    for (index, tip) in config.tips.iter().enumerate() {
        let name = format!("tips[{index}]");
        if tip.tip.trim().is_empty() {
            return Err(format!("{name}.tip cannot be empty"));
        }
        if tip.interval <= 0 {
            return Err(format!("{name}.interval must be greater than 0"));
        }
        if let Some(show_time) = tip.show_time
            && show_time <= 0
        {
            return Err(format!("{name}.showTime must be greater than 0"));
        }
        if let Some(color) = tip.color.as_deref()
            && !is_valid_tip_color(color)
        {
            return Err(format!("{name}.color must use format like #RRGGBB"));
        }
    }

    Ok(())
}

fn run(config: Tips, runner: ReactionRunner, hotkeys: GlobalHotkeys) {
    App::new()
        .insert_resource(ClearColor(Color::NONE))
        .insert_resource(JobConfig(config))
        .insert_resource(runner)
        .insert_resource(hotkeys)
        .add_plugins(
            DefaultPlugins
                .set(RenderPlugin {
                    render_creation: WgpuSettings {
                        backends: Some(Backends::DX12),
                        ..default()
                    }
                    .into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Nyra".into(),
                        resolution: WindowResolution::new(WINDOW_WIDTH, WINDOW_HEIGHT),
                        decorations: false,
                        transparent: true,
                        composite_alpha_mode: CompositeAlphaMode::PreMultiplied,
                        resizable: false,
                        window_level: WindowLevel::AlwaysOnTop,
                        skip_taskbar: true,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(SensoryPlugin)
        .add_plugins(ReactionPlugin)
        .run();
}
