//! Movie playback hands off to an external player (mpv, VLC, ffplay, or the system's default).
//! Decoding video in-process would need FFmpeg; this keeps the binary dependency-free.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

fn find_in_path(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths).map(|d| d.join(name)).find(|p| p.is_file())
}

fn spawn(cmd: &mut Command) -> Option<Child> {
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().ok()
}

/// Starts playing `path` fullscreen. Returns the child process and the player's name.
pub fn launch(path: &Path) -> Result<(Child, &'static str), String> {
    if let Some(mpv) = find_in_path("mpv") {
        if let Some(c) = spawn(Command::new(mpv).args(["--fs", "--really-quiet", "--no-terminal"]).arg(path)) {
            return Ok((c, "mpv"));
        }
    }
    let vlc_candidates = [find_in_path("vlc"), Some(PathBuf::from("/Applications/VLC.app/Contents/MacOS/VLC"))];
    for vlc in vlc_candidates.into_iter().flatten() {
        if vlc.is_file() {
            if let Some(c) = spawn(Command::new(vlc).args(["--fullscreen", "--play-and-exit", "--quiet"]).arg(path)) {
                return Ok((c, "VLC"));
            }
        }
    }
    if let Some(ff) = find_in_path("ffplay") {
        if let Some(c) = spawn(Command::new(ff).args(["-fs", "-autoexit", "-loglevel", "quiet"]).arg(path)) {
            return Ok((c, "ffplay"));
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(c) = spawn(Command::new("open").arg("-W").arg(path)) {
            return Ok((c, "the default player"));
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(x) = find_in_path("xdg-open") {
            if let Some(c) = spawn(Command::new(x).arg(path)) {
                return Ok((c, "the default player"));
            }
        }
    }
    Err("No video player found. Install mpv, VLC or ffmpeg (ffplay) to play movies.".into())
}
