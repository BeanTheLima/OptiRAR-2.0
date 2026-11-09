use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Language {
    #[default]
    EnUs,
    ViVn,
}

impl Language {
    pub fn code(&self) -> &'static str {
        match self {
            Language::EnUs => "en-US",
            Language::ViVn => "vi-VN",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Language::EnUs => "English (en-US)",
            Language::ViVn => "Tiếng Việt (vi-VN)",
        }
    }

    pub fn from_code(code: &str) -> Self {
        if code.to_lowercase().starts_with("vi") {
            Language::ViVn
        } else {
            Language::EnUs
        }
    }
}

static EN_JSON: &str = include_str!("../../../assets/locales/en-US.json");
static VI_JSON: &str = include_str!("../../../assets/locales/vi-VN.json");

/// In-memory localization repository with fallback support.
pub struct I18nManager {
    current_lang: RwLock<Language>,
    translations_en: HashMap<String, String>,
    translations_vi: HashMap<String, String>,
}

impl I18nManager {
    pub fn new() -> Self {
        let mut en_map = HashMap::new();
        let mut vi_map = HashMap::new();

        if let Ok(v) = serde_json::from_str::<Value>(EN_JSON) {
            flatten_json("", &v, &mut en_map);
        }
        if let Ok(v) = serde_json::from_str::<Value>(VI_JSON) {
            flatten_json("", &v, &mut vi_map);
        }

        Self {
            current_lang: RwLock::new(Language::EnUs),
            translations_en: en_map,
            translations_vi: vi_map,
        }
    }

    pub fn set_language(&self, lang: Language) {
        if let Ok(mut lock) = self.current_lang.write() {
            *lock = lang;
        }
    }

    pub fn language(&self) -> Language {
        self.current_lang.read().map(|l| *l).unwrap_or(Language::EnUs)
    }

    pub fn t(&self, key: &str) -> String {
        let lang = self.language();
        match lang {
            Language::ViVn => {
                if let Some(val) = self.translations_vi.get(key) {
                    return val.clone();
                }
            }
            Language::EnUs => {
                if let Some(val) = self.translations_en.get(key) {
                    return val.clone();
                }
            }
        }

        // Fallback to English
        if let Some(val) = self.translations_en.get(key) {
            return val.clone();
        }

        // Fallback to key itself
        key.to_string()
    }

    pub fn t_format(&self, key: &str, replacements: &[(&str, &str)]) -> String {
        let mut result = self.t(key);
        for &(param, val) in replacements {
            let placeholder = format!("{{{}}}", param);
            result = result.replace(&placeholder, val);
        }
        result
    }
}

fn flatten_json(prefix: &str, value: &Value, out: &mut HashMap<String, String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let new_key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", prefix, k)
                };
                flatten_json(&new_key, v, out);
            }
        }
        Value::String(s) => {
            out.insert(prefix.to_string(), s.clone());
        }
        _ => {}
    }
}

// Global thread-safe instance
lazy_static_or_arc!();

// Implementation of global accessor
static GLOBAL_I18N: std::sync::OnceLock<Arc<I18nManager>> = std::sync::OnceLock::new();

macro_rules! lazy_static_or_arc {
    () => {
        pub fn get_i18n() -> Arc<I18nManager> {
            GLOBAL_I18N.get_or_init(|| Arc::new(I18nManager::new())).clone()
        }

        pub fn t(key: &str) -> String {
            get_i18n().t(key)
        }

        pub fn t_format(key: &str, replacements: &[(&str, &str)]) -> String {
            get_i18n().t_format(key, replacements)
        }
    };
}
use lazy_static_or_arc;
