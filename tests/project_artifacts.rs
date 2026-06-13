use std::fs;

#[test]
fn taxonomy_documents_all_supported_profiles() {
    let taxonomy =
        fs::read_to_string("cases/taxonomy.md").expect("case taxonomy document must be readable");

    for profile in [
        "smoke", "core", "extended", "legacy", "stress", "all", "negative", "fuzz",
    ] {
        assert!(
            taxonomy.contains(&format!("`{profile}`")),
            "taxonomy must document the {profile} profile"
        );
    }
}

#[test]
fn taxonomy_documents_canonical_case_id_order() {
    let taxonomy =
        fs::read_to_string("cases/taxonomy.md").expect("case taxonomy document must be readable");

    assert!(
        taxonomy.contains("<domain>/<iod_family>/<descriptor>"),
        "taxonomy must document the normalized case ID shape"
    );
    assert!(
        taxonomy.contains("classic/ct/mono2_i16_rescale_12bit_explicit_le"),
        "taxonomy must include a canonical classic CT example"
    );
    assert!(
        taxonomy.contains("Do not invert segments"),
        "taxonomy must reject non-canonical segment ordering"
    );
}
