use std::fs;
use std::path::PathBuf;

const FILE_NAME: &str = "thrive.ini";

pub fn sound_pack() -> String {
    read_key("soundpack").unwrap_or_else(|| "default".to_string())
}

pub fn set_sound_pack(name: &str) -> Result<(), String> {
    let mut lines = fs::read_to_string(FILE_NAME)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim_start().starts_with("soundpack"))
        .map(|line| line.to_string())
        .collect::<Vec<_>>();
    lines.push(format!("soundpack={name}"));
    fs::write(FILE_NAME, lines.join("\n") + "\n").map_err(|error| error.to_string())
}

fn read_key(key: &str) -> Option<String> {
    let text = fs::read_to_string(FILE_NAME).ok()?;
    for line in text.lines() {
        let line = line.trim();
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        if name.trim() == key {
            return Some(value.trim().to_string());
        }
    }
    None
}

pub fn sounds_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = std::env::current_exe().ok().and_then(|path| path.parent().map(|dir| dir.join("sounds"))) {
        dirs.push(dir);
    }
    let working = PathBuf::from("sounds");
    if dirs.iter().all(|dir| dir != &working) {
        dirs.push(working);
    }
    dirs
}

pub fn sound_packs() -> Vec<String> {
    let mut packs = Vec::new();
    for root in sounds_dirs() {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.path().is_dir() && name.to_ascii_lowercase().starts_with("mastodon-") {
                let short = name[9..].to_string();
                if !packs.contains(&short) {
                    packs.push(short);
                }
            }
        }
    }
    packs.sort();
    if packs.is_empty() {
        packs.push("default".to_string());
    }
    packs
}

pub fn sound_file(pack: &str, file_name: &str) -> Option<PathBuf> {
    for root in sounds_dirs() {
        for folder in [format!("Mastodon-{pack}"), format!("mastodon-{pack}")] {
            let nested = root.join(folder).join(file_name);
            if nested.is_file() {
                return Some(nested);
            }
        }
        let flat = root.join(file_name);
        if flat.is_file() {
            return Some(flat);
        }
    }
    None
}
