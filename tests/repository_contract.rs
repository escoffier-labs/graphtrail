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
