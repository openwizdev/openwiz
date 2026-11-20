//! `openwiz models`: the Cloud harnesses, models and efforts CloudRoom offers
//! right now, from one `cloud status` read. Read-only: no state, no write.
//! Kept from the removed `new` command's module by J233 (#1264):
//! setup's harness check and the onboarding skill read its output.
//!
//! `openwiz models show`, `list` and `set` (#1804, MD2) are the models
//! contract's forms (`contracts/models.v1.md`): they read the project's
//! `docs/agents/fleet.md` through the core's [`openwiz_core::models`] reader,
//! and `list --save` and `set` write that project file and the offered-models
//! snapshot beside it, for the agent to commit.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use openwiz_adapter_cloudroom::allowlist::{Invocation, SpawnArgs};
use openwiz_core::limits::READ_TIMEOUT;
use openwiz_core::models::write::{Asked, RowOutcome, set};
use openwiz_core::models::{
    Choice, FLEET_FILE, Family, Form, From, Pick, Role, RoleRead, SNAPSHOT_FILE, Show, SpawnFlags,
    read,
};
use openwiz_core::new_thread::operation::NewThreadClient;
use openwiz_core::new_thread::options::{CloudOptions, OptionsState, cloud_options};
use openwiz_core::ops::Clock;
use rustix::fs::{AtFlags, CWD, FileType, Mode, OFlags};
use serde_json::{Value, json};

use crate::{EXIT_OK, EXIT_READ_FAILED, EXIT_USAGE, clean, help, utc};

/// Nothing can be offered right now; the report says why.
pub const EXIT_NO_OPTIONS: i32 = 13;
/// `models set`: the choice is not offered, or can't be written to the
/// Models table; nothing was changed.
pub const EXIT_REFUSED: i32 = 5;

/// Runs `openwiz models <args>` and returns the exit code: one `cloud
/// status` read.
pub fn run_models(
    args: &[String],
    client: &dyn NewThreadClient,
    clock: &dyn Clock,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let root = std::env::current_dir().unwrap_or_default();
    run_models_in(args, &root, client, clock, stdout, stderr)
}

/// `openwiz models <args>` run from the project root `root`.
pub fn run_models_in(
    args: &[String],
    root: &Path,
    client: &dyn NewThreadClient,
    clock: &dyn Clock,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    if matches!(
        args.first().map(String::as_str),
        Some("show" | "list" | "set")
    ) {
        return run_form(args, root, client, clock, stdout, stderr);
    }
    let mut json = false;
    for arg in args {
        match arg.as_str() {
            "--help" | "-h" => {
                let _ = stdout.write_all(
                    crate::response::help(
                        crate::response::Request::for_command("models", args).json,
                        help(),
                    )
                    .as_bytes(),
                );
                return EXIT_OK;
            }
            "--json" => json = true,
            other => {
                let _ = writeln!(
                    stderr,
                    "openwiz: models takes only --json, not {}; nothing was read (see openwiz \
                     --help)",
                    clean(other)
                );
                return EXIT_USAGE;
            }
        }
    }
    let deadline = clock.monotonic() + READ_TIMEOUT;
    let options = cloud_options(client.catalogue(deadline), client.cli_levels(), clock.now());
    let printed = if json {
        let mut out = serde_json::to_string_pretty(&options)
            .unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"));
        out.push('\n');
        out
    } else {
        render_options(&options)
    };
    let _ = stdout.write_all(printed.as_bytes());
    let _ = stdout.flush();
    match options.state {
        OptionsState::Offered => EXIT_OK,
        OptionsState::NoOptions { .. } => EXIT_NO_OPTIONS,
    }
}

