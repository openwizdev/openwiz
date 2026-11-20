//! #1804 (MD2): `openwiz models show`, `list` and `set` against every models
//! contract fixture (`contracts/fixtures/models/`), with a fake `cloud
//! status` read that offers exactly the contract's `list/` snapshot. All
//! inputs are synthetic.
use super::models::{FixedClock, Models};
use super::{ROOT, schema};
use openwiz_adapter_cloudroom::source::CloudRoomSource;
use openwiz_core::new_thread::effects::ProjectCandidate;
use openwiz_core::new_thread::operation::{NewThreadClient, ObservedThread, SpawnRequest};
use openwiz_core::new_thread::options::Catalogue;
use openwiz_core::new_thread::outcome::SpawnReply;
use openwiz_core::new_thread::start::ShownThread;
use openwiz_core::{model::Timestamp, source::SourceError};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Child, time::Instant};

const FLEET: &str = "docs/agents/fleet.md";
const SNAPSHOT: &str = "docs/agents/models-offered.json";
const READ: [&str; 3] = ["cloud", "status", "--json"];

fn fixture(rel: &str) -> String {
    fs::read_to_string(Path::new(ROOT).join("contracts/fixtures/models").join(rel)).unwrap()
}

fn doc(rel: &str) -> Value {
    serde_json::from_str(&fixture(rel)).unwrap()
}

/// The `cloud status` answer behind `list/models-offered.json`: two mapped
/// harnesses and one that lists no models.
fn catalogue() -> Value {
    let models = |rows: &[(&str, &[&str])]| -> Value {
        rows.iter()
            .map(|(m, e)| json!({"model": m, "reasoning_levels": e}))
            .collect()
    };
    let all = ["low", "medium", "high", "xhigh", "max"];
    json!({"ready": true, "harnesses": [
        {"id": "claude-code", "model": "syn-default", "reasoning_levels": all, "models": models(&[
            ("claude-opus-5-5", &all), ("claude-sonnet-5-5", &all[..3])])},
        {"id": "codex", "model": "syn-default", "reasoning_levels": all, "models": models(&[
            ("gpt-6.2-sol", &all[..4]), ("vendor/demo", &["medium", "high"]),
            ("syn-vendor/demo:latest", &["medium"])])},
        {"id": "syn-other", "model": "syn-default", "reasoning_levels": all, "models": null},
    ]})
}

/// A sandbox whose `fleet.md` is the named fixture (none when empty) and
/// whose fake `cloud status` answers `answer`.
fn project(fleet: &str, answer: Value) -> Models {
    let fake = Models::new(json!({ "stdout": answer.to_string() }));
    fs::create_dir_all(fake.0.0.join("docs/agents")).unwrap();
    if !fleet.is_empty() {
        fs::write(fake.0.0.join(FLEET), fixture(fleet)).unwrap();
    }
    fake
}

/// `openwiz models <args>` in this process, from the sandbox's root, with
/// the fixed clock: the contract's `read_at`.
fn run(fake: &Models, args: &[&str]) -> (i32, String, String) {
    run_during(fake, args, || {})
}

/// [`run`], with `during` run inside its one `cloud status` read: what
/// another writer does while a `set` waits on CloudRoom.
fn run_during(
    fake: &Models,
    args: &[&str],
    during: impl Fn() + Send + Sync,
) -> (i32, String, String) {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let source = During(CloudRoomSource::new(fake.0.0.join("bin/room-cli")), during);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = openwiz_cli::models::run_models_in(
        &args,
        &fake.0.0,
        &source,
        &FixedClock,
        &mut out,
        &mut err,
    );
    let text = |b| String::from_utf8(b).unwrap();
    (code, text(out), text(err))
}

