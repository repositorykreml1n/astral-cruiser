#[path = "../mixins/astral_cruiser/lang.rs"]
mod astral_lang;

use astral_lang::{resolve, tr, Lang, LangSetting, STRINGS};
use std::collections::HashSet;

#[test]
fn generated_waypoint_name_uses_the_active_language() {
    assert_eq!(tr(Lang::Ru, "waypoint.default_name"), "Точка {}");
    assert_eq!(tr(Lang::En, "waypoint.default_name"), "Waypoint {}");
}

#[test]
fn auto_detects_ru_and_en() {
    assert_eq!(resolve(LangSetting::Auto, "ru_ru"), Lang::Ru);
    assert_eq!(resolve(LangSetting::Auto, "ru_RU"), Lang::Ru);
    assert_eq!(resolve(LangSetting::Auto, "ru"), Lang::Ru);
    assert_eq!(resolve(LangSetting::Auto, "en_us"), Lang::En);
    assert_eq!(resolve(LangSetting::Auto, "en_GB"), Lang::En);
    assert_eq!(resolve(LangSetting::Auto, "de_de"), Lang::En);
    assert_eq!(resolve(LangSetting::Auto, "fr_fr"), Lang::En);
    assert_eq!(resolve(LangSetting::Auto, ""), Lang::En);
}

#[test]
fn explicit_overrides_auto() {
    assert_eq!(resolve(LangSetting::Ru, "en_us"), Lang::Ru);
    assert_eq!(resolve(LangSetting::Ru, "de_de"), Lang::Ru);
    assert_eq!(resolve(LangSetting::En, "ru_ru"), Lang::En);
    assert_eq!(resolve(LangSetting::En, "ru"), Lang::En);
}

#[test]
fn every_key_has_non_empty_ru_and_en() {
    for &(key, ru, en) in STRINGS {
        assert!(!key.trim().is_empty(), "key must not be empty");
        assert!(!ru.trim().is_empty(), "ru translation for '{key}' must not be empty");
        assert!(!en.trim().is_empty(), "en translation for '{key}' must not be empty");
    }
}

#[test]
fn no_duplicate_keys() {
    let mut seen = HashSet::new();
    for &(key, _, _) in STRINGS {
        assert!(seen.insert(key), "duplicate translation key: '{key}'");
    }
}

#[test]
fn unknown_key_returns_itself() {
    assert_eq!(tr(Lang::Ru, "nonexistent.key.123"), "nonexistent.key.123");
    assert_eq!(tr(Lang::En, "nonexistent.key.123"), "nonexistent.key.123");
}

#[test]
fn lang_setting_round_trip_str() {
    for setting in [LangSetting::Auto, LangSetting::Ru, LangSetting::En] {
        let s = setting.as_str();
        assert_eq!(LangSetting::from_str(s), setting);
    }
    assert_eq!(LangSetting::from_str("invalid"), LangSetting::Auto);
}
