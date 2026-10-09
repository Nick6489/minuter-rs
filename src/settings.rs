use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub tick: bool,
    pub low: bool,
    pub warning: bool,
    pub speak: bool,
    pub minute: bool,
    /// None follows the system default; named devices persist across restarts.
    pub device: Option<String>,
}
impl Settings {
    pub fn mask(&self) -> u32 {
        self.tick as u32
            | (self.low as u32) << 1
            | (self.warning as u32) << 2
            | (self.speak as u32) << 3
            | (self.minute as u32) << 4
    }
    pub fn set_all(&mut self, enabled: bool) {
        self.tick = enabled;
        self.low = enabled;
        self.warning = enabled;
        self.speak = enabled;
        self.minute = enabled;
    }
    pub fn load() -> Result<Self, String> {
        match fs::read(path()) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| format!("Could not read saved settings: {e}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("Could not read saved settings: {e}")),
        }
    }
    pub fn save(&self) -> Result<(), String> {
        let file = path();
        fs::create_dir_all(file.parent().unwrap()).map_err(|e| e.to_string())?;
        let temp = file.with_extension("tmp");
        fs::write(
            &temp,
            serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        fs::rename(temp, file).map_err(|e| format!("Could not save settings: {e}"))
    }
}
pub fn path() -> PathBuf {
    #[cfg(windows)]
    let root = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    #[cfg(target_os = "macos")]
    let root = PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
        .join("Library/Application Support");
    #[cfg(not(any(windows, target_os = "macos")))]
    let root = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        });
    #[cfg(all(feature = "rungui-ui", not(feature = "wx-ui")))]
    let folder = "Minuter-rungui";
    #[cfg(not(all(feature = "rungui-ui", not(feature = "wx-ui"))))]
    let folder = "Minuter";
    root.join(folder).join("settings.json")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_compatible_with_missing_fields() {
        let mut s: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(s.mask(), 0);
        s.set_all(true);
        assert_eq!(s.mask(), 31);
        assert_eq!(
            serde_json::from_str::<Settings>(&serde_json::to_string(&s).unwrap())
                .unwrap()
                .mask(),
            31
        );
        s.set_all(false);
        assert_eq!(s.mask(), 0);
    }
}
