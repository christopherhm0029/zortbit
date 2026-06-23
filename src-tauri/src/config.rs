// Zortbit "brain" config — seeded from the onboarding questionnaire.
// Persisted to ~/Library/Application Support/com.xaviour.zortbit/config.json
// so the user can edit it, and the AI reads it on every decision.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub automation: String, // "propose" | "semi" | "auto"
    pub naming: String,     // "kebab"
    pub grouping: String,   // "by_owner"
    pub model: String,      // "qwen2.5:3b"
    pub bulk_scope: Vec<String>,
    pub protected: Vec<String>,
    #[serde(default = "default_true")]
    pub never_auto_delete: bool, // ALWAYS true — deletions need explicit per-file approval
    #[serde(default = "default_true")]
    pub learn_from_decisions: bool,
    #[serde(default = "default_base")]
    pub organize_base: String, // single LOCAL (non-iCloud) root for organized files
    #[serde(default = "default_categories")]
    pub categories: Vec<String>, // closed list of project/area folders qwen may choose from
    #[serde(default = "default_provider")]
    pub provider: String, // "ollama" (native) | "openai" (OpenAI-compatible: Foundry Local, LM Studio…)
    #[serde(default)]
    pub endpoint: String, // OpenAI-compatible chat URL (used when provider != "ollama")
    #[serde(default)]
    pub api_key: String, // optional bearer token (local servers usually need none)
}

fn default_provider() -> String {
    "ollama".into()
}

fn default_true() -> bool {
    true
}

fn default_base() -> String {
    "Organized".into()
}

fn default_categories() -> Vec<String> {
    // Generic starter buckets. Users tailor these to their own projects/areas
    // in config.json (e.g. add "Acme", "Side-Project", a client name, …).
    [
        "Work", "Finance", "Projects", "Learning", "Personal", "Media", "Other",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

impl Default for Config {
    fn default() -> Self {
        Config {
            automation: "propose".into(),
            naming: "kebab".into(),
            grouping: "by_owner".into(),
            model: "qwen2.5:3b".into(),
            bulk_scope: vec![
                "Downloads".into(),
                "Documents".into(),
                "Documents/Screenshots".into(),
            ],
            protected: vec![
                "Desktop".into(),
                "Documents/Personal-Projects".into(),
                "Documents/SE Vault".into(),
                "Library/Mobile Documents".into(), // iCloud Drive
            ],
            never_auto_delete: true,
            learn_from_decisions: true,
            organize_base: "Organized".into(),
            categories: default_categories(),
            provider: "ollama".into(),
            endpoint: String::new(),
            api_key: String::new(),
        }
    }
}

fn config_file() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| dirs::home_dir().expect("home"));
    base.join("com.xaviour.zortbit").join("config.json")
}

impl Config {
    pub fn load_or_init() -> Config {
        let p = config_file();
        if let Ok(txt) = std::fs::read_to_string(&p) {
            if let Ok(cfg) = serde_json::from_str::<Config>(&txt) {
                return cfg;
            }
        }
        let cfg = Config::default();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(txt) = serde_json::to_string_pretty(&cfg) {
            let _ = std::fs::write(&p, txt);
        }
        cfg
    }

    pub fn protected_paths(&self, home: &Path) -> Vec<PathBuf> {
        self.protected.iter().map(|r| home.join(r)).collect()
    }
}
