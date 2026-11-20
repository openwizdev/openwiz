//! #1803 (MD1): the models contract's fixtures agree with its schema and its
//! rules. `contracts/models.v1.md` is the contract; this file is registered
//! in `tests/cli-agent/main.rs` and kept beside the fixtures it checks, so a
//! later reader (MD2's CLI, MD4's launch-fleet, MD5's setup, MD6's app) has
//! one place to compare itself against. All values are synthetic.
//!
//! The reader below is the contract's reference reading of a `fleet.md`:
//! the Models table, or the older five tables when it is absent, then the
//! follow, fixer and cross-family rules. It ships nowhere.
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::ROOT;
use super::schema::{lint, validate};

fn dir() -> PathBuf {
    Path::new(ROOT).join("contracts/fixtures/models")
}

fn load(rel: &str) -> Value {
    let path = dir().join(rel);
    serde_json::from_str(&fs::read_to_string(&path).unwrap())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn text(rel: &str) -> String {
    fs::read_to_string(dir().join(rel)).unwrap()
}

fn schema() -> Value {
    let path = Path::new(ROOT).join("contracts/models.v1.schema.json");
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

/// The schema's definition for a document, by its `schema` id.
fn definition(doc: &Value) -> &'static str {
    match doc["schema"].as_str() {
        Some("openwiz.models-show.v1") => "show_report",
        Some("openwiz.models-list.v1") => "list_report",
        Some("openwiz.models-set.v1") => "set_report",
        Some("openwiz.models-offered.v1") => "snapshot",
        Some("openwiz.models-handoff.v1") => "handoff_fixture",
        other => panic!("no definition for schema {other:?}"),
    }
}

fn check(schema: &Value, doc: &Value) -> Vec<String> {
    let mut one = schema.clone();
    let name = definition(doc);
    one["$ref"] = json!(format!("#/$defs/{name}"));
    one.as_object_mut().unwrap().remove("oneOf");
    validate(&one, doc)
}

/// Every JSON fixture outside `invalid/`, except the case files.
fn documents() -> Vec<String> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir()).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        if path.is_dir() && name != "invalid" {
            for file in fs::read_dir(&path).unwrap() {
                let file = file.unwrap().file_name().into_string().unwrap();
                if file.ends_with(".json") && file != "case.json" {
                    out.push(format!("{name}/{file}"));
                }
            }
        } else if name.ends_with(".json") {
            out.push(name);
        }
    }
    out.sort();
    out
}

// catches: a schema that lets a malformed report or snapshot through (a
// waiting review that names a model, a builder that follows itself, a
// refusal that changed files, a recommended Fixer, a snapshot with no read
// time), or one that refuses a report the contract's fixtures print.
#[test]
fn the_schema_takes_every_report_fixture_and_refuses_each_malformed_one() {
    let schema = schema();
    assert_eq!(
        lint(&schema),
        Vec::<String>::new(),
        "unchecked schema rules"
    );
    let documents = documents();
    assert!(documents.len() >= 11, "{documents:?}");
    for rel in &documents {
        let doc = load(rel);
        assert_eq!(check(&schema, &doc), Vec::<String>::new(), "{rel}");
        assert_eq!(validate(&schema, &doc), Vec::<String>::new(), "{rel}");
    }
    let mut invalid: Vec<_> = fs::read_dir(dir().join("invalid"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    invalid.sort();
    assert!(invalid.len() >= 10, "{invalid:?}");
    for name in invalid {
        let doc = load(&format!("invalid/{name}"));
        assert!(
            !check(&schema, &doc).is_empty(),
            "invalid/{name} passed its definition"
        );
        assert!(
            !validate(&schema, &doc).is_empty(),
            "invalid/{name} passed the schema"
        );
    }
}

// ---- The reference reading of a fleet.md --------------------------------

const MODELS_HEADER: [&str; 5] = [
    "Role",
    "Harness",
    "Model",
    "Effort",
    "`room-cli thread spawn` flags",
];
const OLDER: [&str; 5] = [
    "Builder",
    "Builder for small tickets",
    "Reviewer for Claude-built",
    "Reviewer for GPT-built",
    "Fixer",
];

fn cells(line: &str) -> Option<Vec<String>> {
    let inner = line.strip_prefix('|')?.strip_suffix('|')?;
    Some(inner.split('|').map(|c| c.trim().to_owned()).collect())
}

/// The lines under exactly one `## <name>` heading, to the next `## `.
fn section<'a>(text: &'a str, name: &str) -> Result<Option<Vec<&'a str>>, String> {
    let lines: Vec<&str> = text.lines().collect();
    let heading = format!("## {name}");
    let at: Vec<usize> = (0..lines.len()).filter(|&i| lines[i] == heading).collect();
    match at[..] {
        [] => Ok(None),
        [i] => {
            let end = (i + 1..lines.len())
                .find(|&j| lines[j].starts_with("## "))
                .unwrap_or(lines.len());
            Ok(Some(lines[i + 1..end].to_vec()))
        }
        _ => Err(format!("more than one {heading}")),
    }
}

