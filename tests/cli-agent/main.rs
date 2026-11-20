//! CP1: process/schema/zero-source proof. All inputs and identifiers are synthetic.
use serde_json::Value;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{
    OnceLock,
    atomic::{AtomicU64, Ordering},
};

#[allow(dead_code)]
#[path = "../support/dirs.rs"]
mod dirs;
#[allow(dead_code)]
#[path = "../support/schema.rs"]
mod schema;
// Placing fixture programs without holding their writer (#811).
#[allow(dead_code)]
#[path = "../support/place.rs"]
mod place;

mod fake_loops;
mod github_token;
mod init;
mod loop_apply;
mod loop_export;
mod loop_plan;
mod loop_validate;
mod models;
// #1804 (MD2): `openwiz models show`, `list` and `set`.
mod models_forms;
// #1803 (MD1): the models contract's fixture checks, kept beside the fixtures.
#[path = "../../contracts/fixtures/models/models_contract.rs"]
mod models_contract;
mod process;
mod results;
mod review;
mod ui;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");

fn fixture_cli() -> &'static Path {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT.get_or_init(|| {
        let target = Path::new(env!("CARGO_TARGET_TMPDIR")).parent().unwrap();
        let result = Command::new(env!("CARGO"))
            .args([
                "build",
                "--locked",
                "-p",
                "openwiz-tests",
                "--example",
                "openwiz",
                "--target-dir",
            ])
            .arg(target)
            .current_dir(ROOT)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        target.join("debug/examples/openwiz")
    })
}

struct Sandbox(PathBuf);
impl Sandbox {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "openwiz-cp1-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        dirs::private_dir(&root).unwrap();
        dirs::private_dir(root.join("bin")).unwrap();
        // Both possible source programs leave evidence if discovery calls them.
        let program = root.join("bin/room-cli");
        place::place_script(
            &program,
            "#!/bin/sh\nprintf 'unexpected source call\\n' >> \"$CP1_CALLS\"\nexit 99\n",
        );
        std::os::unix::fs::symlink(&program, root.join("bin/gh")).unwrap();
        Self(root)
    }

    fn run(&self, args: &[&str]) -> Output {
        self.run_os(&args.iter().map(OsString::from).collect::<Vec<_>>())
    }

    fn run_os(&self, args: &[OsString]) -> Output {
        Command::new(fixture_cli())
            .args(args)
            .env_clear()
            .env("PATH", self.0.join("bin"))
            .env("HOME", self.0.join("home"))
            .env("OPENWIZ_STATE_DIR", self.0.join("state"))
            .env("OPENWIZ_FIXTURE_PROGRAM", self.0.join("bin/room-cli"))
            .env("CP1_CALLS", self.0.join("calls"))
            .current_dir(&self.0)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
}