/// The options as text.
pub fn render_options(options: &CloudOptions) -> String {
    let mut out = format!(
        "openwiz models · read {} · {}\n",
        utc(options.read_at),
        clean(&options.label)
    );
    if let OptionsState::NoOptions { text, .. } = &options.state {
        out.push_str(&format!("no options: {}\n", clean(text)));
    }
    for harness in &options.harnesses {
        out.push_str(&format!(
            "{} (spawn provider {})\n",
            clean(&harness.harness),
            clean(&harness.provider)
        ));
        for model in &harness.models {
            out.push_str(&format!(
                "  {}: {}\n",
                clean(&model.model),
                clean(&model.efforts.join(", "))
            ));
        }
    }
    for harness in &options.unavailable {
        out.push_str(&format!(
            "{}: {}\n",
            clean(&harness.harness),
            clean(&harness.text)
        ));
    }
    out
}

/// A parsed `show`, `list` or `set`.
enum Parsed {
    Help,
    Show,
    List {
        save: bool,
    },
    Set {
        role: Role,
        asked: Option<[String; 3]>,
    },
}

const SET_USAGE: &str = "models set takes <role> <provider> <model> <effort>, or \
                         <small-builder|fixer> follows-builder";

fn parse_form(args: &[String]) -> Result<(Parsed, bool), String> {
    let form = args[0].as_str();
    let (mut json, mut save, mut words) = (false, false, Vec::new());
    for arg in &args[1..] {
        match arg.as_str() {
            "--help" | "-h" => return Ok((Parsed::Help, false)),
            "--json" if !json => json = true,
            "--save" if form == "list" && !save => save = true,
            "--json" | "--save" => return Err(format!("models {form} takes {arg} once")),
            other if other.starts_with('-') => {
                return Err(format!("models {form} has no option {}", clean(other)));
            }
            other => words.push(other),
        }
    }
    let parsed = match (form, &words[..]) {
        ("show", []) => Parsed::Show,
        ("list", []) => Parsed::List { save },
        ("show" | "list", _) => return Err(format!("models {form} takes no operands")),
        (_, [role, rest @ ..]) => {
            let role = Role::from_token(role).ok_or_else(|| {
                format!(
                    "models set has no role {}: orchestrator, builder, reviewer, \
                     small-builder or fixer",
                    clean(role)
                )
            })?;
            match rest {
                ["follows-builder"] if role.can_follow() => Parsed::Set { role, asked: None },
                [p, m, e] => Parsed::Set {
                    role,
                    asked: Some([p.to_string(), m.to_string(), e.to_string()]),
                },
                _ => return Err(SET_USAGE.to_owned()),
            }
        }
        _ => return Err(SET_USAGE.to_owned()),
    };
    Ok((parsed, json))
}

/// `models show|list|set`: exit codes as the models contract says.
fn run_form(
    args: &[String],
    root: &Path,
    client: &dyn NewThreadClient,
    clock: &dyn Clock,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let (parsed, json) = match parse_form(args) {
        Ok(parsed) => parsed,
        Err(why) => {
            let _ = writeln!(
                stderr,
                "openwiz: {why}; nothing was read (see openwiz --help)"
            );
            return EXIT_USAGE;
        }
    };
    let form = args[0].as_str();
    let fleet = || {
        std::fs::read_to_string(root.join(FLEET_FILE))
            .map_err(|e| format!("cannot read {FLEET_FILE}: {e}"))
    };
    let read_options = || {
        cloud_options(
            client.catalogue(clock.monotonic() + READ_TIMEOUT),
            client.cli_levels(),
            clock.now(),
        )
    };
    let outcome = match parsed {
        Parsed::Help => {
            let text = crate::response::help(
                crate::response::Request::for_command("models", args).json,
                help(),
            );
            return emit(stdout, stderr, (EXIT_OK, text));
        }
        Parsed::Show => fleet().and_then(|text| read_fleet(&text)).map(|show| {
            let out = if json {
                document(&show)
            } else {
                show_text(&show)
            };
            (EXIT_OK, out)
        }),
        Parsed::List { save } => {
            let options = read_options();
            let current = fleet().ok().and_then(|text| read_fleet(&text).ok());
            let snapshot = snapshot(&options, current.as_ref());
            let offered = matches!(options.state, OptionsState::Offered);
            let saved = save && offered;
            let written = if saved {
                Agents::open(root)
                    .and_then(|agents| put(&agents, SNAPSHOT_FILE, &document(&snapshot)))
            } else {
                Ok(())
            };
            written.map(|()| {
                let code = if offered { EXIT_OK } else { EXIT_NO_OPTIONS };
                let out = if json {
                    document(&json!({
                        "schema": "openwiz.models-list.v1",
                        "offered": snapshot,
                        "saved": if saved { json!(SNAPSHOT_FILE) } else { Value::Null },
                    }))
                } else {
                    format!(
                        "{}{}",
                        render_options(&options),
                        recommended_text(&snapshot)
                    )
                };
                (code, out)
            })
        }
        // One setter at a time: the lock is held from before fleet.md is
        // read until the last write (R2), and every read and write goes
        // through the one `docs/agents` it checked and locked (R1).
        Parsed::Set { role, asked } => Agents::open(root).and_then(|agents| {
            agents.lock()?;
            let text = agents.read(FLEET_FILE)?;
            run_set(&text, role, asked, &agents, json, &read_options)
        }),
    };
    match outcome {
        Ok(printed) => emit(stdout, stderr, printed),
        Err(why) => {
            let _ = writeln!(stderr, "openwiz: models {form}: {}", clean(&why));
            EXIT_READ_FAILED
        }
    }
}

