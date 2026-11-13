//! Pins `tests/fixtures/vectors.json`, which `packages/bid` (TypeScript) also checks.
//! Regenerate with `UPDATE_FIXTURES=1 cargo test -p sealforge-core --test vectors`
//! (or print with `cargo run -p sealforge-core --example vectors`) and review the diff.

#[path = "support/vectors.rs"]
mod support;

#[test]
fn vectors_match_the_checked_in_file() {
    let path = support::fixtures_dir().join("vectors.json");
    let computed = support::vectors();
    if std::env::var_os("UPDATE_FIXTURES").is_some() || !path.exists() {
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&computed).unwrap() + "\n",
        )
        .unwrap();
    }
    let pinned: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(computed, pinned, "vectors changed: regenerate and review");
}
