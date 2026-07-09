use std::{fs, path::Path};

fn repository_file(path: impl AsRef<Path>) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
        .expect("repository contract file should be readable")
}

#[test]
fn docker_context_excludes_private_state() {
    let dockerfile = repository_file("Dockerfile");
    assert!(
        !dockerfile.lines().any(|line| line.trim() == "COPY . ."),
        "Dockerfile must copy only the files needed to build"
    );

    let dockerignore = repository_file(".dockerignore");
    let exclusions: Vec<_> = dockerignore
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();

    for required in [
        ".brigade/",
        ".codex/",
        "memory/",
        ".mcp.json",
        ".env",
        ".env.*",
        "*.pem",
        "*.key",
    ] {
        assert!(
            exclusions.contains(&required),
            ".dockerignore must exclude {required}"
        );
    }
}

#[test]
fn supported_toolchain_and_agent_workflow_are_documented() {
    let ci = repository_file(".github/workflows/ci.yml");
    assert!(ci.contains("toolchain: \"1.85\""), "CI must pin Rust 1.85");
    assert!(
        ci.contains("cargo check --locked --all-features"),
        "the Rust 1.85 CI job must check the locked all-features build"
    );

    let readme = repository_file("README.md");
    assert!(
        readme.contains("Rust 1.85 or newer"),
        "README must state the supported Rust version"
    );
    assert!(
        readme.contains("`refresh: true` is a default-off graph-index write"),
        "README must name the opt-in MCP write boundary"
    );
    assert!(
        readme.contains("before the query opens the graph read-only"),
        "README must explain when the opt-in refresh write occurs"
    );

    let agents = repository_file("AGENTS.md");
    for required in [
        "brigade work brief --target .",
        "brigade work verify run --target . --command",
        "brigade outcome capture",
        ".claude/memory-handoffs/",
    ] {
        assert!(
            agents.contains(required),
            "AGENTS.md must document the Brigade workflow step: {required}"
        );
    }
}
