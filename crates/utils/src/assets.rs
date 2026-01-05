use directories::ProjectDirs;
use rust_embed::RustEmbed;

const PROJECT_ROOT: &str = env!("CARGO_MANIFEST_DIR");
const ASSET_DIR_ENV: &str = "VIBE_ASSET_DIR";

pub fn asset_dir() -> std::path::PathBuf {
    if let Ok(override_dir) = std::env::var(ASSET_DIR_ENV) {
        let override_dir = override_dir.trim();
        if !override_dir.is_empty() {
            let path = std::path::PathBuf::from(override_dir);
            if !path.exists() {
                std::fs::create_dir_all(&path)
                    .expect("Failed to create asset directory");
            }
            return path;
        }
    }

    let path = if cfg!(debug_assertions) {
        std::path::PathBuf::from(PROJECT_ROOT).join("../../dev_assets")
    } else {
        ProjectDirs::from("ai", "bloop", "vibe-kanban")
            .expect("OS didn't give us a home directory")
            .data_dir()
            .to_path_buf()
    };

    // Ensure the directory exists
    if !path.exists() {
        std::fs::create_dir_all(&path).expect("Failed to create asset directory");
    }

    path
    // ✔ macOS → ~/Library/Application Support/MyApp
    // ✔ Linux → ~/.local/share/myapp   (respects XDG_DATA_HOME)
    // ✔ Windows → %APPDATA%\Example\MyApp
}

pub fn config_path() -> std::path::PathBuf {
    asset_dir().join("config.json")
}

pub fn profiles_path() -> std::path::PathBuf {
    asset_dir().join("profiles.json")
}

pub fn credentials_path() -> std::path::PathBuf {
    asset_dir().join("credentials.json")
}

#[derive(RustEmbed)]
#[folder = "../../assets/sounds"]
pub struct SoundAssets;

#[derive(RustEmbed)]
#[folder = "../../assets/scripts"]
pub struct ScriptAssets;