/// The provider, model and effort flag words of a Models table's flags
/// cell, as the `room-cli` allowlist's one spawn builder spells them: a
/// synthetic spawn invocation is built (never run) and the words beside its
/// provider, model and effort values are read back.
fn spawn_flags() -> Result<SpawnFlags, String> {
    let (provider, model, effort) = ("claude-code", "syn-models-flags", "medium");
    let request = format!("ow1-{}", "0".repeat(40));
    let args = SpawnArgs::new("syn-models", provider, model, effort, &request, "syn")
        .map_err(|_| "the allowlist refused the synthetic flags invocation".to_owned())?;
    let argv = Invocation::ThreadSpawn(args).argv();
    let word = |value: &str| {
        let at = argv.iter().position(|w| w == value).filter(|&at| at > 0);
        at.map(|at| argv[at - 1].clone())
            .ok_or_else(|| "the allowlist's spawn flags were not found".to_owned())
    };
    Ok(SpawnFlags {
        provider: word(provider)?,
        model: word(model)?,
        effort: word(effort)?,
    })
}

/// [`read`], with the allowlist's spawn flags.
fn read_fleet(text: &str) -> Result<Show, String> {
    read(text, &spawn_flags()?)
}

fn run_set(
    text: &str,
    role: Role,
    asked: Option<[String; 3]>,
    agents: &Agents,
    json: bool,
    read_options: &dyn Fn() -> CloudOptions,
) -> Result<(i32, String), String> {
    let before = read_fleet(text)?.roles.get(role).clone();
    let (asked_doc, asked) = match &asked {
        None => (json!({"follows": "builder"}), None),
        Some([p, m, e]) => (
            json!({"provider": p, "model": m, "effort": e}),
            Some([p, m, e]),
        ),
    };
    let refused = |code: i32, outcome: &str, reason: &str, why: String, offered: Value| {
        let report = json!({
            "schema": "openwiz.models-set.v1", "outcome": outcome, "role": role,
            "asked": asked_doc, "before": before, "after": null,
            "models_table_added": false, "older_tables": [], "files": [], "warnings": [],
            "refusal": {"reason": reason, "text": why}, "offered": offered,
        });
        let out = if json {
            document(&report)
        } else {
            let mut out = format!("{}\n", clean(&why));
            if let Some(harnesses) = report["offered"].as_array() {
                out.push_str(&harness_lines(harnesses));
            }
            out
        };
        Ok((code, out))
    };
    let (chosen, options) = match asked {
        None => (Asked::Follows, None),
        Some([p, m, e]) => {
            let options = read_options();
            if let OptionsState::NoOptions { text, .. } = &options.state {
                return refused(
                    EXIT_NO_OPTIONS,
                    "no_options",
                    "no_options",
                    text.clone(),
                    Value::Null,
                );
            }
            let offered = json!(offered_harnesses(&options));
            if !is_offered(&options, p, m, e) {
                let why = format!("not offered: {p} {m} {e}; nothing was changed");
                return refused(EXIT_REFUSED, "refused", "not_offered", why, offered);
            }
            let Some(choice) = Choice::named(p, m, e) else {
                let why = format!(
                    "cannot be written to the Models table: {p} {m} {e}; nothing was changed"
                );
                return refused(EXIT_REFUSED, "refused", "not_writable", why, offered);
            };
            (Asked::Choice(choice), Some(options))
        }
    };
    let result = set(text, role, &chosen, &spawn_flags()?)?;
    // Only setters take the lock: anything else that edited fleet.md while
    // this read CloudRoom wins, and nothing is written (R2).
    if agents.read(FLEET_FILE).ok().as_deref() != Some(text) {
        return Err(format!(
            "{FLEET_FILE} changed while set ran, so nothing was written; run it again"
        ));
    }
    let mut files = Vec::new();
    // The snapshot first: if fleet.md then fails, only the offered list moved.
    if let Some(options) = &options {
        let current = read_fleet(result.text.as_deref().unwrap_or(text)).ok();
        put(
            agents,
            SNAPSHOT_FILE,
            &document(&snapshot(options, current.as_ref())),
        )?;
        files.push(SNAPSHOT_FILE);
    }
    if let Some(new) = &result.text {
        put(agents, FLEET_FILE, new)?;
        files.insert(0, FLEET_FILE);
    }
    let outcome = if result.text.is_some() {
        "changed"
    } else {
        "unchanged"
    };
    let report = json!({
        "schema": "openwiz.models-set.v1", "outcome": outcome, "role": role,
        "asked": asked_doc, "before": before, "after": result.after,
        "models_table_added": result.models_table_added, "older_tables": result.older_tables,
        "files": files, "warnings": result.warnings, "refusal": null, "offered": null,
    });
    if json {
        return Ok((EXIT_OK, document(&report)));
    }
    let mut out = format!(
        "{}: {} -> {}\n",
        role.row(),
        role_text(&before),
        role_text(&result.after)
    );
    for row in result
        .older_tables
        .iter()
        .filter(|r| r.outcome != RowOutcome::Unchanged)
    {
        let word = if row.outcome == RowOutcome::Kept {
            "kept"
        } else {
            "changed"
        };
        out.push_str(&format!("{}: {word}\n", row.table));
    }
    for warning in &result.warnings {
        out.push_str(&format!("{}\n", warning.text()));
    }
    for file in files {
        out.push_str(&format!("wrote {file}\n"));
    }
    Ok((EXIT_OK, clean_lines(&out)))
}

