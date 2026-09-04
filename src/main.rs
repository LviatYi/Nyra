mod job;
mod sensory;
mod settings_default_values;

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
    time::Duration,
};

use crate::job::{JobConfig, Tips};
use crate::sensory::SensoryPlugin;
use crate::settings_default_values::{WINDOW_HEIGHT, WINDOW_WIDTH};
use bevy::{
    prelude::*,
    render::{
        RenderPlugin,
        settings::{Backends, WgpuSettings},
    },
    window::{CompositeAlphaMode, WindowLevel, WindowResolution},
    winit::{UpdateMode, WinitSettings},
};

const MAX_TEXT_COLUMNS: usize = 15;

fn main() -> ExitCode {
    configure_render_environment();

    let config_path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("config.json"));

    match load_config(&config_path) {
        Ok(config) => {
            run(config);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Nyra 启动失败：{error}");
            ExitCode::FAILURE
        }
    }
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

fn load_config(path: &Path) -> Result<Tips, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("无法读取配置文件 {}：{error}", path.display()))?;
    let config: Tips = serde_json::from_str(&source)
        .map_err(|error| format!("无法解析配置文件 {}：{error}", path.display()))?;
    validate_config(&config)?;
    Ok(config)
}

fn validate_config(config: &Tips) -> Result<(), String> {
    if config.tips.is_empty() {
        return Err("配置中的 tips 不能为空".into());
    }

    for (index, tip) in config.tips.iter().enumerate() {
        let name = format!("tips[{index}]");
        if tip.tip.trim().is_empty() {
            return Err(format!("{name}.tip 不能为空"));
        }
        if tip.interval <= 0 {
            return Err(format!("{name}.interval 必须是大于 0 的有限秒数"));
        }
        if let Some(show_time) = tip.show_time
            && (show_time <= 0 || show_time > tip.interval)
        {
            return Err(format!(
                "{name}.showTime 必须是大于 0 且不超过 interval 的有限秒数"
            ));
        }
    }

    Ok(())
}

fn run(config: Tips) {
    App::new()
        .insert_resource(ClearColor(Color::NONE))
        .insert_resource(JobConfig(config))
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
        .run();
}
