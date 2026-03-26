pub mod assets;

pub use assets::{ScriptAssets, SoundAssets, asset_dir, credentials_path};

// Get or create cached PowerShell script file
pub async fn get_powershell_script()
-> Result<std::path::PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    use std::io::Write;

    let cache_dir = utils_core::cache_dir();
    let script_path = cache_dir.join("toast-notification.ps1");

    // Check if cached file already exists and is valid
    if script_path.exists() {
        // Verify file has content (basic validation)
        if let Ok(metadata) = std::fs::metadata(&script_path)
            && metadata.len() > 0
        {
            return Ok(script_path);
        }
    }

    // File doesn't exist or is invalid, create it
    let script_content = ScriptAssets::get("toast-notification.ps1")
        .ok_or("Embedded PowerShell script not found: toast-notification.ps1")?
        .data;

    // Ensure cache directory exists
    std::fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("Failed to create cache directory: {e}"))?;

    let mut file = std::fs::File::create(&script_path)
        .map_err(|e| format!("Failed to create PowerShell script file: {e}"))?;

    file.write_all(&script_content)
        .map_err(|e| format!("Failed to write PowerShell script data: {e}"))?;

    drop(file); // Ensure file is closed

    Ok(script_path)
}
