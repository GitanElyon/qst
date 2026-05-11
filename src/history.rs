use dirs::config_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct History {
    pub usage: HashMap<String, u64>,
    #[serde(default)]
    pub favorites: Vec<String>,
}

impl History {
    pub fn load() -> Self {
        if let Some(mut path) = config_dir() {
            path.push("qst");
            path.push("history.toml");
            if path.exists() {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(history) = toml::from_str(&content) {
                        return history;
                    }
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        if let Some(mut path) = config_dir() {
            path.push("qst");
            if fs::create_dir_all(&path).is_ok() {
                path.push("history.toml");
                if let Ok(content) = toml::to_string(self) {
                    let _ = fs::write(path, content);
                }
            }
        }
    }

    pub fn increment(&mut self, app_name: &str) {
        *self.usage.entry(app_name.to_string()).or_insert(0) += 1;
        self.save();
    }

    pub fn get_count(&self, app_name: &str) -> u64 {
        *self.usage.get(app_name).unwrap_or(&0)
    }

    pub fn toggle_favorite(&mut self, app_name: &str) {
        if let Some(pos) = self.favorites.iter().position(|x| x == app_name) {
            self.favorites.remove(pos);
        } else {
            self.favorites.push(app_name.to_string());
        }
        self.save();
    }

    pub fn is_favorite(&self, app_name: &str) -> bool {
        self.favorites.contains(&app_name.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_updates_usage_and_favorites_in_memory() {
        let mut history = History::default();

        assert_eq!(history.get_count("Terminal"), 0);
        assert!(!history.is_favorite("Terminal"));

        history.increment("Terminal");
        history.increment("Terminal");
        assert_eq!(history.get_count("Terminal"), 2);

        history.toggle_favorite("Terminal");
        assert!(history.is_favorite("Terminal"));

        history.toggle_favorite("Terminal");
        assert!(!history.is_favorite("Terminal"));
    }

    #[test]
    fn history_toml_roundtrip_preserves_usage_and_favorites() {
        let mut usage = HashMap::new();
        usage.insert("Terminal".to_string(), 3);
        usage.insert("Editor".to_string(), 1);

        let history = History {
            usage,
            favorites: vec!["Editor".to_string()],
        };

        let content = toml::to_string(&history).expect("history should serialize");
        let loaded: History = toml::from_str(&content).expect("history should deserialize");

        assert_eq!(loaded.get_count("Terminal"), 3);
        assert_eq!(loaded.get_count("Editor"), 1);
        assert!(loaded.is_favorite("Editor"));
        assert!(!loaded.is_favorite("Terminal"));
    }
}
