//! The menu catalogue: the root sections and their submenus (port of the `*MenuFeature` structs).

#[derive(Clone, Debug)]
pub struct RootItem {
    pub id: &'static str,
    pub title: &'static str,
    pub icon: &'static str,
}

#[derive(Clone, Debug)]
pub struct SubItem {
    pub id: String,
    pub title: String,
    pub leads_to_menu: bool,
    pub trailing_text: Option<String>,
    pub shuffle_glyph: bool,
    pub top_divider: bool,
}

pub fn sub(id: &str, title: &str, leads: bool) -> SubItem {
    SubItem { id: id.to_string(), title: title.to_string(), leads_to_menu: leads, trailing_text: None, shuffle_glyph: false, top_divider: false }
}

pub fn dots(mut s: SubItem) -> SubItem {
    s.trailing_text = Some("•••".into());
    s
}

pub fn trail(mut s: SubItem, t: &str) -> SubItem {
    s.trailing_text = Some(t.to_string());
    s
}

pub fn device_name() -> String {
    for var in ["HOSTNAME", "HOST", "COMPUTERNAME"] {
        if let Ok(v) = std::env::var(var) {
            if !v.is_empty() {
                return v;
            }
        }
    }
    if let Ok(out) = std::process::Command::new("hostname").output() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() {
            return s;
        }
    }
    "This Computer".into()
}

pub fn root_items() -> Vec<RootItem> {
    vec![
        RootItem { id: "movies", title: "Movies", icon: "movies" },
        RootItem { id: "music", title: "Music", icon: "music" },
        RootItem { id: "photos", title: "Photos", icon: "photos" },
        RootItem { id: "settings", title: "Settings", icon: "settings" },
        RootItem { id: "network", title: "Network", icon: "sources" },
    ]
}

/// Static submenus. The Network list is completed with discovered computers by the app.
pub fn submenu_items(root: &str, sound_on: bool, saver_on: bool, quality: &str) -> Vec<SubItem> {
    match root {
        "movies" => vec![sub("movies_folder", "Movies Folder", true)],
        "music" => {
            let mut shuffle = sub("music_shuffle_songs", "Shuffle Songs", false);
            shuffle.shuffle_glyph = true;
            vec![
                shuffle,
                sub("music_music_videos", "Music Videos", true),
                sub("music_playlists", "Playlists", true),
                sub("music_artists", "Artists", true),
                sub("music_albums", "Albums", true),
                sub("music_songs", "Songs", true),
                sub("music_genres", "Genres", true),
                sub("music_composers", "Composers", true),
                sub("music_audiobooks", "Audiobooks", true),
            ]
        }
        "photos" => vec![
            sub("photos_library", "Photos", true),
            sub("photos_last_12_months", "Last 12 Months", false),
            sub("photos_last_import", "Last Import", false),
        ],
        "settings" => vec![
            trail(sub("settings_name", "Name", false), &device_name()),
            trail(sub("settings_swversion", "Version", false), env!("CARGO_PKG_VERSION")),
            trail(sub("settings_screensaver", "Screen Saver", false), if saver_on { "On" } else { "Off" }),
            trail(sub("settings_soundeffects", "Sound Effects", false), if sound_on { "On" } else { "Off" }),
            trail(sub("settings_quality", "Video Quality", false), quality),
            trail(sub("settings_media_folders", "Media Folders", false), "..."),
        ],
        "network" => {
            let mut apps = sub("network_apps", "Apps", true);
            apps.top_divider = false;
            vec![apps, sub("network_add", "Add Computer…", false)]
        }
        _ => vec![],
    }
}

pub fn default_submenu_index(_root: &str) -> usize {
    0
}
