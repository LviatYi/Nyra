use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::Duration,
};

const CHAOS_LIMIT: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Offset {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Default, Serialize, Deserialize)]
struct OffsetImpression {
    offsets: Vec<Offset>,
    chaotic: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ScaleImpression {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Default, Serialize, Deserialize)]
struct Impressions {
    version: u32,
    offsets: HashMap<String, OffsetImpression>,
    #[serde(default)]
    scales: HashMap<String, ScaleImpression>,
}

struct State {
    data: Impressions,
    generation: u64,
    saved: u64,
}
struct Owner {
    state: Arc<Mutex<State>>,
    stop: mpsc::Sender<()>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl Drop for Owner {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.lock().unwrap().take() {
            let _ = worker.join();
        }
    }
}

#[derive(Clone, Resource)]
pub(crate) struct ImpressionStore(Arc<Owner>);

impl ImpressionStore {
    pub(crate) fn open(path: PathBuf) -> Self {
        let data = match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Impressions>(&bytes) {
                Ok(data) if data.version == 1 => data,
                _ => {
                    eprintln!(
                        "[Perception] Invalid or unsupported impressions: {}",
                        path.display()
                    );
                    Impressions {
                        version: 1,
                        ..Default::default()
                    }
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Impressions {
                version: 1,
                ..Default::default()
            },
            Err(error) => {
                eprintln!("[Perception] Unable to read impressions: {error}");
                Impressions {
                    version: 1,
                    ..Default::default()
                }
            }
        };
        let state = Arc::new(Mutex::new(State {
            data,
            generation: 0,
            saved: 0,
        }));
        let (stop, receiver) = mpsc::channel();
        let shared = state.clone();
        let worker = thread::spawn(move || {
            loop {
                let stopping = receiver.recv_timeout(Duration::from_secs(5)).is_ok();
                let snapshot = {
                    let state = shared.lock().unwrap();
                    (state.generation != state.saved)
                        .then(|| (state.generation, state.data.clone()))
                };
                if let Some((generation, data)) = snapshot {
                    let result = serde_json::to_vec(&data)
                        .map_err(|e| e.to_string())
                        .and_then(|bytes| {
                            let temp = path.with_extension(format!("{}.tmp", std::process::id()));
                            fs::write(&temp, bytes)
                                .and_then(|_| fs::rename(&temp, &path))
                                .map_err(|e| e.to_string())
                        });
                    match result {
                        Ok(()) => shared.lock().unwrap().saved = generation,
                        Err(error) => eprintln!("[Perception] Unable to save impressions: {error}"),
                    }
                }
                if stopping {
                    break;
                }
            }
        });
        Self(Arc::new(Owner {
            state,
            stop,
            worker: Mutex::new(Some(worker)),
        }))
    }
    pub(crate) fn offsets(&self, key: &str) -> Vec<Offset> {
        let state = self.0.state.lock().unwrap();
        state
            .data
            .offsets
            .get(key)
            .filter(|entry| !entry.chaotic)
            .map(|entry| entry.offsets.clone())
            .unwrap_or_default()
    }
    pub(crate) fn scale(&self, key: &str) -> Option<ScaleImpression> {
        self.0.state.lock().unwrap().data.scales.get(key).copied()
    }

    pub(crate) fn observe_scale(&self, key: &str, impression: ScaleImpression) {
        let mut state = self.0.state.lock().unwrap();
        if state.data.scales.get(key) == Some(&impression) {
            return;
        }
        state.data.scales.insert(key.to_owned(), impression);
        state.generation += 1;
    }
    pub(crate) fn observe(&self, key: &str, offset: Offset) {
        if offset == (Offset { x: 0, y: 0 }) {
            return;
        }
        let mut state = self.0.state.lock().unwrap();
        let entry = state.data.offsets.entry(key.to_owned()).or_default();
        if entry.chaotic {
            return;
        }
        if entry.offsets.first() == Some(&offset) {
            return;
        }
        if let Some(index) = entry.offsets.iter().position(|item| *item == offset) {
            entry.offsets.remove(index);
        }
        entry.offsets.insert(0, offset);
        if entry.offsets.len() > CHAOS_LIMIT {
            entry.chaotic = true;
            entry.offsets.clear();
        }
        state.generation += 1;
    }
}

pub(crate) struct ImpressionPlugin(pub ImpressionStore);
impl Plugin for ImpressionPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.0.clone());
    }
}