fn discovered() -> Value {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&["help", "--agent"]);
    assert_eq!(output.status.code(), Some(0));
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn cp1_catalogue_covers_the_public_command_flag_and_schema_matrix() {
    let catalogue = discovered();
    // Literal expectations from the existing command parsers/spec, not from the
    // catalogue builder. Forms that forbid write flags have their own rows.
    let matrix = [
        ("help", "--json"),
        ("help.agent", "--agent"),
        ("ls", "-l -a --hidden --limit --offset --json --help"),
        ("show", "--json --help"),
        ("models", "--json --help"),
        ("models.show", "--json --help"),
        ("models.list", "--save --json --help"),
        ("models.set", "--json --help"),
        ("fleets", "--json --help"),
        ("fleet", "--json"),
        ("fleet.tick.preview", "--dry-run --json"),
        ("fleet.due", "--github-only --json"),
        ("loop.off", "--json"),
        ("loop.on", "--json"),
        ("loop.validate", "--catalog --json --help"),
        ("loop.export", "--project --out --json --help"),
        ("loop.plan", "--project --adopt --json --help"),
        ("loop.apply", "--project --from --adopt --json --help"),
        ("github.token", "--repository --read --help"),
        ("init", "--repository --state-dir --json"),
        (
            "init.apply",
            "--repository --state-dir --json --yes --operation",
        ),
    ];
    let commands = catalogue["commands"].as_array().unwrap();
    assert_eq!(commands.len(), matrix.len());
    let flags = catalogue["flags"].as_array().unwrap();
    let names: std::collections::BTreeSet<_> =
        flags.iter().map(|f| f["name"].as_str().unwrap()).collect();
    assert_eq!(names.len(), flags.len(), "unique flag definitions");
    for (command, (name, expected)) in commands.iter().zip(matrix) {
        assert_eq!(command["name"], name);
        let actual: Vec<_> = command["flags"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f.as_str().unwrap())
            .collect();
        assert_eq!(
            actual,
            expected.split_whitespace().collect::<Vec<_>>(),
            "{name}"
        );
        for flag in &actual {
            assert!(names.contains(flag), "undefined flag {name}: {flag}");
        }
        for flag in command["required_flags"].as_array().unwrap() {
            assert!(
                actual.contains(&flag.as_str().unwrap()),
                "required flag is available"
            );
        }
        assert!(!command["exits"].as_array().unwrap().is_empty());
        assert!(!command["examples"].as_array().unwrap().is_empty());
        assert!(!command["effects"].as_array().unwrap().is_empty());
        for result in command["results"].as_array().unwrap() {
            let (file, fragment) = result["reference"]
                .as_str()
                .unwrap()
                .split_once('#')
                .unwrap();
            let schema: Value =
                serde_json::from_str(&fs::read_to_string(Path::new(ROOT).join(file)).unwrap())
                    .unwrap();
            let target = schema
                .pointer(fragment)
                .unwrap_or_else(|| panic!("unresolved {}", result["reference"]));
            if result["schema"] == "local loop switch event" {
                assert!(
                    target["properties"].get("schema").is_none(),
                    "unchanged event has no schema field"
                );
            } else {
                assert_eq!(
                    target["properties"]["schema"]["const"], result["schema"],
                    "{name} refers to its actual report definition"
                );
            }
        }
        // Parse documented examples at the public parser seam, never dispatch
        // a write. Loop/models/fleets lack a public parser; checked below/help.
        for example in command["examples"].as_array().unwrap() {
            let args: Vec<String> = example
                .as_array()
                .unwrap()
                .iter()
                .map(|a| a.as_str().unwrap().to_owned())
                .collect();
            let os: Vec<_> = args[1..].iter().map(std::ffi::OsString::from).collect();
            match args[0].as_str() {
                "help" | "ls" | "show" => {
                    openwiz_cli::parse(&args).unwrap();
                }
                "fleet" if matches!(args[1].as_str(), "tick" | "due") => {
                    openwiz_cli::fleet::tick::parse(&os).unwrap();
                }
                "fleet" => {
                    openwiz_cli::fleet::parse(&os).unwrap();
                }
                "loop" | "models" | "fleets" => {}
                // The packaged Python parser is exercised by the existing
                // package-proof stage after CI provisions Python.
                "init" => {}
                "github" => drop(openwiz_cli::github_token::parse(&args[1..]).unwrap()),
                other => panic!("unexpected shipped command {other}"),
            }
        }
    }
    let flag = |name| flags.iter().find(|f| f["name"] == name).unwrap();
    assert_eq!(flag("--limit")["default"], "100");
    assert_eq!(flag("--offset")["default"], "0");
    assert_eq!(flag("--help")["aliases"], serde_json::json!(["-h"]));
}

#[test]
fn cp1_help_aliases_and_invalid_agent_forms_have_no_effects() {
    let sandbox = Sandbox::new();
    for args in [
        vec!["help"],
        vec!["--help"],
        vec!["-h"],
        vec!["ls", "--help"],
        vec!["show", "-h"],
        vec!["models", "--help"],
        vec!["fleets", "-h"],
        vec!["loop", "--help"],
        vec!["loop", "-h"],
    ] {
        let result = sandbox.run(&args);
        assert_eq!(result.status.code(), Some(0), "{args:?}");
        assert!(result.stdout.starts_with(b"openwiz") || result.stdout.starts_with(b"  openwiz"));
        assert!(result.stderr.is_empty());
    }
    for args in [
        vec!["help", "--agent", "extra"],
        vec!["help", "--agent=true"],
        vec!["help", "--unknown"],
        vec!["ui"],
        vec!["fleet", "tick", "--unknown"],
    ] {
        let result = sandbox.run(&args);
        assert_eq!(result.status.code(), Some(2), "{args:?}");
        assert!(result.stdout.is_empty());
        assert!(result.stderr.starts_with(b"openwiz:"));
    }
    assert!(!sandbox.0.join("calls").exists());
    assert!(!sandbox.0.join("state").exists());
    assert!(!sandbox.0.join("home").exists());
}