struct During<F>(CloudRoomSource, F);
impl<F: Fn() + Send + Sync> NewThreadClient for During<F> {
    fn name(&self) -> &str {
        self.0.name()
    }
    fn tested_version(&self) -> &str {
        self.0.tested_version()
    }
    fn cli_levels(&self) -> &[&str] {
        self.0.cli_levels()
    }
    fn version(&self, d: Instant) -> Result<String, SourceError> {
        self.0.version(d)
    }
    fn projects(&self, d: Instant) -> Result<Vec<ProjectCandidate>, SourceError> {
        self.0.projects(d)
    }
    fn catalogue(&self, d: Instant) -> Result<Catalogue, SourceError> {
        (self.1)();
        self.0.catalogue(d)
    }
    fn spawn(&self, s: &SpawnRequest, d: Instant) -> SpawnReply {
        self.0.spawn(s, d)
    }
    fn show_thread(&self, id: &str, d: Instant) -> Result<ShownThread, SourceError> {
        self.0.show_thread(id, d)
    }
    fn threads_since(
        &self,
        p: &str,
        t: Timestamp,
        d: Instant,
    ) -> Result<Vec<ObservedThread>, SourceError> {
        self.0.threads_since(p, t, d)
    }
}

/// The one JSON document a form printed, checked against its definition in
/// `contracts/models.v1.schema.json`.
fn report(out: &str, definition: &str) -> Value {
    assert!(out.ends_with('\n'), "{out}");
    let got: Value = serde_json::from_str(out).expect("one JSON document");
    let mut contract: Value = serde_json::from_str(
        &fs::read_to_string(Path::new(ROOT).join("contracts/models.v1.schema.json")).unwrap(),
    )
    .unwrap();
    contract["$ref"] = json!(format!("#/$defs/{definition}"));
    contract.as_object_mut().unwrap().remove("oneOf");
    assert_eq!(
        schema::validate(&contract, &got),
        Vec::<String>::new(),
        "{got}"
    );
    got
}

/// A contract snapshot with `syn-other`'s text as `openwiz models` words it:
/// the snapshot copies the read's own unavailable text (ruling 1).
fn as_read(mut snapshot: Value) -> Value {
    snapshot["unavailable"][0]["text"] = json!("unavailable (catalogue not reported)");
    snapshot
}

// catches: a reader that disagrees with the contract on any role's harness,
// model, effort or follow, a family's review or fix round, or a warning (a
// stale older table winning, a same-family pair or kept older row unnamed).
#[test]
fn show_prints_every_contract_fixtures_reading_and_reads_nothing_else() {
    for (fleet, want) in [
        ("one-table/fleet.md", "one-table/show.json"),
        ("five-tables/fleet.md", "five-tables/show.json"),
        ("both/fleet.md", "both/show.json"),
        ("overridden/fleet.md", "overridden/show.json"),
        ("same-family/fleet.md", "same-family/show.json"),
        ("set-namespaced/after.md", "set-namespaced/show.json"),
        (
            "set-older-namespaced/after.md",
            "set-older-namespaced/show.json",
        ),
    ] {
        let fake = project(fleet, catalogue());
        let (code, out, err) = run(&fake, &["show", "--json"]);
        assert_eq!((code, err.as_str()), (0, ""), "{fleet}");
        assert_eq!(report(&out, "show_report"), doc(want), "{fleet}");
        assert!(
            fake.invocations().is_empty(),
            "{fleet}: show reads no cloud status"
        );
    }
}