/// The offered harnesses as the snapshot lists them: no attachment or
/// steer facts.
fn offered_harnesses(options: &CloudOptions) -> Vec<Value> {
    options
        .harnesses
        .iter()
        .map(|h| json!({"harness": h.harness, "provider": h.provider, "models": h.models}))
        .collect()
}

fn is_offered(options: &CloudOptions, provider: &str, model: &str, effort: &str) -> bool {
    options.harnesses.iter().any(|h| {
        h.provider == provider
            && h.models
                .iter()
                .any(|m| m.model == model && m.efforts.iter().any(|e| e == effort))
    })
}

/// The offered-models snapshot (`openwiz.models-offered.v1`) of one read,
/// with the recommendations; `current` is the project's `fleet.md`, if read.
fn snapshot(options: &CloudOptions, current: Option<&Show>) -> Value {
    let recommend = |c: Option<Choice>, source: &str| {
        let offered = c
            .as_ref()
            .is_some_and(|c| is_offered(options, &c.provider, &c.model, &c.effort));
        json!({"choice": c, "source": source, "offered": offered})
    };
    let j255 = "OpenWiz's current choice (J255)";
    let orchestrator = match current {
        Some(show) if show.read == Form::ModelsTable => recommend(
            Some(show.roles.orchestrator.choice.clone()),
            "your current Orchestrator row",
        ),
        Some(show) => recommend(
            Some(show.roles.orchestrator.choice.clone()),
            "your current Builder table (the older tables have no Orchestrator row)",
        ),
        None => recommend(None, "no fleet.md read"),
    };
    json!({
        "schema": "openwiz.models-offered.v1",
        "read_at": options.read_at,
        "label": options.label,
        "state": options.state,
        "harnesses": offered_harnesses(options),
        "unavailable": options.unavailable,
        "recommended": {
            "orchestrator": orchestrator,
            "builder": recommend(Choice::named("claude-code", "claude-opus-5-5", "medium"), j255),
            "reviewer": recommend(Choice::named("codex", "gpt-6.2-sol", "medium"), j255),
        },
    })
}

