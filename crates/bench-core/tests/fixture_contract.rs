use cakify_bench_protocol::FixtureManifest;

#[test]
fn committed_manifest_matches_protocol_default() {
    let committed: FixtureManifest = serde_json::from_str(include_str!(
        "../../../bench/fixtures/manifest.json"
    ))
    .expect("fixture manifest must be valid protocol JSON");

    assert_eq!(committed, FixtureManifest::default());
}
