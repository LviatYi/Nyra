use std::collections::HashSet;

use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};

/// Kept on Bevy's main thread so winit pumps the manager's native window messages.
pub(crate) struct GlobalHotkeys {
    manager: GlobalHotKeyManager,
    registered: Vec<HotKey>,
    pressed: HashSet<u32>,
}

impl GlobalHotkeys {
    pub(crate) fn new() -> global_hotkey::Result<Self> {
        Ok(Self {
            manager: GlobalHotKeyManager::new()?,
            registered: Vec::new(),
            pressed: HashSet::new(),
        })
    }

    pub(crate) fn poll(&mut self) {
        self.pressed.clear();
        for event in GlobalHotKeyEvent::receiver().try_iter() {
            if event.state == HotKeyState::Pressed
                && self.registered.iter().any(|hotkey| hotkey.id() == event.id)
            {
                self.pressed.insert(event.id);
            }
        }
    }

    pub(crate) fn set_active(&mut self, hotkeys: &[HotKey]) -> global_hotkey::Result<()> {
        for index in (0..self.registered.len()).rev() {
            let hotkey = self.registered[index];
            if !hotkeys.contains(&hotkey) {
                self.manager.unregister(hotkey)?;
                self.registered.swap_remove(index);
                self.pressed.remove(&hotkey.id());
            }
        }
        for &hotkey in hotkeys {
            if !self.registered.contains(&hotkey) {
                match self.manager.register(hotkey) {
                    Ok(()) => self.registered.push(hotkey),
                    // Keep other applications' bindings and retry on the next update.
                    Err(global_hotkey::Error::AlreadyRegistered(_)) => continue,
                    Err(error) => return Err(error),
                }
            }
        }
        Ok(())
    }

    pub(crate) fn just_pressed(&self, hotkey: HotKey) -> bool {
        self.pressed.contains(&hotkey.id())
    }
}

impl Drop for GlobalHotkeys {
    fn drop(&mut self) {
        let _ = self.manager.unregister_all(&self.registered);
    }
}