// catches: `list` making more than one read, dropping or adding an offered
// choice, recommending other than J255 or the current Orchestrator, or
// `--save` writing anything but the snapshot (or writing without `--save`).
#[test]
fn list_offers_the_read_with_recommendations_and_saves_only_when_asked() {
    let fake = project("one-table/fleet.md", catalogue());
    let (code, out, _) = run(&fake, &["list", "--json"]);
    let mut want = doc("list/list.json");
    want["offered"] = as_read(want["offered"].take());
    want["saved"] = Value::Null;
    assert_eq!((code, report(&out, "list_report")), (0, want.clone()));
    assert!(!fake.0.0.join(SNAPSHOT).exists(), "saved without --save");
    let (code, out, _) = run(&fake, &["list", "--save", "--json"]);
    want["saved"] = json!(SNAPSHOT);
    assert_eq!((code, report(&out, "list_report")), (0, want));
    let saved: Value =
        serde_json::from_str(&fs::read_to_string(fake.0.0.join(SNAPSHOT)).unwrap()).unwrap();
    assert_eq!(saved, as_read(doc("list/models-offered.json")));
    assert_eq!(fake.invocations(), [READ, READ], "one read per run");
    assert_eq!(
        fs::read_to_string(fake.0.0.join(FLEET)).unwrap(),
        fixture("one-table/fleet.md")
    );
}

// catches: `set` writing a choice that wasn't offered or can't be written,
// changing another role, leaving the older tables out of step or a kept row
// unnamed, a missing same-family warning, or files other than its report's.
#[test]
fn set_does_what_every_contract_case_says_and_writes_only_its_files() {
    for case in [
        "set-older",
        "set-same-family",
        "set-namespaced",
        "set-older-namespaced",
        "unoffered",
        "unwritable",
    ] {
        let spec = doc(&format!("{case}/case.json"));
        let start = spec["fleet"].as_str().unwrap();
        let fake = project(start, catalogue());
        let argv: Vec<&str> = spec["argv"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_str().unwrap())
            .collect();
        assert_eq!(argv[0], "models");
        let (code, out, _) = run(&fake, &argv[1..]);
        assert_eq!(code, spec["exit"], "{case}: {out}");
        assert_eq!(
            report(&out, "set_report"),
            doc(&format!("{case}/set.json")),
            "{case}"
        );
        let writes = &spec["writes"];
        let fleet = writes[FLEET].as_str().unwrap_or(start);
        assert_eq!(
            fs::read_to_string(fake.0.0.join(FLEET)).unwrap(),
            fixture(fleet),
            "{case}"
        );
        match writes[SNAPSHOT].as_str() {
            Some(want) => {
                let saved = fs::read_to_string(fake.0.0.join(SNAPSHOT)).unwrap();
                assert_eq!(
                    serde_json::from_str::<Value>(&saved).unwrap(),
                    as_read(doc(want)),
                    "{case}"
                );
            }
            None => assert!(
                !fake.0.0.join(SNAPSHOT).exists(),
                "{case}: wrote a snapshot"
            ),
        }
        assert_eq!(fake.invocations(), [READ], "{case}: one read");
    }
}