fn recommended_text(snapshot: &Value) -> String {
    let mut out = String::new();
    for (key, row) in [
        ("orchestrator", "Orchestrator"),
        ("builder", "Builder"),
        ("reviewer", "Reviewer"),
    ] {
        let rec = &snapshot["recommended"][key];
        let source = rec["source"].as_str().unwrap_or_default();
        let line = match &rec["choice"] {
            Value::Null => format!("recommended {row}: none ({source})"),
            c => {
                let shown = format!(
                    "{} {} on {} ({} {} {})",
                    c["names"]["model"].as_str().unwrap_or_default(),
                    c["names"]["effort"].as_str().unwrap_or_default(),
                    c["names"]["harness"].as_str().unwrap_or_default(),
                    c["provider"].as_str().unwrap_or_default(),
                    c["model"].as_str().unwrap_or_default(),
                    c["effort"].as_str().unwrap_or_default(),
                );
                if rec["offered"] == true {
                    format!("recommended {row}: {shown}, {source}")
                } else {
                    format!("recommended {row}: not offered now: {shown}")
                }
            }
        };
        out.push_str(&clean(&line));
        out.push('\n');
    }
    out
}

fn role_text(role: &RoleRead) -> String {
    match role.follows {
        Some(_) => format!("follows Builder: {}", role.choice.shown()),
        None => role.choice.shown(),
    }
}

fn pick_text(pick: &Pick) -> String {
    let from = match pick.from {
        From::Waits => return "waits".to_owned(),
        From::Reviewer => "Reviewer",
        From::Builder => "Builder",
        From::Fixer => "Fixer",
        From::OlderTable => "older table",
    };
    let shown = pick.choice.as_ref().map(Choice::shown).unwrap_or_default();
    format!("{from}: {shown}")
}

fn show_text(show: &Show) -> String {
    let mut out = String::new();
    for role in Role::ALL {
        out.push_str(&format!(
            "{}: {}\n",
            role.row(),
            role_text(show.roles.get(role))
        ));
    }
    for family in Family::BOTH {
        let f = family.shown();
        out.push_str(&format!(
            "Review of {f}-built work: {}\n",
            pick_text(show.reviews.get(family))
        ));
        out.push_str(&format!(
            "Fix round for a {f} builder: {}\n",
            pick_text(show.fixers.get(family))
        ));
    }
    for warning in &show.warnings {
        out.push_str(&format!("{}\n", warning.text()));
    }
    clean_lines(&out)
}

