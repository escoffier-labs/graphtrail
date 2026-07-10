use std::{fs, path::PathBuf};

use serde_json::json;

#[test]
fn brigade_station_manifest_is_portable_and_default_feature_safe() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("station.json");
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();

    assert_eq!(
        manifest,
        json!({
            "schema": "brigade.station.v1",
            "name": "graphtrail",
            "station": "search",
            "summary": "local code graph context, impact, and freshness checks",
            "lifecycle": "active",
            "tools": [{
                "name": "graphtrail",
                "command": "graphtrail",
                "summary": "local code graph CLI and context pack renderer",
                "install": ["cargo", "install", "graphtrail"],
                "surfaces": [
                    {
                        "kind": "brief-markdown",
                        "command": ["graphtrail", "context", "<task>", "--markdown"],
                        "timeout_seconds": 10,
                        "max_chars": 4000,
                        "probe": ["graphtrail", "context", "--help"],
                        "probe_contains": ["--markdown"]
                    },
                    {
                        "kind": "verify-exit",
                        "command": ["graphtrail", "--version"],
                        "timeout_seconds": 10
                    },
                    {
                        "kind": "doctor-json",
                        "command": ["graphtrail", "doctor", "--json"],
                        "timeout_seconds": 30,
                        "probe": ["graphtrail", "doctor", "--help"],
                        "probe_contains": ["--json"]
                    }
                ]
            }]
        })
    );

    let text =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("station.json")).unwrap();
    assert!(!text.contains("--blend-code-search"));
    assert!(!text.contains("--evidence"));
    assert!(!text.contains("--path"));
}
