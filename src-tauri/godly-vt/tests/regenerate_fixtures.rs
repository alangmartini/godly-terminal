mod helpers;

/// Regenerate all fixture JSON files from current godly-vt behavior.
/// Run with: cargo test -p godly-vt --test regenerate_fixtures -- --ignored --nocapture
#[test]
#[ignore]
fn regenerate_all_fixtures() {
    let fixtures = [
        "absolute_movement",
        "alternate_buffer",
        "deckpam",
        "decsc",
        "decstbm",
        "ed",
        "el",
        "ich_dch_ech",
        "il_dl",
        "modes",
        "origin_mode",
        "relative_movement",
        "ri",
        "ris",
        "scroll",
        "split_escape_sequences",
        "split_utf8",
        "unknown_osc",
    ];

    for name in &fixtures {
        eprintln!("regenerating {name}...");
        helpers::regenerate_fixture(name);
    }
    eprintln!("done");
}