#[test]
fn cp1_schema_refuses_broken_catalogues_and_pins_additive_result_shapes() {
    let contract: Value = serde_json::from_str(
        &fs::read_to_string(Path::new(ROOT).join("contracts/cli.v1.schema.json")).unwrap(),
    )
    .unwrap();
    let catalogue = discovered();
    for field in ["flags", "commands", "schema", "kind"] {
        let mut bad = catalogue.clone();
        bad.as_object_mut().unwrap().remove(field);
        assert!(
            !schema::validate(&contract, &bad).is_empty(),
            "missing {field}"
        );
    }
    let mut bad = catalogue.clone();
    bad["commands"][0]["effects"] = serde_json::json!(["arbitrary_process"]);
    assert!(!schema::validate(&contract, &bad).is_empty());
    let mut bad = catalogue.clone();
    bad["commands"][0]["exits"][0]["code"] = serde_json::json!(4);
    assert!(!schema::validate(&contract, &bad).is_empty());
    let documents = [
        serde_json::json!({"schema":"openwiz.cli-error.v1","command":"ls","exit_code":2,"error":{"kind":"usage","message":"unknown option"}}),
        serde_json::json!({"schema":"openwiz.cli-help.v1","text":"synthetic help"}),
        serde_json::json!({"schema":"openwiz.fleet-due.v1","candidates":[{"spec":123,"ticket":124},{"spec":125,"ticket":null}]}),
        serde_json::json!({"loop_id":"loop-synthetic","definition_revision":"1111111111111111111111111111111111111111","repository":"synthetic/repo","project":"project-synthetic","enabled":false,"actor":"synthetic-operator","at_ms":1000}),
    ];
    for document in documents {
        assert_eq!(schema::validate(&contract, &document), Vec::<String>::new());
        let mut extra = document.clone();
        extra["unexpected"] = serde_json::json!(true);
        assert!(!schema::validate(&contract, &extra).is_empty());
        for field in document.as_object().unwrap().keys() {
            let mut missing = document.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(
                !schema::validate(&contract, &missing).is_empty(),
                "missing {field}"
            );
        }
    }
}

#[test]
fn cp1_advertised_flags_are_accepted_by_the_shipped_public_parsers() {
    let catalogue = discovered();
    for command in catalogue["commands"].as_array().unwrap() {
        let name = command["name"].as_str().unwrap();
        let base = match name {
            "help" => "help",
            "help.agent" => "help",
            "ls" => "ls",
            "show" => "show thread-synthetic",
            "fleet" => "fleet 123",
            "fleet.tick.preview" => "fleet tick --dry-run",
            "fleet.due" => "fleet due --github-only",
            // These three parsers are private; their flags are checked through
            // real processes below, using only trap sources or absent config.
            "fleets" | "models" | "models.show" | "models.list" | "models.set" | "loop.off"
            | "loop.on" | "loop.validate" | "loop.export" | "loop.plan" | "loop.apply" => continue,
            "init" | "init.apply" => continue,
            "github.token" => {
                for flag in command["flags"].as_array().unwrap() {
                    let mut args = vec!["token", "--repository", "synthetic-owner/work"];
                    args.extend(Some(flag.as_str().unwrap()).filter(|f| *f != "--repository"));
                    let args: Vec<String> = args.into_iter().map(str::to_owned).collect();
                    assert!(
                        openwiz_cli::github_token::parse(&args).is_ok(),
                        "advertised flag rejected: {args:?}"
                    );
                }
                continue;
            }
            other => panic!("unmapped form {other}"),
        };
        for flag in command["flags"].as_array().unwrap() {
            let flag = flag.as_str().unwrap();
            let mut args: Vec<String> = base.split_whitespace().map(str::to_owned).collect();
            if !args.iter().any(|a| a == flag) {
                args.push(flag.to_owned());
                let value = match flag {
                    "--limit" => Some("10"),
                    "--offset" => Some("2"),
                    _ => None,
                };
                if let Some(value) = value {
                    args.push(value.to_owned());
                }
            }
            let os: Vec<_> = args[1..].iter().map(std::ffi::OsString::from).collect();
            let accepted = match args[0].as_str() {
                "help" | "ls" | "show" => openwiz_cli::parse(&args).is_ok(),
                "fleet" if matches!(args[1].as_str(), "tick" | "due") => {
                    openwiz_cli::fleet::tick::parse(&os).is_ok()
                }
                "fleet" => openwiz_cli::fleet::parse(&os).is_ok(),
                _ => unreachable!(),
            };
            assert!(accepted, "advertised flag rejected: {args:?}");
        }
    }
    for (args, code) in [
        (vec!["fleets", "--json"], 0),
        (vec!["models", "--json"], 13),
        (vec!["fleet", "tick", "--dry-run", "--json"], 1),
        (vec!["fleet", "due", "--github-only"], 1),
        (vec!["loop", "off", "loop-synthetic", "--json"], 1),
        (vec!["loop", "on", "loop-synthetic", "--json"], 1),
        (vec!["loop", "--json", "off", "loop-synthetic"], 2),
    ] {
        let sandbox = Sandbox::new();
        let output = sandbox.run(&args);
        assert_eq!(
            output.status.code(),
            Some(code),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!sandbox.0.join("state").exists());
    }
}

