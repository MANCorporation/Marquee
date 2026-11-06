//! Background image decoding with a small texture cache.

use crate::gfx::{decode_file, decode_image, texture_from, Decoded};
use macroquad::prelude::Texture2D;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

#[derive(Clone, Debug)]
pub enum Source {
    File(PathBuf),
    /// Embedded cover art of an audio file (or folder art next to it).
    Cover(PathBuf),
    Bytes(&'static [u8]),
    /// A frame grabbed from a video file or URL with ffmpeg.
    Thumb(String),
    /// An image downloaded over HTTP.
    Url(String),
    /// A generated abstract picture (screen saver without photos).
    Generated(u32),
}

pub struct TexCache {
    map: HashMap<String, Option<Texture2D>>,
    order: VecDeque<String>,
    requested: std::collections::HashSet<String>,
    tx: Sender<(String, Option<Decoded>)>,
    rx: Receiver<(String, Option<Decoded>)>,
    cap: usize,
}

impl TexCache {
    pub fn new(cap: usize) -> TexCache {
        let (tx, rx) = channel();
        TexCache { map: HashMap::new(), order: VecDeque::new(), requested: Default::default(), tx, rx, cap }
    }

    pub fn poll(&mut self) {
        while let Ok((key, dec)) = self.rx.try_recv() {
            let tex = dec.as_ref().map(texture_from);
            self.requested.remove(&key);
            self.map.insert(key.clone(), tex);
            self.order.push_back(key);
            while self.order.len() > self.cap {
                if let Some(old) = self.order.pop_front() {
                    self.map.remove(&old);
                }
            }
        }
    }

    /// The texture if it has loaded; otherwise starts loading it. `Some(None)` means it failed to decode.
    pub fn get(&mut self, key: &str, src: Source, max_side: u32) -> Option<Option<&Texture2D>> {
        if self.map.contains_key(key) {
            return self.map.get(key).map(|o| o.as_ref());
        }
        if self.requested.insert(key.to_string()) {
            let tx = self.tx.clone();
            let key = key.to_string();
            std::thread::spawn(move || {
                let dec = match src {
                    Source::File(p) => decode_file(&p, max_side),
                    Source::Cover(p) => crate::library::cover_art(&p).and_then(|b| decode_image(&b, max_side)),
                    Source::Bytes(b) => decode_image(b, max_side),
                    Source::Thumb(s) => crate::ffmpeg::thumbnail(&s).and_then(|b| decode_image(&b, max_side)),
                    Source::Generated(seed) => Some(Decoded { w: 640, h: 400, rgba: crate::synth::card(seed, 640, 400) }),
                    Source::Url(u) => crate::net::fetch_bytes(&u, 30_000_000).ok().and_then(|b| decode_image(&b, max_side)),
                };
                let _ = tx.send((key, dec));
            });
        }
        None
    }

    pub fn ready(&self, key: &str) -> bool {
        matches!(self.map.get(key), Some(Some(_)))
    }

    pub fn peek(&self, key: &str) -> Option<&Texture2D> {
        self.map.get(key).and_then(|o| o.as_ref())
    }
}
