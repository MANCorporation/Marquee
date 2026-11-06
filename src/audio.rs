//! Sound effects and the music player (rodio).

use crate::library::Track;
use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Cursor};
use std::sync::Arc;
use std::time::Duration;

pub struct Audio {
    stream: Option<OutputStream>,
    sfx: HashMap<&'static str, Arc<[u8]>>,
    pub sfx_enabled: bool,
}


impl Audio {
    pub fn new() -> Audio {
        let mut stream = OutputStreamBuilder::open_default_stream().ok();
        if let Some(s) = stream.as_mut() {
            s.log_on_drop(false);
        }
        let mut sfx = HashMap::new();
        for name in ["Selection", "SelectionChange", "Exit", "Limit"] {
            sfx.insert(name, Arc::<[u8]>::from(crate::synth::sfx(name)));
        }
        Audio { stream, sfx, sfx_enabled: true }
    }

    pub fn available(&self) -> bool {
        self.stream.is_some()
    }

    pub fn play_sfx(&self, name: &str) {
        if !self.sfx_enabled {
            return;
        }
        let (Some(stream), Some(bytes)) = (self.stream.as_ref(), self.sfx.get(name)) else { return };
        if let Ok(dec) = Decoder::new(Cursor::new(bytes.clone())) {
            let sink = Sink::connect_new(stream.mixer());
            sink.append(dec);
            sink.detach();
        }
    }

    pub fn new_sink(&self) -> Option<Sink> {
        self.sink()
    }

    fn sink(&self) -> Option<Sink> {
        self.stream.as_ref().map(|s| Sink::connect_new(s.mixer()))
    }
}

/// Plays a queue of tracks one after another.
pub struct Player {
    sink: Option<Sink>,
    pub queue: Vec<Track>,
    pub index: usize,
    pub shuffled: bool,
    pub volume: f32,
    paused: bool,
    pub error: Option<String>,
    pub generation: u64,
}

impl Player {
    pub fn new() -> Player {
        Player { sink: None, queue: vec![], index: 0, shuffled: false, volume: 1.0, paused: false, error: None, generation: 0 }
    }

    pub fn has_session(&self) -> bool {
        self.sink.is_some() && !self.queue.is_empty()
    }

    pub fn current(&self) -> Option<&Track> {
        self.queue.get(self.index)
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn play_queue(&mut self, audio: &Audio, tracks: Vec<Track>, start: usize, shuffled: bool) {
        self.queue = tracks;
        self.shuffled = shuffled;
        self.start_index(audio, start);
    }

    fn start_index(&mut self, audio: &Audio, index: usize) {
        self.generation += 1;
        if let Some(s) = self.sink.take() {
            s.stop();
        }
        self.error = None;
        if self.queue.is_empty() {
            return;
        }
        self.index = index.min(self.queue.len() - 1);
        let path = self.queue[self.index].path.clone();
        let Some(sink) = audio.sink() else {
            self.error = Some("No audio output device".into());
            return;
        };
        match File::open(&path).map_err(|e| e.to_string()).and_then(|f| Decoder::new(BufReader::new(f)).map_err(|e| e.to_string())) {
            Ok(dec) => {
                sink.set_volume(self.volume);
                sink.append(dec);
                self.paused = false;
                self.sink = Some(sink);
            }
            Err(e) => {
                self.error = Some(format!("Can't play {}: {e}", path.display()));
                self.sink = None;
            }
        }
    }

    pub fn toggle_pause(&mut self) {
        if let Some(s) = &self.sink {
            if self.paused {
                s.play();
            } else {
                s.pause();
            }
            self.paused = !self.paused;
        }
    }

    pub fn next(&mut self, audio: &Audio) {
        if self.index + 1 < self.queue.len() {
            let i = self.index + 1;
            self.start_index(audio, i);
        }
    }

    pub fn prev(&mut self, audio: &Audio) {
        if self.position() > 3.0 {
            self.seek_to(0.0);
        } else if self.index > 0 {
            let i = self.index - 1;
            self.start_index(audio, i);
        }
    }

    pub fn position(&self) -> f32 {
        self.sink.as_ref().map(|s| s.get_pos().as_secs_f32()).unwrap_or(0.0)
    }

    pub fn duration(&self) -> f32 {
        self.current().map(|t| t.duration).unwrap_or(0.0)
    }

    pub fn seek_to(&mut self, secs: f32) {
        let dur = self.duration();
        let target = if dur > 0.0 { secs.clamp(0.0, (dur - 0.5).max(0.0)) } else { secs.max(0.0) };
        if let Some(s) = &self.sink {
            let _ = s.try_seek(Duration::from_secs_f32(target));
        }
    }

    pub fn seek_by(&mut self, delta: f32) {
        let p = self.position() + delta;
        self.seek_to(p);
    }

    pub fn stop(&mut self) {
        self.generation += 1;
        if let Some(s) = self.sink.take() {
            s.stop();
        }
        self.queue.clear();
        self.index = 0;
        self.paused = false;
    }

    /// Moves on when the current track ends. Returns true if the playing track changed.
    pub fn update(&mut self, audio: &Audio) -> bool {
        let ended = self.sink.as_ref().map(|s| s.empty() && !self.paused).unwrap_or(false);
        if ended {
            if self.index + 1 < self.queue.len() {
                let i = self.index + 1;
                self.start_index(audio, i);
                return true;
            } else {
                self.sink = None;
                return true;
            }
        }
        false
    }
}

/// Background music for slideshows and the screen saver.
pub struct Ambient {
    sink: Option<Sink>,
}

impl Ambient {
    pub fn new() -> Ambient {
        Ambient { sink: None }
    }

    pub fn start_default(&mut self, audio: &Audio) {
        self.stop();
        if let Some(sink) = audio.sink() {
            let pad = SamplesBuffer::new(2, 44100, crate::synth::ambient());
            sink.append(pad.repeat_infinite());
            sink.set_volume(0.8);
            self.sink = Some(sink);
        }
    }

    pub fn stop(&mut self) {
        if let Some(s) = self.sink.take() {
            s.stop();
        }
    }

    pub fn is_playing(&self) -> bool {
        self.sink.is_some()
    }
}
