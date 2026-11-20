//! The approved dependency set, from `cargo metadata`. Dependency direction
//! itself (core, adapter, CLI) needs no test: each crate uses the one below
//! it, so a dependency pointing back is a cycle Cargo refuses to build (#811).

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

const APPROVED_DIRECT: &[&str] = &["serde", "serde_json", "rustix"];
fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/.."))
}

/// This machine's target triple, from `rustc -vV`.
fn host() -> String {
    let out = Command::new("rustc")
        .arg("-vV")
        .output()
        .expect("run rustc -vV");
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .find_map(|l| l.strip_prefix("host: ").map(str::to_owned))
        .expect("rustc reports its host")
}

/// The resolved graph for this platform: offline, a build only downloads
/// this platform's crates (the lock record below still covers them all).
fn metadata() -> Value {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--locked", "--offline"])
        .arg("--filter-platform")
        .arg(host())
        .current_dir(root())
        .output()
        .expect("run cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("metadata is JSON")
}

#[test]
fn serde_and_serde_json_are_the_only_direct_dependencies() {
    let meta = metadata();
    let members: BTreeSet<&str> = meta["workspace_members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m.as_str().unwrap())
        .collect();
    for package in meta["packages"].as_array().unwrap() {
        if !members.contains(package["id"].as_str().unwrap()) {
            continue;
        }
        for dep in package["dependencies"].as_array().unwrap() {
            let name = dep["name"].as_str().unwrap();
            let is_workspace_crate = dep["path"].is_string();
            assert!(
                is_workspace_crate || APPROVED_DIRECT.contains(&name),
                "{} depends on {name}, which is not approved",
                package["name"]
            );
        }
    }
}

#[test]
fn dependencies_md_records_every_locked_crate_with_its_checksum() {
    let lock = fs::read_to_string(root().join("Cargo.lock")).expect("Cargo.lock");
    let record =
        fs::read_to_string(root().join("tests/checks/dependencies.md")).expect("dependencies.md");
    let mut locked = 0;
    for block in lock.split("[[package]]").skip(1) {
        let field = |key: &str| {
            block
                .lines()
                .find_map(|l| l.strip_prefix(&format!("{key} = \"")))
                .map(|v| v.trim_end_matches('"').to_owned())
        };
        let Some(checksum) = field("checksum") else {
            continue; // a workspace crate
        };
        let (name, version) = (field("name").unwrap(), field("version").unwrap());
        // One row per locked version: a crate can be locked twice.
        let row = record
            .lines()
            .find(|l| l.starts_with(&format!("| `{name}` | `{version}` |")))
            .unwrap_or_else(|| panic!("{name} {version} is not in dependencies.md"));
        assert!(row.contains(&checksum), "{name}: checksum not recorded");
        locked += 1;
    }
    assert!(locked >= 2, "the lock file has the approved crates");
}
