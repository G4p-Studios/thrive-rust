use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const FILE_NAME: &str = "user.dat";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub client_id: String,
    pub client_secret: String,
    pub access_token: String,
    pub instance_url: String,
    pub username: String,
}

pub fn path() -> PathBuf {
    PathBuf::from(FILE_NAME)
}

pub fn load() -> Option<Session> {
    let bytes = fs::read(path()).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn save(session: &Session) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(session).map_err(|error| error.to_string())?;
    fs::write(path(), bytes).map_err(|error| error.to_string())
}