// catches: the contract's text forms drifting: a role, review or fix line,
// the same-family warning, a refusal without the offered list, or `list`
// losing today's text or its recommendation lines.
#[test]
fn the_text_forms_print_the_contracts_lines() {
    let fake = project("same-family/fleet.md", catalogue());
    let (code, out, _) = run(&fake, &["show"]);
    assert_eq!(code, 0, "{out}");
    for line in [
        "Orchestrator: Opus 5.5 Medium on Claude Code (claude-code claude-opus-5-5 medium)",
        "Small builder: follows Builder: Opus 5.5 Medium on Claude Code (claude-code claude-opus-5-5 medium)",
        "Review of Claude-built work: waits",
        "Fix round for a GPT builder: waits",
        "The Reviewer and the Builder are both Claude models, so reviews of Claude-built work wait (ADR 0008).",
    ] {
        assert!(out.lines().any(|l| l == line), "{line:?} not in:\n{out}");
    }
    let (code, out, _) = run(
        &fake,
        &[
            "set",
            "builder",
            "claude-code",
            "syn-model-not-offered",
            "medium",
        ],
    );
    assert_eq!(code, 5, "{out}");
    assert!(
        out.starts_with(
            "not offered: claude-code syn-model-not-offered medium; nothing was changed\n"
        ),
        "{out}"
    );
    assert!(out.contains("  vendor/demo: medium, high\n"), "{out}");
    let (code, out, _) = run(
        &fake,
        &["set", "reviewer", "codex", "gpt-6.2-sol", "medium"],
    );
    assert_eq!(code, 0, "{out}");
    for line in [
        "Reviewer: Opus 5.5 High on Claude Code (claude-code claude-opus-5-5 high) -> Sol 6.2 Medium on Codex (codex gpt-6.2-sol medium)",
        "wrote docs/agents/fleet.md",
        "wrote docs/agents/models-offered.json",
    ] {
        assert!(out.lines().any(|l| l == line), "{line:?} not in:\n{out}");
    }
    let (code, out, _) = run(&fake, &["list"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.starts_with("openwiz models · read 2023-11-14T22:23:20Z · "),
        "{out}"
    );
    assert!(out.ends_with(
        "recommended Builder: Opus 5.5 Medium on Claude Code (claude-code claude-opus-5-5 medium), OpenWiz's current choice (J255)\n\
         recommended Reviewer: Sol 6.2 Medium on Codex (codex gpt-6.2-sol medium), OpenWiz's current choice (J255)\n"
    ), "{out}");
}

// catches: `follows-builder` reading CloudRoom or writing a snapshot, a row
// written in another form than the contract's, or setting a role's current
// value touching the file.
#[test]
fn set_follows_builder_reads_nothing_and_an_unchanged_set_leaves_the_file() {
    let fake = project("overridden/fleet.md", catalogue());
    let (code, out, _) = run(&fake, &["set", "fixer", "follows-builder", "--json"]);
    let got = report(&out, "set_report");
    assert_eq!(
        (code, &got["outcome"], &got["files"]),
        (0, &json!("changed"), &json!([FLEET]))
    );
    let written = fs::read_to_string(fake.0.0.join(FLEET)).unwrap();
    assert!(
        written.contains("\n| Fixer | follows Builder | | | |\n"),
        "{written}"
    );
    assert!(fake.invocations().is_empty() && !fake.0.0.join(SNAPSHOT).exists());
    let (code, out, _) = run(&fake, &["set", "fixer", "follows-builder", "--json"]);
    assert_eq!(
        (code, &report(&out, "set_report")["outcome"]),
        (0, &json!("unchanged"))
    );
    assert_eq!(fs::read_to_string(fake.0.0.join(FLEET)).unwrap(), written);
}

// catches: a usage error that reads CloudRoom or writes, nothing offered
// still writing, or a missing fleet.md not exiting 1 with the CLI's error
// document.
#[test]
fn usage_errors_nothing_offered_and_no_fleet_md_change_nothing() {
    let fake = project("one-table/fleet.md", catalogue());
    for args in [
        &["set", "builder", "follows-builder"][..],
        &["set", "boss", "codex", "gpt-6.2-sol", "medium"],
        &["set", "builder", "codex", "gpt-6.2-sol"],
        &["show", "--json", "--json"],
        &["list", "--save", "--save"],
        &["show", "extra"],
        &["set", "builder", "codex", "gpt-6.2-sol", "medium", "--save"],
    ] {
        let (code, out, err) = run(&fake, args);
        assert_eq!((code, out.as_str()), (2, ""), "{args:?}");
        assert!(
            err.starts_with("openwiz: ") && err.lines().count() == 1,
            "{err}"
        );
    }
    assert!(fake.invocations().is_empty(), "a usage error reads nothing");
    let none = project(
        "one-table/fleet.md",
        json!({"ready": false, "harnesses": null}),
    );
    let (code, out, _) = run(
        &none,
        &["set", "reviewer", "codex", "gpt-6.2-sol", "high", "--json"],
    );
    let got = report(&out, "set_report");
    assert_eq!(
        (code, &got["outcome"], &got["files"]),
        (13, &json!("no_options"), &json!([]))
    );
    let (code, out, _) = run(&none, &["list", "--save", "--json"]);
    assert_eq!(
        (code, &report(&out, "list_report")["saved"]),
        (13, &Value::Null)
    );
    assert!(!none.0.0.join(SNAPSHOT).exists());
    assert_eq!(
        fs::read_to_string(none.0.0.join(FLEET)).unwrap(),
        fixture("one-table/fleet.md")
    );
    let empty = project("", catalogue());
    let (code, out, _) = empty.run(&["models", "show", "--json"]);
    let got: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        (code, &got["schema"], &got["command"]),
        (1, &json!("openwiz.cli-error.v1"), &json!("models"))
    );
}

