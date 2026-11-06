//! marquee-share: choose folders and videos on this computer and share them as libraries.
//! Other computers running Marquee find it automatically (Network section) and can browse and play them.
//!
//! Run it, pick your libraries in the page that opens (http://127.0.0.1:8765), and leave it running.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{Read, Seek, SeekFrom};
use std::net::{IpAddr, UdpSocket};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

const VIDEO: &[&str] = &["mp4", "m4v", "mov", "mkv", "avi", "webm", "mpg", "mpeg", "wmv", "ts", "flv", "ogv"];
const AUDIO: &[&str] = &["mp3", "m4a", "m4b", "aac", "flac", "ogg", "oga", "opus", "wav", "aif", "aiff"];
const PHOTO: &[&str] = &["jpg", "jpeg", "png", "gif", "webp"];

#[derive(Clone, Serialize, Deserialize)]
struct Lib {
    id: String,
    name: String,
    kind: String,
    paths: Vec<PathBuf>,
}

struct State {
    libs: Vec<Lib>,
    next_id: u32,
}

fn config_path() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("marquee-share").join("libraries.json")
}

fn load() -> State {
    let libs: Vec<Lib> = std::fs::read_to_string(config_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let next_id = libs.iter().filter_map(|l| l.id.parse::<u32>().ok()).max().map(|m| m + 1).unwrap_or(0);
    State { libs, next_id }
}

fn save(s: &State) {
    let p = config_path();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(p, serde_json::to_string_pretty(&s.libs).unwrap_or_default());
}

fn ext(p: &Path) -> String {
    p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

fn matches_kind(kind: &str, p: &Path) -> bool {
    let e = ext(p);
    let e = e.as_str();
    match kind {
        "music" => AUDIO.contains(&e),
        "photos" => PHOTO.contains(&e),
        _ => VIDEO.contains(&e),
    }
}

fn hostname() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Computer".into())
}

fn query(url: &str, key: &str) -> Option<String> {
    let q = url.split_once('?')?.1;
    for pair in q.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == key {
                return Some(decode(v));
            }
        }
    }
    None
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                let h = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
                if let Some(v) = h {
                    out.push(v);
                    i += 3;
                    continue;
                }
                out.push(b'%');
            }
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn header(k: &str, v: &str) -> Header {
    Header::from_bytes(k.as_bytes(), v.as_bytes()).unwrap()
}

fn json_resp(v: Value) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(v.to_string()).with_header(header("Content-Type", "application/json")).with_header(header("Access-Control-Allow-Origin", "*"))
}

fn mime(p: &Path) -> &'static str {
    match ext(p).as_str() {
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "m4a" | "m4b" | "aac" => "audio/mp4",
        "flac" => "audio/flac",
        "ogg" | "oga" | "opus" => "audio/ogg",
        "wav" => "audio/wav",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
}

/// Resolves "idx" or "idx/rel/path" inside a library without allowing `..` escapes.
fn resolve(lib: &Lib, p: &str) -> Option<PathBuf> {
    let mut parts = p.splitn(2, '/');
    let idx: usize = parts.next()?.parse().ok()?;
    let base = lib.paths.get(idx)?;
    let rel = parts.next().unwrap_or("");
    if base.is_file() {
        return if rel.is_empty() { Some(base.clone()) } else { None };
    }
    let mut out = base.clone();
    for c in Path::new(rel).components() {
        match c {
            Component::Normal(s) => out.push(s),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(out)
}

fn list_lib(lib: &Lib, p: &str) -> Value {
    let mut entries = vec![];
    if p.is_empty() {
        for (i, path) in lib.paths.iter().enumerate() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("Folder").to_string();
            entries.push(json!({"name": name, "dir": path.is_dir(), "p": i.to_string()}));
        }
    } else if let Some(dir) = resolve(lib, p) {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            let mut v: Vec<(String, PathBuf, bool)> = rd
                .flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if name.starts_with('.') {
                        return None;
                    }
                    let path = e.path();
                    let is_dir = path.is_dir();
                    (is_dir || matches_kind(&lib.kind, &path)).then_some((name, path, is_dir))
                })
                .collect();
            v.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.to_lowercase().cmp(&b.0.to_lowercase())));
            for (name, _, is_dir) in v {
                entries.push(json!({"name": name, "dir": is_dir, "p": format!("{p}/{name}")}));
            }
        }
    }
    json!({ "entries": entries })
}