#[test]
fn cp1_fleet_forms_separate_execution_from_read_only_candidates_and_plans() {
    let catalogue = discovered();
    let commands = catalogue["commands"].as_array().unwrap();
    let form = |name| commands.iter().find(|c| c["name"] == name).unwrap();
    // The executing tick was retired (#800): only its read-only plan remains.
    assert!(commands.iter().all(|c| c["name"] != "fleet.tick"));
    assert_eq!(
        form("fleet.tick.preview")["effects"],
        serde_json::json!(["source_read", "state_read"])
    );
    assert_eq!(
        form("fleet.tick.preview")["required_flags"],
        serde_json::json!(["--dry-run"])
    );
    assert_eq!(
        form("fleet.due")["effects"],
        serde_json::json!(["source_read"])
    );
    assert_eq!(form("fleet.due")["sources"], serde_json::json!(["gh"]));
    assert_eq!(
        form("fleet.due")["required_flags"],
        serde_json::json!(["--github-only"])
    );
    assert_eq!(form("fleet.due")["json"], true);
    // Inspect the real public parser without executing any fleet action.
    for (args, due, dry_run, json) in [
        (vec!["tick", "--dry-run", "--json"], false, true, true),
        (vec!["due", "--github-only"], true, false, false),
        (vec!["due", "--github-only", "--json"], true, false, true),
    ] {
        let os: Vec<_> = args.into_iter().map(std::ffi::OsString::from).collect();
        let parsed = openwiz_cli::fleet::tick::parse(&os).unwrap();
        assert_eq!(
            (parsed.due, parsed.dry_run, parsed.json),
            (due, dry_run, json)
        );
    }
    for args in [
        vec!["due"],
        vec!["tick"],
        vec!["tick", "--json"],
        vec![
            "tick",
            "--expect-plan",
            "synthetic-reviewed-plan.json",
            "--json",
        ],
        vec!["tick", "--json", "--json"],
        vec!["tick", "--dry-run=true"],
        vec!["tick", "--"],
    ] {
        let os: Vec<_> = args.into_iter().map(std::ffi::OsString::from).collect();
        assert!(openwiz_cli::fleet::tick::parse(&os).is_err());
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn cp1_catalogue_is_discoverable_without_sources_or_state() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&["help", "--agent"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "help --agent must execute: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let catalogue: Value = serde_json::from_slice(&output.stdout).expect("one catalogue document");
    // Compact means a single JSON line, independent of terminal width.
    assert_eq!(output.stdout.iter().filter(|&&b| b == b'\n').count(), 1);
    let contract: Value = serde_json::from_str(
        &fs::read_to_string(Path::new(ROOT).join("contracts/cli.v1.schema.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(schema::lint(&contract), Vec::<String>::new());
    assert_eq!(
        schema::validate(&contract, &catalogue),
        Vec::<String>::new()
    );
    assert_eq!(catalogue["schema"], "openwiz.cli.v1");
    assert_eq!(catalogue["kind"], "catalogue");
    let commands = catalogue["commands"].as_array().unwrap();
    let actual: Vec<_> = commands
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        actual,
        [
            "help",
            "help.agent",
            "ls",
            "show",
            "models",
            "models.show",
            "models.list",
            "models.set",
            "fleets",
            "fleet",
            "fleet.tick.preview",
            "fleet.due",
            "loop.off",
            "loop.on",
            "loop.validate",
            "loop.export",
            "loop.plan",
            "loop.apply",
            "github.token",
            "init",
            "init.apply"
        ]
    );
    assert!(
        !sandbox.0.join("calls").exists(),
        "exactly zero source calls"
    );
    assert!(
        !sandbox.0.join("state").exists(),
        "no state directory created"
    );
    assert!(
        !sandbox.0.join("home").exists(),
        "no fallback state created"
    );

    // Discovery must also leave an existing store byte-for-byte unchanged.
    dirs::private_dir(sandbox.0.join("state")).unwrap();
    fs::write(sandbox.0.join("state/sentinel"), b"synthetic state\n").unwrap();
    let repeated = sandbox.run(&["help", "--agent"]);
    assert_eq!(repeated.status.code(), Some(0));
    assert_eq!(repeated.stdout, output.stdout);
    assert!(repeated.stderr.is_empty());
    assert_eq!(
        fs::read(sandbox.0.join("state/sentinel")).unwrap(),
        b"synthetic state\n"
    );
    assert_eq!(fs::read_dir(sandbox.0.join("state")).unwrap().count(), 1);
    assert!(!sandbox.0.join("calls").exists());
    assert!(!sandbox.0.join("home").exists());
}

// Reuse the existing Node recording fixture, with this target's shared dispatcher.
mod support {
    pub fn openwiz_release() -> std::path::PathBuf {
        super::fixture_cli().to_owned()
    }
}