fn harness_lines(harnesses: &[Value]) -> String {
    let mut out = String::new();
    for h in harnesses {
        out.push_str(&format!(
            "{} (spawn provider {})\n",
            h["harness"].as_str().unwrap_or_default(),
            h["provider"].as_str().unwrap_or_default()
        ));
        for m in h["models"].as_array().into_iter().flatten() {
            let efforts: Vec<&str> = m["efforts"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            out.push_str(&format!(
                "  {}: {}\n",
                m["model"].as_str().unwrap_or_default(),
                efforts.join(", ")
            ));
        }
    }
    clean_lines(&out)
}

/// Each line made safe for a terminal, the newlines kept.
fn clean_lines(text: &str) -> String {
    text.lines().map(|l| format!("{}\n", clean(l))).collect()
}

fn document(value: &impl serde::Serialize) -> String {
    let mut out =
        serde_json::to_string_pretty(value).unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"));
    out.push('\n');
    out
}

/// The checkout's `docs/agents`, held open: `docs`, then `agents` inside
/// it, each opened as a directory and never through a link, so nothing is
/// read for a write or written otherwise (R1). Every later step is a call
/// relative to this handle (`openat`, `fstatat`, `renameat`, `unlinkat`,
/// through `rustix`), so a folder swapped for a link after the check can't
/// move a read, staging write, rename or cleanup anywhere else, on Linux and
/// macOS alike.
struct Agents(File);

impl Agents {
    fn open(root: &Path) -> Result<Self, String> {
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let refused = |dir: &str| {
            format!(
                "{dir} is not a directory of this checkout (missing or a link); nothing was written"
            )
        };
        let docs = rustix::fs::openat(CWD, root.join("docs"), flags, Mode::empty())
            .map_err(|_| refused("docs"))?;
        rustix::fs::openat(&docs, "agents", flags, Mode::empty())
            .map(|fd| Agents(File::from(fd)))
            .map_err(|_| refused("docs/agents"))
    }

    fn read(&self, rel: &str) -> Result<String, String> {
        let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let mut text = String::new();
        rustix::fs::openat(&self.0, name(rel), flags, Mode::empty())
            .map_err(std::io::Error::from)
            .and_then(|fd| File::from(fd).read_to_string(&mut text))
            .map_err(|e| format!("cannot read {rel}: {e}"))?;
        Ok(text)
    }

    /// Locks this folder for one `set`, so setters run one at a time;
    /// released when it is dropped. It creates no file.
    fn lock(&self) -> Result<(), String> {
        self.0
            .lock()
            .map_err(|e| format!("cannot lock docs/agents: {e}"))
    }
}

/// A `docs/agents/<name>` path's last part.
fn name(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

/// Writes a project file whole, through `docs/agents` held open (R1): the
/// bytes go to a fresh file created exclusively beside it (never one
/// already there, link or not), owner-only and then given the target's
/// permissions before any byte is written (R5), which is then renamed over
/// the target. On a failure only that fresh file is removed; a target that
/// is not a regular file is refused.
fn put(agents: &Agents, rel: &str, text: &str) -> Result<(), String> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let fail = |e: std::io::Error| format!("cannot write {rel}: {e}");
    let dir = &agents.0;
    let target = name(rel);
    let existing = rustix::fs::statat(dir, target, AtFlags::SYMLINK_NOFOLLOW).ok();
    if existing.is_some_and(|st| FileType::from_raw_mode(st.st_mode) != FileType::RegularFile) {
        return Err(format!("cannot write {rel}: it is not a regular file"));
    }
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let temporary = format!(".{target}.openwiz-{}-{n}", std::process::id());
    let create =
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mode = match existing {
        Some(_) => Mode::RUSR | Mode::WUSR,
        None => Mode::RUSR | Mode::WUSR | Mode::RGRP | Mode::WGRP | Mode::ROTH | Mode::WOTH,
    };
    let fd = rustix::fs::openat(dir, &temporary, create, mode).map_err(|e| fail(e.into()))?;
    let written = match existing {
        Some(st) => rustix::fs::fchmod(&fd, Mode::from_raw_mode(st.st_mode)),
        None => Ok(()),
    }
    .map_err(std::io::Error::from)
    .and_then(|()| {
        let mut file = File::from(fd);
        file.write_all(text.as_bytes())?;
        file.sync_all()
    })
    .and_then(|()| Ok(rustix::fs::renameat(dir, &temporary, dir, target)?));
    written.map_err(|e| {
        let _ = rustix::fs::unlinkat(dir, &temporary, AtFlags::empty());
        fail(e)
    })
}

fn emit(stdout: &mut dyn Write, stderr: &mut dyn Write, (code, text): (i32, String)) -> i32 {
    match stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
    {
        Ok(()) => code,
        Err(e) => {
            let _ = writeln!(stderr, "openwiz: models: cannot write the output: {e}");
            EXIT_READ_FAILED
        }
    }
}