// catches: a broken Models table (a misnamed or misplaced row, a following
// Builder, broken flags, a wrong header, two `## Models`) that `show` or
// `set` still reads, or that `set` rewrites; and `show` writing anything.
#[test]
fn a_broken_models_table_is_refused_by_show_and_set_and_never_written() {
    let good = fixture("one-table/fleet.md");
    let builder = "| Builder | Claude Code | Opus 5.5 | Medium | `--provider claude-code --model claude-opus-5-5 --reasoning-level medium` |";
    for (from, to) in [
        ("| Reviewer | Codex", "| Reviewers | Codex"),
        (builder, "| Builder | follows Builder | | | |"),
        (
            "--reasoning-level medium` |\n| Reviewer",
            "--effort medium` |\n| Reviewer",
        ),
        (
            "| Small builder | follows Builder | | | |\n| Fixer",
            "| Fixer | follows Builder | | | |\n| Small builder",
        ),
        ("`room-cli thread spawn` flags |", "flags |"),
        ("## Loop", "## Models\n\n## Loop"),
    ] {
        assert!(good.contains(from), "{from}");
        let broken = good.replacen(from, to, 1);
        let fake = project("one-table/fleet.md", catalogue());
        fs::write(fake.0.0.join(FLEET), &broken).unwrap();
        for args in [
            &["show", "--json"][..],
            &["set", "fixer", "follows-builder"],
        ] {
            let (code, out, err) = run(&fake, args);
            assert_eq!((code, out.as_str()), (1, ""), "{to}: {args:?}");
            assert!(err.starts_with("openwiz: models "), "{err}");
        }
        assert_eq!(
            fs::read_to_string(fake.0.0.join(FLEET)).unwrap(),
            broken,
            "{to}"
        );
        let mut left: Vec<_> = fs::read_dir(fake.0.0.join("docs/agents"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        left.sort();
        assert_eq!(left, ["fleet.md"], "{to}: wrote another file");
        assert!(fake.invocations().is_empty(), "{to}");
    }
}

// catches (R1): a write that replaces or removes a file it didn't create
// (one already at a staging path), or that follows a linked `docs/agents`
// out of the checkout.
#[test]
fn writes_create_their_own_staging_file_and_stay_in_the_checkout() {
    let fake = project("one-table/fleet.md", catalogue());
    let agents = fake.0.0.join("docs/agents");
    for name in [
        "models-offered.json.openwiz-writing",
        "fleet.md.openwiz-writing",
    ] {
        fs::write(agents.join(name), "synthetic bystander\n").unwrap();
    }
    let (code, _, err) = run(&fake, &["set", "reviewer", "codex", "gpt-6.2-sol", "high"]);
    assert_eq!(code, 0, "{err}");
    let mut left: Vec<_> = fs::read_dir(&agents)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    left.sort();
    assert_eq!(
        left,
        [
            "fleet.md",
            "fleet.md.openwiz-writing",
            "models-offered.json",
            "models-offered.json.openwiz-writing"
        ]
    );
    for name in [
        "models-offered.json.openwiz-writing",
        "fleet.md.openwiz-writing",
    ] {
        assert_eq!(
            fs::read_to_string(agents.join(name)).unwrap(),
            "synthetic bystander\n"
        );
    }
    let outside = fake.0.0.join("outside");
    fs::rename(&agents, &outside).unwrap();
    std::os::unix::fs::symlink(&outside, &agents).unwrap();
    let before = fs::read_to_string(outside.join("fleet.md")).unwrap();
    for args in [
        &["set", "fixer", "follows-builder"][..],
        &["list", "--save"],
    ] {
        let (code, _, err) = run(&fake, args);
        assert_eq!(code, 1, "{args:?}: {err}");
    }
    assert_eq!(
        fs::read_to_string(outside.join("fleet.md")).unwrap(),
        before
    );
}

// catches (R2): a `set` that replaces fleet.md over an edit made while it
// waited on CloudRoom, or two setters where the later one reverts the other.
#[test]
fn set_never_loses_an_edit_made_while_it_reads_cloudroom() {
    let fake = project("one-table/fleet.md", catalogue());
    let fleet = fake.0.0.join(FLEET);
    let edited = format!(
        "{}\nA synthetic edit made meanwhile.\n",
        fixture("one-table/fleet.md")
    );
    let (code, out, err) = run_during(
        &fake,
        &["set", "orchestrator", "codex", "gpt-6.2-sol", "high"],
        || fs::write(&fleet, &edited).unwrap(),
    );
    assert_eq!((code, out.as_str()), (1, ""), "{err}");
    assert_eq!(fs::read_to_string(&fleet).unwrap(), edited);
    assert!(!fake.0.0.join(SNAPSHOT).exists(), "wrote a snapshot");
    let fake = project("overridden/fleet.md", catalogue());
    let other = std::sync::Mutex::new(None::<Child>);
    let (code, _, err) = run_during(
        &fake,
        &["set", "orchestrator", "codex", "gpt-6.2-sol", "medium"],
        || {
            let mut child = fake.spawn(&["models", "set", "fixer", "follows-builder"]);
            // Long enough for an unserialized setter to finish; a serialized
            // one waits for this `set` whatever the time.
            std::thread::sleep(std::time::Duration::from_millis(500));
            let _ = child.try_wait();
            *other.lock().unwrap() = Some(child);
        },
    );
    assert_eq!(code, 0, "{err}");
    let status = other.lock().unwrap().take().unwrap().wait().unwrap();
    assert!(status.success());
    let (_, out, _) = run(&fake, &["show", "--json"]);
    let show: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        show["roles"]["orchestrator"]["choice"]["effort"], "medium",
        "{show}"
    );
    assert_eq!(show["roles"]["fixer"]["follows"], "builder", "{show}");
}

// catches (R3): older tables with a surplus or duplicate row, a second table
// or two Fixer rows of one family read, or migrated by `set`, as if whole.
#[test]
fn older_tables_with_surplus_rows_are_refused() {
    let five = fixture("five-tables/fleet.md");
    let effort = "| Effort | Medium | `--reasoning-level medium` |\n";
    let claude = "| Claude | Claude Code | Opus 5.5 | Medium | `--provider claude-code --model claude-opus-5-5 --reasoning-level medium` |\n";
    for (from, to) in [
        (effort.to_owned(), format!("{effort}{effort}")),
        (claude.to_owned(), format!("{claude}{claude}")),
        (
            "## Builder for small tickets\n".to_owned(),
            format!("{effort}\n| a | b | c |\n\n## Builder for small tickets\n"),
        ),
    ] {
        assert!(five.contains(&from), "{from}");
        let fake = project("five-tables/fleet.md", catalogue());
        let broken = five.replacen(&from, &to, 1);
        fs::write(fake.0.0.join(FLEET), &broken).unwrap();
        for args in [&["show"][..], &["set", "fixer", "follows-builder"]] {
            assert_eq!(run(&fake, args).0, 1, "{to}: {args:?}");
        }
        assert_eq!(fs::read_to_string(fake.0.0.join(FLEET)).unwrap(), broken);
    }
}

// catches (R4): an older row written with an id today's older readers
// refuse (an upper-case model id), instead of kept and named, which stops
// every fleet still reading the older tables.
#[test]
fn an_id_the_older_readers_refuse_stays_in_the_models_table_only() {
    let mut offer = catalogue();
    let demo = json!({"model": "Demo.Model", "reasoning_levels": ["medium"]});
    offer["harnesses"][0]["models"]
        .as_array_mut()
        .unwrap()
        .push(demo);
    let fake = project("five-tables/fleet.md", offer);
    let (code, out, _) = run(
        &fake,
        &[
            "set",
            "builder",
            "claude-code",
            "Demo.Model",
            "medium",
            "--json",
        ],
    );
    let got = report(&out, "set_report");
    assert_eq!(
        (code, &got["after"]["choice"]["model"]),
        (0, &json!("Demo.Model"))
    );
    assert_eq!(
        got["older_tables"][0],
        json!({"table": "Builder", "outcome": "kept"})
    );
    let written = fs::read_to_string(fake.0.0.join(FLEET)).unwrap();
    assert!(!written.contains("`--model Demo.Model`"), "{written}");
    assert!(
        got["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["table"] == "Builder"),
        "{got}"
    );
}

/// Runs `step` on another thread over and over until `body` returns.
fn racing<T>(step: impl Fn() + Send + Sync, body: impl FnOnce() -> T) -> T {
    let stop = std::sync::atomic::AtomicBool::new(false);
    std::thread::scope(|s| {
        s.spawn(|| {
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                step();
            }
        });
        let out = body();
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        out
    })
}

// catches (R1, re-check): a write checked once and then made by pathname, so
// `docs/agents` swapped for a link to a folder outside the checkout between
// the check and the staging write, rename or cleanup puts the file there.
#[test]
fn every_write_goes_through_the_directory_it_checked() {
    let fake = project("one-table/fleet.md", catalogue());
    let agents = fake.0.0.join("docs/agents");
    let aside = fake.0.0.join("docs/agents-aside");
    let outside = fake.0.0.join("outside");
    fs::create_dir(&outside).unwrap();
    let fleet = fixture("one-table/fleet.md");
    fs::write(outside.join("fleet.md"), &fleet).unwrap();
    racing(
        || {
            if fs::rename(&agents, &aside).is_ok() {
                std::os::unix::fs::symlink(&outside, &agents).unwrap();
                fs::remove_file(&agents).unwrap();
                fs::rename(&aside, &agents).unwrap();
            }
        },
        || {
            for _ in 0..400 {
                let _ = run(&fake, &["list", "--save"]);
            }
        },
    );
    let mut left: Vec<_> = fs::read_dir(&outside)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    left.sort();
    assert_eq!(left, ["fleet.md"], "a write landed outside the checkout");
    assert_eq!(fs::read_to_string(outside.join("fleet.md")).unwrap(), fleet);
}

/// `openwiz models <args>` as a process under the common 022 umask, so a
/// file created with the default mode is world-readable whatever this
/// test's own umask; returns its exit code.
fn under_umask_022(fake: &Models, args: &[&str]) -> i32 {
    let root = &fake.0.0;
    let path = format!(
        "{}:{}",
        root.join("bin").display(),
        std::env::var("PATH").unwrap()
    );
    std::process::Command::new("sh")
        .args([
            "-c",
            "umask 022; exec \"$0\" models \"$@\"",
            &super::fixture_cli().to_string_lossy(),
        ])
        .args(args)
        .env_clear()
        .env("PATH", path)
        .env("HOME", root.join("home"))
        .env("OPENWIZ_STATE_DIR", root.join("state"))
        .env("OPENWIZ_FIXTURE_PROGRAM", root.join("bin/room-cli"))
        .current_dir(root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap()
        .code()
        .expect("exited")
}

// catches (R5): a staged copy created with the process's default mode
// (0644 under a 022 umask) while the file it replaces is owner-only.
#[test]
fn a_staged_copy_is_never_more_readable_than_its_target() {
    use std::os::unix::fs::PermissionsExt;
    let fake = project("one-table/fleet.md", catalogue());
    let agents = fake.0.0.join("docs/agents");
    let owner_only = fs::Permissions::from_mode(0o600);
    fs::write(agents.join("models-offered.json"), "{}\n").unwrap();
    for name in ["fleet.md", "models-offered.json"] {
        fs::set_permissions(agents.join(name), owner_only.clone()).unwrap();
    }
    let seen = std::sync::Mutex::new(Vec::new());
    racing(
        || {
            for entry in fs::read_dir(&agents).unwrap().flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if let (true, Ok(meta)) = (name.starts_with('.'), entry.metadata()) {
                    seen.lock()
                        .unwrap()
                        .push((name, meta.permissions().mode() & 0o777));
                }
            }
        },
        || {
            for effort in ["high", "medium"].iter().cycle().take(40) {
                let args = ["set", "reviewer", "codex", "gpt-6.2-sol", effort];
                assert_eq!(under_umask_022(&fake, &args), 0);
            }
        },
    );
    let seen = seen.into_inner().unwrap();
    assert!(!seen.is_empty(), "never saw a staged copy");
    let mut open: Vec<_> = seen
        .iter()
        .filter(|(_, mode)| mode & 0o077 != 0)
        .map(|(name, mode)| format!("{} {mode:o}", name.split('.').nth(1).unwrap_or("")))
        .collect();
    open.sort();
    open.dedup();
    assert!(open.is_empty(), "staged copies of 600 targets: {open:?}");
    for name in ["fleet.md", "models-offered.json"] {
        let mode = fs::metadata(agents.join(name))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "{name}");
    }
}

