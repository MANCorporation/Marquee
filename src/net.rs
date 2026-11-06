//! Network client: discovers computers running `marquee-share` and browses their libraries.
//!
//! Discovery: each share app broadcasts `MARQUEE1\t<name>\t<port>` over UDP port 48765 every 2 s.
//! Libraries are served over plain HTTP (with Range support, so ffmpeg can seek).

use serde_json::Value;
use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const DISCOVERY_PORT: u16 = 48765;

#[derive(Clone, Debug)]
pub struct Peer {
    pub name: String,
    pub addr: String, // ip:port
    seen: Instant,
}

#[derive(Clone)]
pub struct Discovery {
    peers: Arc<Mutex<HashMap<String, Peer>>>,
    pub listening: bool,
}

impl Discovery {
    pub fn start() -> Discovery {
        let peers: Arc<Mutex<HashMap<String, Peer>>> = Arc::default();
        let sock = UdpSocket::bind(("0.0.0.0", DISCOVERY_PORT)).ok();
        let listening = sock.is_some();
        if let Some(sock) = sock {
            let _ = sock.set_read_timeout(Some(Duration::from_millis(800)));
            let map = peers.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 512];
                loop {
                    if let Ok((n, from)) = sock.recv_from(&mut buf) {
                        let text = String::from_utf8_lossy(&buf[..n]).to_string();
                        let mut it = text.split('\t');
                        if it.next() == Some("MARQUEE1") {
                            let name = it.next().unwrap_or("Computer").to_string();
                            let port = it.next().unwrap_or("8765");
                            let addr = format!("{}:{}", from.ip(), port);
                            if let Ok(mut m) = map.lock() {
                                m.insert(addr.clone(), Peer { name, addr, seen: Instant::now() });
                            }
                        }
                    }
                    if let Ok(mut m) = map.lock() {
                        m.retain(|_, p| p.seen.elapsed() < Duration::from_secs(8));
                    }
                }
            });
        }
        Discovery { peers, listening }
    }

    pub fn peers(&self) -> Vec<Peer> {
        let mut v: Vec<Peer> = self.peers.lock().map(|m| m.values().cloned().collect()).unwrap_or_default();
        // The same computer is heard on several addresses (loopback and LAN): show it once.
        v.sort_by(|a, b| a.addr.starts_with("127.").cmp(&b.addr.starts_with("127.")).then(a.addr.cmp(&b.addr)));
        let mut seen = std::collections::HashSet::new();
        v.retain(|p| seen.insert((p.name.clone(), p.addr.rsplit(':').next().unwrap_or("").to_string())));
        v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.addr.cmp(&b.addr)));
        v
    }

    /// Registers a computer typed in by hand (for networks that block UDP broadcast).
    pub fn add_manual(&self, addr: &str) {
        let addr = if addr.contains(':') { addr.to_string() } else { format!("{addr}:8765") };
        if let Ok(mut m) = self.peers.lock() {
            m.insert(addr.clone(), Peer { name: addr.clone(), addr, seen: Instant::now() + Duration::from_secs(86400 * 365) });
        }
    }
}

pub fn enc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn fetch_bytes(url: &str, max: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let resp = ureq::get(url).timeout(Duration::from_secs(20)).call().map_err(|e| e.to_string())?;
    let mut buf = vec![];
    resp.into_reader().take(max).read_to_end(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}

fn get_json(url: &str) -> Result<Value, String> {
    ureq::get(url)
        .timeout(Duration::from_secs(10))
        .call()
        .map_err(|e| format!("Couldn't reach that computer ({e})"))?
        .into_json()
        .map_err(|e| e.to_string())
}

#[derive(Clone, Debug)]
pub struct RemoteLib {
    pub id: String,
    pub name: String,
    pub kind: String,
}

pub fn libraries(addr: &str) -> Result<Vec<RemoteLib>, String> {
    let v = get_json(&format!("http://{addr}/api/libraries"))?;
    Ok(v.as_array()
        .map(|a| {
            a.iter()
                .map(|l| RemoteLib {
                    id: l["id"].as_str().unwrap_or("").to_string(),
                    name: l["name"].as_str().unwrap_or("Library").to_string(),
                    kind: l["kind"].as_str().unwrap_or("movies").to_string(),
                })
                .collect()
        })
        .unwrap_or_default())
}

#[derive(Clone, Debug)]
pub struct RemoteEntry {
    pub name: String,
    pub dir: bool,
    pub p: String,
}

pub fn list(addr: &str, lib: &str, p: &str) -> Result<Vec<RemoteEntry>, String> {
    let v = get_json(&format!("http://{addr}/api/ls?lib={}&p={}", enc(lib), enc(p)))?;
    Ok(v["entries"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|e| RemoteEntry {
                    name: e["name"].as_str().unwrap_or("").to_string(),
                    dir: e["dir"].as_bool().unwrap_or(false),
                    p: e["p"].as_str().unwrap_or("").to_string(),
                })
                .collect()
        })
        .unwrap_or_default())
}

pub fn media_url(addr: &str, lib: &str, p: &str) -> String {
    format!("http://{addr}/media/{}/{}", enc(lib), enc(p))
}

/// Downloads a remote audio file into the cache so it can be played like a local track.
pub fn download_to_cache(url: &str, ext: &str) -> Result<std::path::PathBuf, String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    url.hash(&mut h);
    let dir = dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("marquee").join("downloads");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{:016x}.{ext}", h.finish()));
    if !path.is_file() {
        let bytes = fetch_bytes(url, 500_000_000)?;
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    }
    Ok(path)
}
