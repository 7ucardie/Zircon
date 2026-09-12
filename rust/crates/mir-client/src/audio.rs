//! Sound playback (rodio): Zircon sound effects by `SoundIndex` number
//! (`Sound/<n>.wav`) and one looping music track.
//!
//! Without an output device (CI, headless) or with `ZIRCON_MUTE=1` every
//! call is a no-op, so the client never depends on audio working.

use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};

pub struct Audio {
    sink: Option<MixerDeviceSink>,
    root: PathBuf,
    cache: HashMap<u16, Option<Arc<[u8]>>>,
    music: Option<(u16, Player)>,
    effects: Vec<Player>,
    pub effect_volume: f32,
    pub music_volume: f32,
}

impl Audio {
    /// `root` is the client root holding `Sound/`.
    pub fn new(root: impl AsRef<Path>) -> Audio {
        let muted = std::env::var_os("ZIRCON_MUTE").is_some();
        let sink = if muted {
            None
        } else {
            match DeviceSinkBuilder::open_default_sink() {
                Ok(s) => Some(s),
                Err(e) => {
                    tracing::warn!("no audio output: {e}");
                    None
                }
            }
        };
        Audio {
            sink,
            root: root.as_ref().join("Sound"),
            cache: HashMap::new(),
            music: None,
            effects: Vec::new(),
            effect_volume: 0.6,
            music_volume: 0.4,
        }
    }

    fn bytes(&mut self, index: u16) -> Option<Arc<[u8]>> {
        if let Some(b) = self.cache.get(&index) {
            return b.clone();
        }
        let path = self.root.join(format!("{index}.wav"));
        let loaded = std::fs::read(&path).ok().map(Arc::<[u8]>::from);
        if loaded.is_none() {
            tracing::debug!(index, "missing sound file");
        }
        self.cache.insert(index, loaded.clone());
        loaded
    }

    /// Play a one-shot effect.
    #[allow(dead_code)] // wired to actions, magics and monsters next
    pub fn play(&mut self, index: u16) {
        if index == 0 || self.sink.is_none() {
            return;
        }
        let Some(bytes) = self.bytes(index) else {
            return;
        };
        let Ok(source) = Decoder::new_wav(Cursor::new(bytes)) else {
            return;
        };
        let sink = self.sink.as_ref().unwrap();
        let player = Player::connect_new(sink.mixer());
        player.set_volume(self.effect_volume);
        player.append(source);
        self.effects.retain(|p| !p.empty());
        // A hard cap keeps a burst of hits from piling up players.
        if self.effects.len() >= 24 {
            self.effects.remove(0);
        }
        self.effects.push(player);
    }

    /// Loop a music track; re-requesting the playing track is a no-op.
    pub fn play_music(&mut self, index: u16) {
        if self.sink.is_none() {
            return;
        }
        if index == 0 {
            self.stop_music();
            return;
        }
        if self.music.as_ref().map(|(i, _)| *i) == Some(index) {
            return;
        }
        self.stop_music();
        let Some(bytes) = self.bytes(index) else {
            return;
        };
        let Ok(source) = Decoder::new_wav(Cursor::new(bytes)) else {
            return;
        };
        let sink = self.sink.as_ref().unwrap();
        let player = Player::connect_new(sink.mixer());
        player.set_volume(self.music_volume);
        player.append(source.repeat_infinite());
        self.music = Some((index, player));
    }

    #[allow(dead_code)]
    pub fn stop_music(&mut self) {
        if let Some((_, p)) = self.music.take() {
            p.stop();
        }
    }
}