fn serve_file(req: Request, path: &Path) {
    let Ok(mut file) = std::fs::File::open(path) else {
        let _ = req.respond(Response::from_string("not found").with_status_code(404));
        return;
    };
    let total = file.metadata().map(|m| m.len()).unwrap_or(0);
    let range = req.headers().iter().find(|h| h.field.equiv("Range")).map(|h| h.value.to_string());
    let (start, end, partial) = match range.as_deref().and_then(|r| r.strip_prefix("bytes=")) {
        Some(r) => {
            let (a, b) = r.split_once('-').unwrap_or((r, ""));
            let start: u64 = a.parse().unwrap_or(0);
            let end: u64 = b.parse().unwrap_or(total.saturating_sub(1)).min(total.saturating_sub(1));
            (start, end, true)
        }
        None => (0, total.saturating_sub(1), false),
    };
    if start >= total && total > 0 {
        let _ = req.respond(Response::from_string("").with_status_code(416));
        return;
    }
    let len = if total == 0 { 0 } else { end - start + 1 };
    let _ = file.seek(SeekFrom::Start(start));
    let body = file.take(len);
    let mut headers = vec![header("Content-Type", mime(path)), header("Accept-Ranges", "bytes")];
    let status = if partial {
        headers.push(header("Content-Range", &format!("bytes {start}-{end}/{total}")));
        StatusCode(206)
    } else {
        StatusCode(200)
    };
    let _ = req.respond(Response::new(status, headers, body, Some(len as usize), None));
}

fn is_local(req: &Request) -> bool {
    req.remote_addr().map(|a| match a.ip() {
        IpAddr::V4(v) => v.is_loopback(),
        IpAddr::V6(v) => v.is_loopback(),
    }).unwrap_or(false)
}

fn fs_listing(path: Option<String>) -> Value {
    let dir = path.map(PathBuf::from).filter(|p| p.is_dir()).or_else(dirs::home_dir).unwrap_or_else(|| PathBuf::from("/"));
    let mut dirs_v = vec![];
    let mut files = vec![];
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let p = e.path();
            if p.is_dir() {
                dirs_v.push(json!({"name": name, "path": p}));
            } else if matches_kind("movies", &p) || matches_kind("music", &p) || matches_kind("photos", &p) {
                files.push(json!({"name": name, "path": p}));
            }
        }
    }
    let by_name = |a: &Value, b: &Value| a["name"].as_str().unwrap_or("").to_lowercase().cmp(&b["name"].as_str().unwrap_or("").to_lowercase());
    dirs_v.sort_by(by_name);
    files.sort_by(by_name);
    json!({"path": dir, "parent": dir.parent(), "dirs": dirs_v, "files": files})
}

