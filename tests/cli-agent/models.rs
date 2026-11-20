//! The kept `openwiz models` (J233, #1440): its read-only output contract,
//! run as a process against the fake `room-cli`. Restored from the cases
//! #1264 removed (N18's `models` cases, N15's no-write row and CP2's
//! `models` half) with their assertions unchanged; #1517 then tightened the
//! one-read and failed-read cases. All inputs are synthetic.
use super::{ROOT, Sandbox, fixture_cli, schema};
use openwiz_adapter_cloudroom::source::CloudRoomSource;
use openwiz_core::model::Timestamp;
use openwiz_core::ops::Clock;
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    time::Instant,
};

/// The synthetic catalogue's read time, 2023-11-14T22:23:20Z, so the text's
/// read line is fixed; real monotonic time, for the read deadline.
pub(super) struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        Timestamp(1_700_000_600_000)
    }
    fn monotonic(&self) -> Instant {
        Instant::now()
    }
}

/// One synthetic `cloud status --json` answer, by fixture name.
fn status_file(name: &str) -> String {
    Path::new(ROOT)
        .join("tests/fixtures/room-cli-v127/new-thread")
        .join(name)
        .to_str()
        .unwrap()
        .to_owned()
}

pub(super) struct Models(pub(super) Sandbox);
impl Models {
    /// A sandbox whose `room-cli` is the fake, answering `cloud status
    /// --json` with `answer` and accepting any spawn, so a stray write is
    /// counted rather than refused.
    pub(super) fn new(answer: Value) -> Self {
        let f = Self(Sandbox::new());
        let bin = f.0.0.join("bin");
        fs::remove_file(bin.join("room-cli")).unwrap();
        std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_fake-room-cli"), bin.join("room-cli"))
            .unwrap();
        let mut status = json!({"argv": ["cloud", "status", "--json"]});
        status
            .as_object_mut()
            .unwrap()
            .extend(answer.as_object().unwrap().clone());
        let scenario = f.0.0.join("scenario.json");
        fs::write(
            &scenario,
            json!({
                "extends": Path::new(ROOT).join("tests/fixtures/room-cli-0.43.1/normal").to_str().unwrap(),
                "responses": [
                    {"argv": ["--version"], "stdout": "v127\n"},
                    status,
                    {"any_spawn": true, "creates": true, "stdout_file": status_file("spawn-receipt-created.json")},
                ],
            })
            .to_string(),
        )
        .unwrap();
        fs::write(
            bin.join("fake-room-cli.scenario"),
            scenario.to_str().unwrap(),
        )
        .unwrap();
        f
    }
    fn file(name: &str) -> Self {
        Self::new(json!({"stdout_file": status_file(name)}))
    }
    pub(super) fn run(&self, args: &[&str]) -> (i32, String, String) {
        let out = self.command(args).output().unwrap();
        (
            out.status.code().expect("exited"),
            String::from_utf8(out.stdout).unwrap(),
            String::from_utf8(out.stderr).unwrap(),
        )
    }
    /// The `openwiz <args>` process, started; its output discarded.
    pub(super) fn spawn(&self, args: &[&str]) -> std::process::Child {
        let null = Stdio::null;
        self.command(args)
            .stdout(null())
            .stderr(null())
            .spawn()
            .unwrap()
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(fixture_cli());
        command
            .args(args)
            .env_clear()
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.0.0.join("bin").display(),
                    std::env::var("PATH").unwrap()
                ),
            )
            .env("HOME", self.0.0.join("home"))
            .env("OPENWIZ_STATE_DIR", self.0.0.join("state"))
            .env("OPENWIZ_FIXTURE_PROGRAM", self.0.0.join("bin/room-cli"))
            .current_dir(&self.0.0)
            .stdin(Stdio::null());
        command
    }
    /// `openwiz models <args>` (the words after `models`) in this process, through the production
    /// `run_models` and the real CloudRoom source, with the fixed clock.
    fn run_fixed(&self, args: &[&str]) -> (i32, String, String) {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        let source = CloudRoomSource::new(self.0.0.join("bin/room-cli"));
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = openwiz_cli::models::run_models(&args, &source, &FixedClock, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }
    /// `models --json`: its exit code, its one JSON document, checked
    /// against `contracts/new-thread.v1.schema.json#/$defs/options`, and its
    /// stderr.
    fn json(&self) -> (i32, Value, String) {
        let (code, out, err) = self.run(&["models", "--json"]);
        assert!(out.ends_with('\n'), "{out}{err}");
        let doc: Value = serde_json::from_str(&out).expect("one complete JSON document");
        assert_eq!(doc["schema"], "openwiz.cloud-options.v1", "{doc}");
        let mut contract: Value = serde_json::from_str(
            &fs::read_to_string(Path::new(ROOT).join("contracts/new-thread.v1.schema.json"))
                .unwrap(),
        )
        .unwrap();
        contract["$ref"] = json!("#/$defs/options");
        assert_eq!(
            schema::validate(&contract, &doc),
            Vec::<String>::new(),
            "{doc}"
        );
        (code, doc, err)
    }
    pub(super) fn invocations(&self) -> Vec<Vec<String>> {
        fs::read_to_string(self.0.0.join("bin/invocations.log"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
}

const STATUS_READ: [&str; 3] = ["cloud", "status", "--json"];

// catches: the text dropping an offered harness, model or effort, inventing a
// default, offering an unreported effort, or making more than one read.
#[test]
fn models_shows_the_options_as_reported_and_no_default() {
    let fake = Models::file("cloud-status.json");
    let (code, out, err) = fake.run_fixed(&[]);
    assert_eq!(code, 0, "{err}");
    for words in [
        "as reported by CloudRoom; it may answer from a saved copy",
        "read 2023-11-14T22:23:20Z",
        "codex",
        "claude-code",
        "syn-codex-model-01: low, medium, high, xhigh, max",
        "syn-claude-model-01: low, medium, high, xhigh, max",
        "pi: unavailable (catalogue not reported)",
        "cursor: unavailable (catalogue not reported)",
    ] {
        assert!(out.contains(words), "{words:?} not shown:\n{out}");
    }
    assert!(!out.contains("ultra"), "never offered:\n{out}");
    assert!(
        !out.to_lowercase().contains("default"),
        "no default:\n{out}"
    );
    assert_eq!(fake.invocations(), [STATUS_READ], "one read");
}

// catches: a not-ready, missing or drifted catalogue being offered (exit 0) or
// reported with the wrong reason, in JSON or in the text.
#[test]
fn models_with_nothing_offered_exits_13_with_its_reason() {
    for (name, reason) in [
        ("cloud-status-not-ready.json", "not_ready"),
        ("cloud-status-harnesses-null.json", "harnesses_not_reported"),
        ("cloud-status-drift-array.json", "unexpected_shape"),
    ] {
        let fake = Models::file(name);
        let (code, json, _) = fake.json();
        assert_eq!(code, 13, "{name}: {json}");
        assert_eq!(json["state"]["reason"], reason, "{name}");
        let (code, out, _) = fake.run(&["models"]);
        assert_eq!(code, 13, "{name}");
        assert!(out.contains("no options"), "{name}: {out}");
    }
}

// catches: `models --json` leaving the cloud-options contract or exiting
// non-zero when options are offered, which setup and onboarding read.
#[test]
fn models_json_offers_options_under_its_contract_with_exit_0() {
    let fake = Models::file("cloud-status.json");
    let (code, doc, _) = fake.json();
    assert_eq!(code, 0, "{doc}");
    let offered: Vec<&Value> = doc["harnesses"]
        .as_array()
        .expect("offered harnesses")
        .iter()
        .map(|row| &row["harness"])
        .collect();
    assert_eq!(offered, [&json!("codex"), &json!("claude-code")], "{doc}");
}

// catches: an unavailable harness dropped from JSON or losing its reason.
#[test]
fn models_json_keeps_unavailable_harness_with_its_reason_and_explanation() {
    let (code, report, _) = Models::file("cloud-status.json").json();
    assert_eq!(code, 0, "{report}");
    let cursor: Vec<&Value> = report["unavailable"]
        .as_array()
        .expect("models JSON reports unavailable harnesses")
        .iter()
        .filter(|row| row["harness"] == "cursor")
        .collect();
    assert_eq!(cursor.len(), 1, "one unavailable cursor harness: {report}");
    assert_eq!(cursor[0]["reason"], "catalogue_not_reported", "{report}");
    assert_eq!(
        cursor[0]["text"], "unavailable (catalogue not reported)",
        "{report}"
    );
}

// catches: `models` reaching a write: a spawn, a tell or any call beyond its
// one `cloud status` read, in text or JSON. A fresh fake for each process, so
// two text reads cannot hide a JSON run that reads nothing (#1517).
#[test]
fn models_makes_one_status_read_and_never_spawns_or_tells() {
    for args in [&["models"][..], &["models", "--json"][..]] {
        let fake = Models::file("cloud-status.json");
        let (code, out, err) = fake.run(args);
        assert_eq!(code, 0, "{args:?}: {out}{err}");
        assert_eq!(fake.invocations(), [STATUS_READ], "{args:?}: one read");
        for count in ["spawn-calls.count", "tell-calls.count"] {
            assert!(
                !fake.0.0.join("bin").join(count).exists(),
                "{args:?}: {count}"
            );
        }
    }
}

// catches: a usage error reading CloudRoom or printing to stdout.
#[test]
fn models_usage_errors_exit_2_on_stderr_and_read_nothing() {
    let fake = Models::file("cloud-status.json");
    for args in [&["models", "--bogus"][..], &["models", "codex"][..]] {
        let (code, out, err) = fake.run(args);
        assert_eq!(code, 2, "{args:?}: {out}{err}");
        assert!(out.is_empty(), "{out}");
        assert_eq!(err.lines().count(), 1, "{err}");
        assert!(err.starts_with("openwiz: "), "{err}");
    }
    assert!(fake.invocations().is_empty(), "a usage error reads nothing");
}

// catches: `models`' exit codes or usage line dropping out of `--help`.
#[test]
fn models_exit_codes_and_usage_are_documented() {
    let help = openwiz_cli::help();
    for (code, words) in [
        (13, "models: nothing can be offered"),
        (0, "models: options"),
    ] {
        assert!(
            help.lines()
                .any(|l| l.trim_start().starts_with(&format!("{code} ")) && l.contains(words)),
            "exit {code} ({words}) is not documented:\n{help}"
        );
    }
    for words in ["openwiz models [--json]", "openwiz.cloud-options.v1"] {
        assert!(help.contains(words), "{words:?} not in --help:\n{help}");
    }
    let fake = Models::file("cloud-status.json");
    let (code, out, _) = fake.run(&["models", "--help"]);
    assert_eq!(code, 0);
    assert_eq!(out, help);
    assert!(fake.invocations().is_empty(), "help reads nothing");
}

// catches: a failed `cloud status` read printing its stderr (which may hold a
// secret) into the report, or being offered instead of exiting 13.
#[test]
fn models_read_failure_exits_13_and_never_prints_the_source_stderr() {
    let fake = Models::new(json!({"exit": 1, "stderr": "SYNTHETIC_SECRET_DO_NOT_PRINT"}));
    let (code, doc, err) = fake.json();
    assert_eq!(code, 13, "{doc}");
    assert!(
        !format!("{doc}{err}").contains("SYNTHETIC_SECRET_DO_NOT_PRINT"),
        "{doc}{err}"
    );
    let (code, out, err) = fake.run(&["models"]);
    assert_eq!(code, 13, "{out}{err}");
    assert!(
        !format!("{out}{err}").contains("SYNTHETIC_SECRET_DO_NOT_PRINT"),
        "{out}{err}"
    );
}