fn id(word: &str) -> Result<String, String> {
    if !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
    {
        Ok(word.to_owned())
    } else {
        Err(format!("bad id {word:?}"))
    }
}

/// A model id: one or more id segments joined by `/` (no empty segment, no
/// leading or trailing `/`), so `vendor/demo` is one.
fn model_id(word: &str) -> Result<String, String> {
    for segment in word.split('/') {
        id(segment).map_err(|_| format!("bad model id {word:?}"))?;
    }
    Ok(word.to_owned())
}

fn family(provider: &str) -> Value {
    match provider {
        "claude-code" => json!("claude"),
        "codex" => json!("gpt"),
        _ => Value::Null,
    }
}

fn choice(names: [&str; 3], ids: [String; 3]) -> Result<Value, String> {
    if names.iter().any(|n| n.is_empty()) {
        return Err("an empty name cell".into());
    }
    Ok(json!({
        "provider": ids[0], "model": ids[1], "effort": ids[2], "family": family(&ids[0]),
        "names": {"harness": names[0], "model": names[1], "effort": names[2]},
    }))
}

/// `--provider P --model M --reasoning-level E`, in one code span.
fn flags(cell: &str) -> Result<[String; 3], String> {
    let inner = cell
        .strip_prefix('`')
        .and_then(|c| c.strip_suffix('`'))
        .ok_or("flags not in one code span")?;
    match inner.split(' ').collect::<Vec<_>>()[..] {
        ["--provider", p, "--model", m, "--reasoning-level", e] => {
            Ok([id(p)?, model_id(m)?, id(e)?])
        }
        _ => Err(format!("bad flags {inner:?}")),
    }
}