// catches (planner J87, ruling 16 overturned): a writer that refuses off
// Linux or names its files through `/proc`, so `set` and `list --save` do
// nothing on macOS; and on every Unix, a `docs/agents` that is a link, or is
// swapped for one while `set` reads CloudRoom, moving a write outside.
#[test]
fn a_raced_docs_agents_is_refused_the_same_on_every_unix() {
    let source = fs::read_to_string(Path::new(ROOT).join("crates/cli/src/models.rs")).unwrap();
    for route in ["target_os", "/proc/"] {
        assert!(!source.contains(route), "the writer depends on {route}");
    }
    let fake = project("one-table/fleet.md", catalogue());
    let agents = fake.0.0.join("docs/agents");
    let aside = fake.0.0.join("docs/agents-aside");
    let outside = fake.0.0.join("outside");
    fs::create_dir(&outside).unwrap();
    let fleet = fixture("one-table/fleet.md");
    fs::write(outside.join("fleet.md"), &fleet).unwrap();
    let (code, _, err) = run_during(
        &fake,
        &["set", "reviewer", "codex", "gpt-6.2-sol", "high"],
        || {
            fs::rename(&agents, &aside).unwrap();
            std::os::unix::fs::symlink(&outside, &agents).unwrap();
        },
    );
    assert_eq!(code, 0, "{err}");
    assert_ne!(fs::read_to_string(aside.join("fleet.md")).unwrap(), fleet);
    assert!(aside.join("models-offered.json").exists());
    for args in [
        &["set", "fixer", "follows-builder"][..],
        &["list", "--save"],
    ] {
        let (code, _, err) = run(&fake, args);
        assert_eq!(code, 1, "{args:?}");
        assert!(
            err.contains("docs/agents is not a directory of this checkout"),
            "{err}"
        );
    }
    let mut left: Vec<_> = fs::read_dir(&outside)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    left.sort();
    assert_eq!(left, ["fleet.md"], "a write landed outside the checkout");
    assert_eq!(fs::read_to_string(outside.join("fleet.md")).unwrap(), fleet);
}