fn handle(mut req: Request, state: Arc<Mutex<State>>, name: String) {
    let url = req.url().to_string();
    let path_only = url.split('?').next().unwrap_or("/").to_string();
    let method = req.method().clone();
    match (method, path_only.as_str()) {
        (Method::Get, "/") if is_local(&req) => {
            let _ = req.respond(Response::from_string(ADMIN_HTML).with_header(header("Content-Type", "text/html; charset=utf-8")));
        }
        (Method::Get, "/api/info") => {
            let _ = req.respond(json_resp(json!({"name": name, "version": 1})));
        }
        (Method::Get, "/api/libraries") => {
            let libs: Vec<Value> = state.lock().unwrap().libs.iter().map(|l| json!({"id": l.id, "name": l.name, "kind": l.kind})).collect();
            let _ = req.respond(json_resp(Value::Array(libs)));
        }
        (Method::Get, "/api/ls") => {
            let lib_id = query(&url, "lib").unwrap_or_default();
            let p = query(&url, "p").unwrap_or_default();
            let lib = state.lock().unwrap().libs.iter().find(|l| l.id == lib_id).cloned();
            match lib {
                Some(l) => {
                    let _ = req.respond(json_resp(list_lib(&l, &p)));
                }
                None => {
                    let _ = req.respond(Response::from_string("no such library").with_status_code(404));
                }
            }
        }
        (Method::Get, p) if p.starts_with("/media/") => {
            let rest = decode(&p["/media/".len()..]);
            let (lib_id, rel) = rest.split_once('/').unwrap_or((&rest, ""));
            let lib = state.lock().unwrap().libs.iter().find(|l| l.id == lib_id).cloned();
            match lib.and_then(|l| resolve(&l, rel)) {
                Some(file) if file.is_file() => serve_file(req, &file),
                _ => {
                    let _ = req.respond(Response::from_string("not found").with_status_code(404));
                }
            }
        }
        (Method::Get, "/admin/fs") if is_local(&req) => {
            let _ = req.respond(json_resp(fs_listing(query(&url, "path"))));
        }
        (Method::Get, "/admin/libs") if is_local(&req) => {
            let libs = serde_json::to_value(&state.lock().unwrap().libs).unwrap_or_default();
            let _ = req.respond(json_resp(libs));
        }
        (Method::Post, "/admin/libs") if is_local(&req) => {
            let mut body = String::new();
            let _ = req.as_reader().read_to_string(&mut body);
            let v: Value = serde_json::from_str(&body).unwrap_or_default();
            let paths: Vec<PathBuf> = v["paths"].as_array().map(|a| a.iter().filter_map(|p| p.as_str().map(PathBuf::from)).collect()).unwrap_or_default();
            if paths.is_empty() {
                let _ = req.respond(Response::from_string("choose at least one folder or file").with_status_code(400));
                return;
            }
            let mut s = state.lock().unwrap();
            let id = s.next_id.to_string();
            s.next_id += 1;
            s.libs.push(Lib {
                id,
                name: v["name"].as_str().filter(|n| !n.trim().is_empty()).unwrap_or("Library").to_string(),
                kind: v["kind"].as_str().unwrap_or("movies").to_string(),
                paths,
            });
            save(&s);
            let _ = req.respond(json_resp(json!({"ok": true})));
        }
        (Method::Post, "/admin/delete") if is_local(&req) => {
            let id = query(&url, "id").unwrap_or_default();
            let mut s = state.lock().unwrap();
            s.libs.retain(|l| l.id != id);
            save(&s);
            let _ = req.respond(json_resp(json!({"ok": true})));
        }
        _ => {
            let _ = req.respond(Response::from_string("not found").with_status_code(404));
        }
    }
}

fn beacon(name: String, port: u16) {
    std::thread::spawn(move || {
        let Ok(sock) = UdpSocket::bind(("0.0.0.0", 0)) else { return };
        let _ = sock.set_broadcast(true);
        let msg = format!("MARQUEE1\t{name}\t{port}");
        loop {
            let _ = sock.send_to(msg.as_bytes(), ("255.255.255.255", 48765));
            let _ = sock.send_to(msg.as_bytes(), ("127.0.0.1", 48765));
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    });
}

fn main() {
    let mut server = None;
    let mut port = 8765u16;
    for p in 8765..8775 {
        if let Ok(s) = Server::http(("0.0.0.0", p)) {
            server = Some(s);
            port = p;
            break;
        }
    }
    let Some(server) = server else {
        eprintln!("Could not open a port (8765-8774 are all in use).");
        std::process::exit(1);
    };
    let name = std::env::var("MARQUEE_SHARE_NAME").unwrap_or_else(|_| hostname());
    let state = Arc::new(Mutex::new(load()));
    beacon(name.clone(), port);
    let url = format!("http://127.0.0.1:{port}");
    println!("Marquee Share is running as “{name}”.\nManage your libraries at {url}\nPress Ctrl+C to stop sharing.");
    if std::env::var_os("MARQUEE_SHARE_NO_BROWSER").is_none() {
        let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        let _ = std::process::Command::new(opener).arg(&url).spawn();
    }
    for req in server.incoming_requests() {
        let st = state.clone();
        let n = name.clone();
        std::thread::spawn(move || handle(req, st, n));
    }
}

const ADMIN_HTML: &str = include_str!("share_admin.html");
