# Marquee

A living-room media centre in Rust for **macOS and Linux**. It started as a port of
[JacksonDam/FirstRow](https://github.com/JacksonDam/FirstRow) (the Front Row recreation) and has since been
restyled and extended: in-app video playback with thumbnails, a Network section that finds other computers and
plays their libraries, and installable Apps such as YouTube.

Two programs are built:

| Binary | Purpose |
| --- | --- |
| `marquee` | The media centre (menus, music, photos, video, Network, Apps) |
| `marquee-share` | Pick folders or individual videos/music/photos and share them as libraries |

## Requirements

- Rust (stable) to build
- **FFmpeg** (`ffmpeg` + `ffprobe`) for video playback and thumbnails
- **yt-dlp** for the YouTube / SoundCloud / Open Link apps (optional)
- Linux build packages: `libasound2-dev pkg-config libx11-dev libxi-dev libgl1-mesa-dev`

```bash
brew install ffmpeg yt-dlp        # macOS      |   sudo apt install ffmpeg yt-dlp   # Debian/Ubuntu
cargo run --release --bin marquee
cargo run --release --bin marquee-share
```

## Sharing a library over the network

1. On the computer that holds your videos, run `marquee-share`. A page opens at `http://127.0.0.1:8765`.
2. Create a library: name it, pick a type, browse your disk and add folders and/or single files.
3. On any other computer on the same network open **Network** in Marquee. The computer appears by itself
   (UDP broadcast on port 48765); open it, then browse and play. If broadcasts are blocked, use
   **Network > Add Computer…** and type its IP address.

Network libraries stream the same way. Libraries are plain HTTP with Range support (port 8765, or the next free one). The admin page and its
endpoints only accept connections from the same machine; the library endpoints are open to your local network,
so only run it on networks you trust.

## Apps

**Network > Apps** lists installable apps (YouTube, SoundCloud, Open Link). Installing one adds it to your list;
The **YouTube** app follows the classic Apple TV layout: Featured, Most Viewed, Most Recent, Top Rated, Subscriptions,
History, Search and Search Channels (open a channel to subscribe and browse its videos; press **C** on any video to open
its channel), with a scrolling wall of thumbnails on the left. SoundCloud and Open Link are simpler. Playback happens inside
Marquee and is **streamed live, never downloaded**: yt-dlp finds the video and audio streams, and one FFmpeg process
reads them directly over HTTP (up to 4K, any length; seeking restarts the stream at the new position, so an 8-hour
video is as quick to skip around in as a short one). The picture is decoded in hardware where FFmpeg can, and
fitted to your window. **Settings > Video Quality** chooses: Auto (best stream, shown up to 1080p), Ultra (4K output
for large windows, needs a fast computer), 1080p, or 720p for slower machines.

## Icons

The app icon is `assets/appicon/marquee.svg`. All symbols in the interface (chevrons, folder, play/pause, search, ...)
are **Adwaita symbolic icons** from the GNOME `adwaita-icon-theme`, rasterised from SVG at exactly the size they are
shown and snapped to whole device pixels, so they stay crisp at any window size. Those SVGs are licensed
CC BY-SA 3.0 / LGPL-3.0 (copies in `assets/adwaita/`).

- macOS: `packaging/macos/make-app.sh` builds `dist/Marquee.app` with the icon.
- Linux: `packaging/linux/install.sh` installs the binaries, icon and a menu entry for the current user.

## Controls

| Key | Action |
| --- | --- |
| Arrow keys | Navigate (hold to repeat); in video: Left/Right ±10 s, Up/Down ±60 s |
| Enter / Space | Select; pause/resume in players |
| Esc / Backspace | Back |
| Ctrl/Cmd + Esc or Q | Quit |
| F11 / Alt+Enter | Fullscreen |

## Media folders

`~/Movies`, `~/Music`, `~/Pictures` plus `Movies`/`Music`/`Pictures` on mounted volumes; add more in
**Settings > Media Folders** or via `MARQUEE_MOVIES`, `MARQUEE_MUSIC`, `MARQUEE_PHOTOS`.
`MARQUEE_NO_DEFAULTS=1` ignores the default locations. Settings: `<config dir>/marquee/settings.json`.

## Not included

Podcasts, Apple Music / Photos / iTunes / iCloud integrations, Siri Remote / game-controller / touch input and
the iOS/tvOS/visionOS targets (Apple-only APIs; iTunes and iCloud features were removed on purpose).
Subtitles and audio-track selection are not implemented in the video player yet.

## Testing without a keyboard

`marquee --script "wait:0.5,down,enter,type:hello_world" --shot out.png@4 --exit-at 4.5`
(`up down left right enter back space`, `wait:<s>`, `type:<text>` with `_` as space).

## Licensing

- **Marquee's own code and original artwork: All Rights Reserved** (`LICENSE`). That covers `src/`, `tools/`,
  `packaging/`, the app icon, the carousel icons, the selection bar, and the generated sounds, music and pictures.
- **Everything else keeps its own licence** (`THIRD_PARTY_NOTICES.md`): the FirstRow-derived carousel and layout
  logic (CC BY 4.0, attribution required), the Adwaita icons (CC BY-SA 3.0 / LGPL-3.0), the Lato font (SIL OFL 1.1),
  the YouTube logo (Google trademark), and all Rust crates (mostly MIT / Apache-2.0).
- No artwork, sound or other media from Apple or from FirstRow is included.
- FFmpeg and yt-dlp are not bundled; they are separate programs with their own licences.