struct Read {
    form: &'static str,
    roles: Vec<(&'static str, Value, Value)>,
    reviews: Option<[Value; 2]>,
    fixers: Option<[Value; 2]>,
}

fn models_table(lines: &[&str]) -> Result<Vec<(&'static str, Value, Value)>, String> {
    let start = lines
        .iter()
        .position(|l| l.starts_with('|'))
        .ok_or("no table")?;
    let rows: Vec<Vec<String>> = lines[start..]
        .iter()
        .take_while(|l| l.starts_with('|'))
        .map(|l| cells(l).ok_or("not a table row"))
        .collect::<Result<_, _>>()?;
    if rows.len() < 2 || rows[0] != MODELS_HEADER || rows[1] != ["---"; 5] {
        return Err("bad Models header".into());
    }
    let order = [
        ("Orchestrator", "orchestrator"),
        ("Builder", "builder"),
        ("Reviewer", "reviewer"),
        ("Small builder", "small_builder"),
        ("Fixer", "fixer"),
    ];
    let mut out = Vec::new();
    let mut next = 0;
    for row in &rows[2..] {
        if row.len() != 5 {
            return Err("a Models row without five cells".into());
        }
        let at = order[next..]
            .iter()
            .position(|(r, _)| *r == row[0])
            .ok_or_else(|| format!("row {:?} out of place", row[0]))?
            + next;
        if at > next && next < 3 {
            return Err("a required row is missing".into());
        }
        let key = order[at].1;
        if row[1] == "follows Builder" && row[2..].iter().all(String::is_empty) {
            if at < 3 {
                return Err(format!("{} cannot follow", row[0]));
            }
            out.push((key, json!("builder"), Value::Null));
        } else {
            let c = choice([&row[1], &row[2], &row[3]], flags(&row[4])?)?;
            out.push((key, Value::Null, c));
        }
        next = at + 1;
    }
    if next < 3 {
        return Err("a required row is missing".into());
    }
    let builder = out[1].2.clone();
    for (_, key) in &order[3..] {
        if !out.iter().any(|(k, _, _)| k == key) {
            out.push((key, json!("builder"), Value::Null));
        }
    }
    for role in &mut out {
        if role.1 == "builder" {
            role.2 = builder.clone();
        }
    }
    Ok(out)
}

fn setting_table(lines: &[&str]) -> Result<Value, String> {
    let rows: Vec<Vec<String>> = lines
        .iter()
        .filter(|l| l.starts_with('|'))
        .map(|l| cells(l).ok_or("not a table row"))
        .collect::<Result<_, _>>()?;
    let mut names = Vec::new();
    let mut ids = Vec::new();
    for (i, (setting, flag)) in [
        ("Harness", "--provider"),
        ("Model", "--model"),
        ("Effort", "--reasoning-level"),
    ]
    .iter()
    .enumerate()
    {
        let row = rows.get(i + 2).ok_or("a short table")?;
        let value = row
            .get(2)
            .and_then(|c| c.strip_prefix(&format!("`{flag} ")))
            .and_then(|c| c.strip_suffix('`'))
            .filter(|_| row.len() == 3 && row[0] == *setting)
            .ok_or_else(|| format!("bad {setting} row"))?;
        names.push(row[1].clone());
        ids.push(if i == 1 { model_id(value)? } else { id(value)? });
    }
    choice([&names[0], &names[1], &names[2]], ids.try_into().unwrap())
}

fn older_tables(text: &str) -> Result<Read, String> {
    let mut tables = Vec::new();
    for name in &OLDER[..4] {
        let lines = section(text, name)?.ok_or_else(|| format!("no ## {name}"))?;
        tables.push(setting_table(&lines)?);
    }
    let fixer = section(text, "Fixer")?.ok_or("no ## Fixer")?;
    let mut fixers = Vec::new();
    for fam in ["Claude", "GPT"] {
        let rows: Vec<Vec<String>> = fixer.iter().filter_map(|l| cells(l)).collect();
        let row = rows
            .iter()
            .find(|r| r.len() == 5 && r[0] == fam)
            .ok_or_else(|| format!("no Fixer row {fam}"))?;
        fixers.push(choice([&row[1], &row[2], &row[3]], flags(&row[4])?)?);
    }
    let builder = tables[0].clone();
    let reviewer = match builder["family"].as_str() {
        Some("claude") => tables[2].clone(),
        Some("gpt") => tables[3].clone(),
        _ => return Err("older Builder of no known family".into()),
    };
    let fixer = if builder["family"] == "claude" {
        fixers[0].clone()
    } else {
        fixers[1].clone()
    };
    let from = |c: &Value| json!({"from": "older_table", "choice": c});
    Ok(Read {
        form: "older_tables",
        roles: vec![
            ("orchestrator", json!("builder"), builder.clone()),
            ("builder", Value::Null, builder),
            ("reviewer", Value::Null, reviewer),
            ("small_builder", Value::Null, tables[1].clone()),
            ("fixer", Value::Null, fixer),
        ],
        reviews: Some([from(&tables[2]), from(&tables[3])]),
        fixers: Some([from(&fixers[0]), from(&fixers[1])]),
    })
}

/// An older-tables reading in `show`'s shape, for `older_rows`.
fn older_view(older: &Read) -> Value {
    let (reviews, fixers) = (
        older.reviews.as_ref().unwrap(),
        older.fixers.as_ref().unwrap(),
    );
    json!({
        "roles": {"builder": {"choice": older.roles[1].2}, "small_builder": {"choice": older.roles[3].2}},
        "reviews": {"claude": reviews[0], "gpt": reviews[1]},
        "fixers": {"claude": fixers[0], "gpt": fixers[1]},
    })
}

/// The kept line's two values: the model ids, or `waits`; when the model ids
/// are equal, the model and effort; when those are too, all three ids.
fn values(now: &Value, was: &Value) -> (String, String) {
    let shown = |c: &Value, keys: &[&str]| {
        if c.is_null() {
            "waits".to_owned()
        } else {
            keys.iter()
                .map(|k| c[k].as_str().unwrap())
                .collect::<Vec<_>>()
                .join(" ")
        }
    };
    for keys in [
        &["model"][..],
        &["model", "effort"],
        &["provider", "model", "effort"],
    ] {
        let (a, b) = (shown(now, keys), shown(was, keys));
        if a != b {
            return (a, b);
        }
    }
    unreachable!("the two choices differ")
}

fn read(text: &str) -> Result<Value, String> {
    let older_present = OLDER
        .iter()
        .any(|name| text.lines().any(|l| l == format!("## {name}")));
    let read = match section(text, "Models")? {
        Some(lines) => Read {
            form: "models_table",
            roles: models_table(&lines)?,
            reviews: None,
            fixers: None,
        },
        None if older_present => older_tables(text)?,
        None => return Err("no model settings".into()),
    };
    let role = |key: &str| read.roles.iter().find(|r| r.0 == key).unwrap().2.clone();
    let (builder, reviewer, fixer) = (role("builder"), role("reviewer"), role("fixer"));
    let mut reviews = serde_json::Map::new();
    let mut fixers = serde_json::Map::new();
    for (i, fam) in ["claude", "gpt"].into_iter().enumerate() {
        let pick = |from: &str, c: &Value| json!({"from": from, "choice": c});
        let other = |c: &Value| !c["family"].is_null() && c["family"] != fam;
        let review = match &read.reviews {
            Some(older) => older[i].clone(),
            None if other(&reviewer) => pick("reviewer", &reviewer),
            None if other(&builder) => pick("builder", &builder),
            None => pick("waits", &Value::Null),
        };
        let fix = match &read.fixers {
            Some(older) => older[i].clone(),
            None if fixer["family"] == fam => pick("fixer", &fixer),
            None if reviewer["family"] == fam => pick("reviewer", &reviewer),
            None => pick("waits", &Value::Null),
        };
        reviews.insert(fam.into(), review);
        fixers.insert(fam.into(), fix);
    }
    let mut warnings = Vec::new();
    if let Some(fam) = reviewer["family"].as_str()
        && reviewer["family"] == builder["family"]
    {
        let shown = if fam == "claude" { "Claude" } else { "GPT" };
        warnings.push(json!({
            "kind": "same_family", "family": fam,
            "text": format!("The Reviewer and the Builder are both {shown} models, so reviews of {shown}-built work wait (ADR 0008)."),
        }));
    }
    let resolved = json!({
        "roles": {"builder": {"choice": builder}, "small_builder": {"choice": role("small_builder")}},
        "reviews": reviews.clone(), "fixers": fixers.clone(),
    });
    if read.form == "models_table"
        && older_present
        && let Ok(older) = older_tables(text)
    {
        let kept = older_rows(&older_view(&older));
        for (i, (table, now)) in older_rows(&resolved).into_iter().enumerate() {
            let was = &kept[i].1;
            if &now != was {
                let (new, old) = values(&now, was);
                warnings.push(json!({
                    "kind": "older_table_kept", "table": table, "models_table": new, "older": old,
                    "text": format!("Models table: {new}; older {table} table kept at {old}; fleets start {old} until launch-fleet reads the Models table (MD4)"),
                }));
            }
        }
    }
    let roles: serde_json::Map<String, Value> = read
        .roles
        .iter()
        .map(|(k, f, c)| (k.to_string(), json!({"follows": f, "choice": c})))
        .collect();
    Ok(json!({
        "schema": "openwiz.models-show.v1", "file": "docs/agents/fleet.md",
        "read": read.form, "older_tables_present": read.form == "models_table" && older_present,
        "roles": roles, "reviews": reviews, "fixers": fixers, "warnings": warnings,
    }))
}

/// Harness, model and effort only: what spawns.
fn spawned(c: &Value) -> Value {
    json!([c["provider"], c["model"], c["effort"]])
}

// catches: an older-tables fallback, or a one-table follow, fixer or
// cross-family rule, that starts any role or any builder family's reviewer
// or fixer on a different harness, model or effort than today's J255 tables.
#[test]
fn todays_choices_read_the_same_from_the_one_table_and_the_five_tables() {
    let one = read(&text("one-table/fleet.md")).unwrap();
    let five = read(&text("five-tables/fleet.md")).unwrap();
    assert_eq!(
        (one["read"].as_str(), five["read"].as_str()),
        (Some("models_table"), Some("older_tables"))
    );
    for role in [
        "orchestrator",
        "builder",
        "reviewer",
        "small_builder",
        "fixer",
    ] {
        assert_eq!(
            one["roles"][role]["choice"], five["roles"][role]["choice"],
            "{role}"
        );
    }
    for fam in ["claude", "gpt"] {
        for kind in ["reviews", "fixers"] {
            let (a, b) = (&one[kind][fam]["choice"], &five[kind][fam]["choice"]);
            assert!(a.is_object(), "{kind} of {fam}-built work waits: {one}");
            assert_eq!(spawned(a), spawned(b), "{kind} of {fam}-built work");
        }
    }
    // J255, as the contract records it: Claude-built work is reviewed by
    // Codex Sol 6.2 Medium and GPT-built work by Opus 5.5 Medium.
    assert_eq!(
        spawned(&one["reviews"]["claude"]["choice"]),
        json!(["codex", "gpt-6.2-sol", "medium"])
    );
    assert_eq!(
        spawned(&one["reviews"]["gpt"]["choice"]),
        json!(["claude-code", "claude-opus-5-5", "medium"])
    );
}

// catches: a show fixture that disagrees with the contract's rules (an
// absent or following row that doesn't follow the Builder, a stale older
// table winning over the Models table, a same-family pair with no warning or
// no waiting review), so every later reader would agree on a wrong answer;
// or a namespaced model id (`vendor/demo`) that `set` writes but `show`
// can't read back.
#[test]
fn every_show_fixture_is_its_fleet_files_reading() {
    for (file, show) in [
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
        let got = read(&text(file));
        assert!(got.is_ok(), "{file} refused: {got:?}");
        assert_eq!(got.unwrap(), load(show), "{file}");
    }
}

// catches: a malformed Models table that some reader would still take
// (a missing or misplaced required row, a following Builder, broken flags),
// so two readers start different models from the same file.
#[test]
fn a_malformed_models_table_is_refused_not_guessed() {
    let good = text("one-table/fleet.md");
    let breaks = [
        ("| Reviewer | Codex", "| Reviewers | Codex"),
        (
            "| Builder | Claude Code | Opus 5.5 | Medium | `--provider claude-code --model claude-opus-5-5 --reasoning-level medium` |",
            "| Builder | follows Builder | | | |",
        ),
        (
            "--reasoning-level medium` |\n| Reviewer",
            "--effort medium` |\n| Reviewer",
        ),
        (
            "| Small builder | follows Builder | | | |\n| Fixer",
            "| Fixer | follows Builder | | | |\n| Small builder",
        ),
        ("`room-cli thread spawn` flags |", "flags |"),
    ];
    for (from, to) in breaks {
        assert!(good.contains(from), "{from}");
        let broken = good.replacen(from, to, 1);
        assert!(read(&broken).is_err(), "taken: {to}");
    }
}

/// The older rows a Models table resolves to, as the contract's `set` keeps
/// them while both forms are present.
fn older_rows(show: &Value) -> Vec<(&'static str, Value)> {
    vec![
        ("Builder", show["roles"]["builder"]["choice"].clone()),
        (
            "Builder for small tickets",
            show["roles"]["small_builder"]["choice"].clone(),
        ),
        (
            "Reviewer for Claude-built",
            show["reviews"]["claude"]["choice"].clone(),
        ),
        (
            "Reviewer for GPT-built",
            show["reviews"]["gpt"]["choice"].clone(),
        ),
        ("Fixer, Claude", show["fixers"]["claude"]["choice"].clone()),
        ("Fixer, GPT", show["fixers"]["gpt"]["choice"].clone()),
    ]
}

// catches: a set fixture whose written file reads as something other than
// the report says, leaves the older tables out of step with the Models
// table, changes another role, a not-offered refusal that names an offered
// choice, or a choice set that the Models table can't hold (R1: an offered
// namespaced model is listed, but refused as not writable).
#[test]
fn every_set_fixture_writes_what_its_report_says() {
    for case in [
        "set-older",
        "set-same-family",
        "set-namespaced",
        "set-older-namespaced",
        "unoffered",
        "unwritable",
    ] {
        let spec = load(&format!("{case}/case.json"));
        let report = load(&format!("{case}/set.json"));
        let before = read(&text(spec["fleet"].as_str().unwrap())).unwrap();
        let role = report["role"].as_str().unwrap();
        assert_eq!(report["before"], before["roles"][role], "{case}");
        let offered = load("list/models-offered.json");
        let asked = &report["asked"];
        let is_offered = offered["harnesses"].as_array().unwrap().iter().any(|h| {
            h["provider"] == asked["provider"]
                && h["models"].as_array().unwrap().iter().any(|m| {
                    m["model"] == asked["model"]
                        && m["efforts"].as_array().unwrap().contains(&asked["effort"])
                })
        });
        let writable = id(asked["provider"].as_str().unwrap()).is_ok()
            && model_id(asked["model"].as_str().unwrap()).is_ok()
            && id(asked["effort"].as_str().unwrap()).is_ok();
        let fleet = spec["writes"]["docs/agents/fleet.md"].as_str();
        if report["outcome"] == "refused" {
            let why = if is_offered {
                "not_writable"
            } else {
                "not_offered"
            };
            assert_eq!(report["refusal"]["reason"], why, "{case}");
            assert!(fleet.is_none() && (!is_offered || !writable), "{case}");
            assert_eq!(report["offered"], offered["harnesses"], "{case}");
            assert_eq!(spec["exit"], 5, "{case}");
            continue;
        }
        assert!(is_offered && writable, "{case}");
        let after = read(&text(fleet.unwrap())).unwrap();
        assert_eq!(after["read"], "models_table", "{case}");
        assert_eq!(report["after"], after["roles"][role], "{case}");
        assert_eq!(
            spawned(&report["after"]["choice"]),
            spawned(asked),
            "{case}"
        );
        assert_eq!(report["warnings"], after["warnings"], "{case}");
        for other in [
            "orchestrator",
            "builder",
            "reviewer",
            "small_builder",
            "fixer",
        ] {
            if other != role && after["roles"][other]["follows"].is_null() {
                assert_eq!(
                    after["roles"][other]["choice"], before["roles"][other]["choice"],
                    "{case}: {other}"
                );
            }
        }
        assert_eq!(
            after["older_tables_present"],
            report["models_table_added"].as_bool().unwrap()
                || before["older_tables_present"] == true
        );
        if after["older_tables_present"] == true {
            let older = older_view(&older_tables(&text(fleet.unwrap())).unwrap());
            let rows = older_rows(&before);
            for (i, (table, want)) in older_rows(&after).into_iter().enumerate() {
                let in_file = &older_rows(&older)[i].1;
                let outcome = if in_file == &want {
                    if rows[i].1 != want {
                        "changed"
                    } else {
                        "unchanged"
                    }
                } else {
                    // Kept (ruling 14): left as it was; the warning names it.
                    assert_eq!(
                        in_file, &rows[i].1,
                        "{case}: {table} neither in step nor kept"
                    );
                    "kept"
                };
                assert_eq!(
                    report["older_tables"][i],
                    json!({"table": table, "outcome": outcome}),
                    "{case}"
                );
            }
        } else {
            assert_eq!(report["older_tables"], json!([]), "{case}");
        }
    }
}

// catches: a hand-off line whose command sets another role or choice than
// the line names, or names a choice the snapshot never offered.
#[test]
fn every_handoff_line_runs_the_set_it_names() {
    let offered = load("list/models-offered.json");
    for entry in load("handoff.json")["lines"].as_array().unwrap() {
        let line = entry["line"].as_str().unwrap();
        let command = line.split('`').nth(1).unwrap();
        let words: Vec<&str> = command.split(' ').collect();
        let token = entry["role"].as_str().unwrap().replace('_', "-");
        assert_eq!(
            words[..4],
            ["openwiz", "models", "set", token.as_str()],
            "{line}"
        );
        assert!(line.ends_with("` and commit the change."), "{line}");
        if entry["to"].is_null() {
            assert_eq!(words[4..], ["follows-builder"], "{line}");
            continue;
        }
        let to = &entry["to"];
        assert_eq!(
            words[4..],
            [
                to["provider"].as_str().unwrap(),
                to["model"].as_str().unwrap(),
                to["effort"].as_str().unwrap()
            ],
            "{line}"
        );
        assert!(
            offered["harnesses"]
                .as_array()
                .unwrap()
                .iter()
                .any(|h| h["provider"] == to["provider"]
                    && h["models"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|m| m["model"] == to["model"]
                            && m["efforts"].as_array().unwrap().contains(&to["effort"]))),
            "{line}"
        );
    }
}
