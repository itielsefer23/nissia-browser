use std::path::PathBuf;

/// Returns the nissia data directory (~/.local/share/nissia or platform equivalent).
/// Creates it if it doesn't exist.
pub fn data_dir() -> PathBuf {
    let dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("nissia");
    std::fs::create_dir_all(&dir).ok();
    dir
}
