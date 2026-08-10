use shadowdate::config::{Appearance, Reminders, ServiceConfig, MAX_LEAD_MIN};

/// A unique temp path for this test process so parallel `cargo test` runs
/// (separate processes sharing the global temp dir) never collide on the same
/// fixed filename or trip over stale files from a crashed run.
fn tmp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{}-{}", std::process::id(), name))
}

#[test]
fn config_defaults_on_missing_file() {
    let p = tmp("shadowdate_service_missing.toml");
    std::fs::remove_file(&p).ok();
    let cfg = ServiceConfig::load(&p);
    assert_eq!(cfg.reminders.lead_min, 10);
    assert_eq!(cfg.reminders.all_day_hour, 9);
    assert_eq!(cfg.reminders.all_day_minute, 0);
    assert!(cfg.appearance.show_portrait, "portrait must default to visible");
}

#[test]
fn config_defaults_on_invalid_content() {
    let p = tmp("shadowdate_service_bad.toml");
    std::fs::write(&p, "not = [valid").unwrap();
    let cfg = ServiceConfig::load(&p);
    assert_eq!(cfg.reminders.lead_min, 10);
    assert!(cfg.appearance.show_portrait);
    std::fs::remove_file(&p).ok();
}

#[test]
fn config_without_appearance_key_defaults_to_visible() {
    // A config written before the appearance section existed must not hide the
    // portrait: the missing key loads as `show_portrait = true`.
    let p = tmp("shadowdate_service_no_appearance.toml");
    std::fs::write(
        &p,
        "[reminders]\nlead_min = 5\nall_day_hour = 9\nall_day_minute = 0\n",
    )
    .unwrap();
    let cfg = ServiceConfig::load(&p);
    assert_eq!(cfg.reminders.lead_min, 5);
    assert!(cfg.appearance.show_portrait);
    std::fs::remove_file(&p).ok();
}

#[test]
fn config_clamps_out_of_range_values() {
    // A hand-edited config with absurd hours/minutes must not panic the daemon
    // when scheduling; values are clamped to valid ranges instead.
    let p = tmp("shadowdate_service_clamp.toml");
    std::fs::write(
        &p,
        "[reminders]\nlead_min = 5000\nall_day_hour = 99\nall_day_minute = 99\n",
    )
    .unwrap();
    let cfg = ServiceConfig::load(&p);
    assert_eq!(
        cfg.reminders.lead_min,
        MAX_LEAD_MIN,
        "lead must clamp to the shared MAX_LEAD_MIN contract"
    );
    assert_eq!(cfg.reminders.all_day_hour, 23);
    assert_eq!(cfg.reminders.all_day_minute, 59);
    std::fs::remove_file(&p).ok();
}

#[test]
fn config_save_load_roundtrip() {
    let p = tmp("shadowdate_service_rt.toml");
    let cfg = ServiceConfig {
        reminders: Reminders {
            lead_min: 30,
            all_day_hour: 8,
            all_day_minute: 45,
        },
        appearance: Appearance {
            show_portrait: false,
        },
    };
    cfg.save(&p).unwrap();
    let back = ServiceConfig::load(&p);
    assert_eq!(back.reminders.lead_min, 30);
    assert_eq!(back.reminders.all_day_hour, 8);
    assert_eq!(back.reminders.all_day_minute, 45);
    assert!(!back.appearance.show_portrait);
    std::fs::remove_file(&p).ok();
}
