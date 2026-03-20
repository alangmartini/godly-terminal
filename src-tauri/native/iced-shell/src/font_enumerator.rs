use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Enumerate monospace font families installed on the system.
///
/// Results are cached after the first call. "Geist Mono" is always pinned
/// first in the returned list.
pub fn enumerate_monospace_fonts() -> Vec<String> {
    static CACHED: OnceLock<Vec<String>> = OnceLock::new();
    CACHED
        .get_or_init(|| {
            let mut db = fontdb::Database::new();
            db.load_system_fonts();

            let mut families = std::collections::BTreeSet::new();
            for face in db.faces() {
                if face.monospaced {
                    families.insert(face.families[0].0.clone());
                }
            }

            let mut list: Vec<String> = families.into_iter().collect();
            // Pin "Geist Mono" first if present.
            if let Some(pos) = list.iter().position(|n| n == "Geist Mono") {
                list.remove(pos);
            }
            list.insert(0, "Geist Mono".to_string());
            list
        })
        .clone()
}

/// Intern a font name as `&'static str`.
///
/// Iced's `font::Family::Name` requires a `&'static str`. We leak each
/// unique name exactly once via a global interning table. The total leaked
/// memory is negligible (~20-50 bytes per font family).
pub fn intern_font_name(name: &str) -> &'static str {
    static INTERNED: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    let map = INTERNED.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = map.lock().unwrap();
    if let Some(&s) = map.get(name) {
        return s;
    }
    let leaked: &'static str = Box::leak(name.to_string().into_boxed_str());
    map.insert(name.to_string(), leaked);
    leaked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerate_returns_geist_mono_first() {
        let fonts = enumerate_monospace_fonts();
        assert!(!fonts.is_empty(), "should find at least one font");
        assert_eq!(fonts[0], "Geist Mono", "Geist Mono should be pinned first");
    }

    #[test]
    fn intern_returns_same_pointer() {
        let a = intern_font_name("TestFont");
        let b = intern_font_name("TestFont");
        assert!(std::ptr::eq(a, b), "interned names should be same pointer");
    }
}
