# Models contract, version 1 (`openwiz.models.v1`)

Spec #1802 (model config: three roles, one command, one page), ticket MD1
(#1803). This contract fixes what every reader and writer of a project's
model config builds against: the one `## Models` table in
`docs/agents/fleet.md`, the follow, fixer and cross-family rules, the
fallback to the older five tables, the offered-models snapshot, the reports
of `openwiz models show`, `list` and `set`, and the exact line the app hands
to the user's agent. [`models.v1.schema.json`](models.v1.schema.json) pins
the JSON shapes; [`fixtures/models/`](fixtures/models/) holds one case per
rule, and every reader runs them and must agree on each role's harness,
model and effort.

Readers: MD2 (#1804, `openwiz models` and the core reader), MD3 (#1805, the
fleet tick), MD4 (#1806, launch-fleet's starts), MD5 (#1807, setup and the
first fleet) and MD6 (#1808, the app's Models page). WizSkills carries a
byte-identical copy of this file and its schema.

Every example below is synthetic, except the model ids and names of
OpenWiz's current choices (J255: builders Opus 5.5 Medium, reviewers Codex
Sol 6.2 Medium; Jake, 2026-11-19, recorded in `docs/agents/fleet.md`).

## Words

| Word | Means |
| --- | --- |
| Role | One of five: Orchestrator, Builder, Reviewer (each always has its own row), Small builder and Fixer (each may follow the Builder) |
| Choice | One harness, model and effort: the `--provider`, `--model` and `--reasoning-level` ids `room-cli thread spawn` takes, with the names people read |
| Family | A provider's model family: `claude-code` is `claude`, `codex` is `gpt`; any other provider's family is unknown |
| Follow | An optional role that follows takes the Builder's choice, whatever it is now |

JSON spells the roles `orchestrator`, `builder`, `reviewer`,
`small_builder` and `fixer`; the command line spells them the same with
`small-builder`; the table and people write `Orchestrator`, `Builder`,
`Reviewer`, `Small builder` and `Fixer`.

## The Models table

One `## Models` section in `docs/agents/fleet.md`. Prose may come before its
table; the section's first table is the Models table and must be exactly:

```md
## Models

| Role | Harness | Model | Effort | `room-cli thread spawn` flags |
| --- | --- | --- | --- | --- |
| Orchestrator | Claude Code | Opus 5.5 | Medium | `--provider claude-code --model claude-opus-5-5 --reasoning-level medium` |
| Builder | Claude Code | Opus 5.5 | Medium | `--provider claude-code --model claude-opus-5-5 --reasoning-level medium` |
| Reviewer | Codex | Sol 6.2 | Medium | `--provider codex --model gpt-6.2-sol --reasoning-level medium` |
| Small builder | follows Builder | | | |
| Fixer | follows Builder | | | |
```

- **Header:** the five column names above, in that order, then `| --- |` for
  each. Cells are read trimmed of spaces.
- **Rows,** in this order: Orchestrator, Builder and Reviewer, all three
  required; then optionally Small builder, then optionally Fixer. No other
  row, no repeat, no other order.
- **A choice row:** Harness, Model and Effort are non-empty names, and the
  flags cell is one code span, exactly
  `--provider <id> --model <model id> --reasoning-level <id>` with single
  spaces. The flags are what spawns;
  the names are what people read. Readers take both as written.
- **Ids, one rule** (the schema's `id` and `model_id`, `set`, and every
  table reader alike): a provider or effort id is letters, digits, `.`, `_`
  or `-`; a **model id** is one or more such segments joined by `/`, with no
  empty segment and no leading or trailing `/` (so `vendor/demo` is one, and
  `vendor//demo` and `vendor/` are not). No spaces, no shell metacharacters.
  Planner's ruling, 2026-11-20: the user picks from what their harnesses
  offer (MC5), so namespaced models are settable.
- **A following row:** the Harness cell is exactly `follows Builder` and the
  other three cells are empty. Only Small builder and Fixer may follow.
- **Refused, never guessed:** a reader refuses a `fleet.md` whose Models
  section breaks any rule above, or that has more than one `## Models`
  heading. It never falls back to the older tables past a broken Models
  table.
- **Writers** (`set`, setup) write all five rows, a following row as
  `| Small builder | follows Builder | | | |`, and change nothing outside
  the rows they set.

**Names `set` writes.** The Harness cell is `Claude Code` for `claude-code`
and `Codex` for `codex`; the Model cell is `Opus 5.5` for
`claude-opus-5-5`, `Sonnet 5.5` for `claude-sonnet-5-5` and `Sol 6.2` for
`gpt-6.2-sol`; any other id is written as the id itself. The Effort cell is
the level with its first letter upper-cased (`medium` is `Medium`, `xhigh`
is `Xhigh`).

## The follow rule

The Small builder and the Fixer each follow the Builder when their row is
absent or follows. The Small builder starts tickets with an exact `polish`,
`huntr` or `proposal` label; the Builder starts every other ticket. The
Orchestrator starts a spec's fleet thread.

Fixtures: `one-table/` (both follow by row), `same-family/` (both absent),
`overridden/` (both have their own choice).

## The fixer rule

The fixer runs the review-loops contract's one fresh fix round
([review-loops.v2](review-loops.v2.md#the-outcomes)) and must be of the
original builder's family. For a pull request whose builder's family is F:

1. the Fixer role (its own choice, or the Builder's it follows), when its
   family is F;
2. otherwise the Reviewer role, when its family is F;
3. otherwise the fix round **waits**, as today. An unknown F waits.

Fixtures: `one-table/` (Claude: the Fixer following the Builder; GPT: the
Reviewer), `overridden/` (Claude: the Fixer's own choice), `same-family/`
(GPT: waits).

## The cross-family rule

A pull request is reviewed by a model of the other family than its builder
(ADR 0008). For a pull request whose builder's family is F:

1. the Reviewer role, when its family is known and not F;
2. otherwise the Builder role, when its family is known and not F;
3. otherwise the review **waits**, as today; it is never swapped for
   another model. An unknown F waits.

The builder's family comes from the ticket's branch-matching
`launch-fleet: start checked` record and that exact thread's provider, as
today. When the Reviewer and the Builder share a known family, `show` and
`set` warn, with exactly (for `Claude`, or `GPT`):

```text
The Reviewer and the Builder are both Claude models, so reviews of Claude-built work wait (ADR 0008).
```

Today's J255 choices give exactly today's reviews: Claude-built work by Codex
Sol 6.2 Medium (the Reviewer), GPT-built work by Opus 5.5 Medium (the
Builder). Fixtures: `one-table/`, `same-family/` (Claude-built waits, with
the warning).

## The older five tables

A `fleet.md` without a `## Models` heading is read from the older tables, as
today: `## Builder`, `## Builder for small tickets`,
`## Reviewer for Claude-built` and `## Reviewer for GPT-built`, each a
`| Setting | Value | \`room-cli thread spawn\` flag |` table of exactly the
Harness, Model and Effort rows, with the flag cells `` `--provider <id>` ``,
`` `--model <model id>` `` and `` `--reasoning-level <id>` ``; and `## Fixer`, a
`| Builder family | Harness | Model | Effort | \`room-cli thread spawn\` flags |`
table with a `Claude` and a `GPT` row. All five must be present and whole,
and the Builder's family known, or the reader refuses.

| Role or rule | Read from the older tables |
| --- | --- |
| Orchestrator | Follows the Builder: the older tables have no Orchestrator |
| Builder | `## Builder` |
| Small builder | `## Builder for small tickets` |
| Reviewer | The reviewer table of the Builder's family (`Reviewer for Claude-built` for a Claude Builder) |
| Fixer | The `## Fixer` row of the Builder's family |
| Review of F-built work | `## Reviewer for F-built`, as written |
| Fix round for an F builder | The `## Fixer` row F, as written |

When both forms are present, readers read the Models table and ignore the
older tables. Fixtures: `five-tables/` (today's J255 tables: every role and
every family's review and fix round read the same as `one-table/`), `both/`
(a stale `Builder for small tickets` loses to the Models table).

**`set` keeps them in step.** While the older tables are present, every
`set` that changes the Models table rewrites each older row to what the
Models table now resolves to:

| Older row | Kept equal to |
| --- | --- |
| `## Builder` | The Builder |
| `## Builder for small tickets` | The Small builder |
| `## Reviewer for Claude-built`, `## Reviewer for GPT-built` | The review of that family's work |
| `## Fixer` rows `Claude`, `GPT` | That family's fix round |

An older row whose resolution waits can't be written: `set` leaves it as it
was and reports it `kept`, beside the same-family warning; until the older
tables go, their readers start that row's old choice. An older row whose new
model id holds a `/` is also left as it was and reported `kept`: today's
older-table readers take no `/` and would refuse the whole file (ruling 14,
accepted by the planner, 2026-11-20).

**A kept row is never silent.** While the older tables are present, each
older row that differs from what the Models table resolves to (a `kept`
row, or one edited by hand) is named in both `show` and the `set` report,
as an `older_table_kept` warning, one per row in the table order above,
after any same-family warning, with exactly this text:

```text
Models table: vendor/demo; older Reviewer for Claude-built table kept at gpt-6.2-sol; fleets start gpt-6.2-sol until launch-fleet reads the Models table (MD4)
```

`Models table: <new>; older <table> table kept at <old>; fleets start <old> until launch-fleet reads the Models table (MD4)`,
where `<table>` is the older row's name as `older_tables` spells it
(`Builder`, `Builder for small tickets`, `Reviewer for Claude-built`,
`Reviewer for GPT-built`, `Fixer, Claude`, `Fixer, GPT`), and the values are
the two model ids, or `waits` for a rule that waits; when the model ids are
equal, the model id and effort; when those are too, the provider, model id
and effort, each joined by a space. Fixtures: `set-older-namespaced/` (a
`set` that writes `vendor/demo` on a five-older-tables project keeps two
rows, and its report and the `show` of the written file both name them),
`both/` (a hand-edited stale row). A `set` on a file with
only the older tables first inserts a `## Models` section, immediately
before the first older table's heading and followed by one blank line, with
all five rows written as choices from the older reading (Small builder and
Fixer as their own rows, not following), then makes the change and keeps the
older rows in step; its report says `models_table_added: true`. Fixture:
`set-older/`.

## The offered-models snapshot

`docs/agents/models-offered.json`, beside `fleet.md`, is a
`snapshot` (`openwiz.models-offered.v1`): what the user's signed-in
harnesses offered at one `cloud status` read, so the app can show real
choices without access to the harnesses.

- `read_at` (milliseconds since the epoch), `label`, `state`, `harnesses`
  and `unavailable` mean what `openwiz models --json`
  ([`new-thread.v1`'s `options`](new-thread.v1.schema.json)) says, without
  its attachment and steer facts. The offered list is copied as that read
  lists it, every provider, model and effort included, with no other
  pattern on the ids.
- **Settable choices.** A choice can be written to the Models table only
  when its ids follow the ids rule above; a namespaced model such as
  `vendor/demo` does, and round-trips through `set` and `show` (fixture
  `set-namespaced/`). An offered choice outside the rule (such as
  `syn-vendor/demo:latest`) stays in the snapshot and in `list`, but `set`
  refuses it as `not_writable` and the app shows it without a Change line.
  Fixture: `list/` lists both; `unwritable/` refuses the second.
- `recommended` has the Orchestrator, Builder and Reviewer, each with its
  `choice`, its `source` and `offered`: whether this read offers that exact
  provider, model and effort. A choice is shown as recommended only when
  `offered` is true.
- **Recommendations** are OpenWiz's current choices: the Builder
  `claude-code claude-opus-5-5 medium` and the Reviewer
  `codex gpt-6.2-sol medium`, each with the source
  `OpenWiz's current choice (J255)`. The Orchestrator's is its current
  choice as read, with the source `your current Orchestrator row`, or
  `your current Builder table (the older tables have no Orchestrator row)`
  from the older tables; when `fleet.md` can't be read its choice is null,
  `offered` false and the source `no fleet.md read`. Changing a
  recommendation is a change to this contract.
- It is written whole by `openwiz models list --save` and by every `set`
  that reads the offered list, and only when the read offered something
  (`state.kind` `offered`); otherwise the file is left as it was.

Fixture: `list/models-offered.json`.

## The reports

Each command reads `docs/agents/fleet.md` under the current directory (run
it from the repository root) and prints one JSON document and a newline
with `--json`, or text lines without it. A handled error with `--json` is
the CLI's existing `openwiz.cli-error.v1` document. Exit numbers keep the
[CLI contract](cli.v1.md#fixed-exits-and-existing-results)'s meanings.

**The CLI rows.** These are the CLI contract's forms `models.show`,
`models.list` and `models.set`, with results `models.v1.schema.json#/$defs/show_report`,
`list_report` and `set_report`; they ship with MD2 (#1804) and are listed
in `help --agent`'s catalogue (`crates/cli/src/agent-catalogue.json`) and
human help. Plain `openwiz models [--json]` is unchanged. `cli.v1.md` is a
contract WizSkills carries byte for byte, so these rows live here until a
WizSkills release carries them; `cli.v1.schema.json` already lists their
effect.

| Form | Effects | Sources |
| --- | --- | --- |
| `models.show` | `source_read` (`docs/agents/fleet.md` only) | none |
| `models.list` | `source_read`; with `--save`, `project_write` (`docs/agents/models-offered.json`) | `room-cli` |
| `models.set` | `source_read`, `project_write` (`docs/agents/fleet.md`, `docs/agents/models-offered.json`) | `room-cli`, except `follows-builder` |

`project_write` is a write to a file of the project checkout under the
current directory, never to OpenWiz state, CloudRoom or GitHub; the agent
commits it. Each form reads one `cloud status` at most, takes options once
with separate values, refuses unknown, repeated or non-UTF-8 arguments, and
reads no stdin. `--help`/`-h` prints the form's help and exits 0.

### `openwiz models show [--json]`

Reads `fleet.md` only: no `cloud status` read, no write. The report
(`show_report`, `openwiz.models-show.v1`) has `read` (`models_table` or
`older_tables`), `older_tables_present` (true only when the Models table was
read and older tables are also present), `roles` (each role's `follows`,
`builder` or null, and its `choice`; a following role's choice is the
Builder's), `reviews` and `fixers` (for each builder family, `from` the
role or older table that serves it, with its choice, or `waits` with a null
choice) and `warnings` (the same-family warning, then one
`older_table_kept` line per older row that differs, or none).

Text, one line per role, then each family's review and fix round, then
each warning:

```text
Orchestrator: Opus 5.5 Medium on Claude Code (claude-code claude-opus-5-5 medium)
Small builder: follows Builder: Opus 5.5 Medium on Claude Code (claude-code claude-opus-5-5 medium)
Review of Claude-built work: Reviewer: Sol 6.2 Medium on Codex (codex gpt-6.2-sol medium)
Fix round for a GPT builder: waits
```

Exits: 0 shown; 1 `fleet.md` missing, unreadable or refused (no model
settings, a broken Models table, broken older tables) or an output failure;
2 usage. Fixtures: every `*/show.json` beside its `fleet.md` (`set-namespaced/show.json` reads its `after.md`).

### `openwiz models list [--save] [--json]`

Today's one `cloud status` read (the same read and the same offered list as
plain `openwiz models`), plus a `fleet.md` read for the Orchestrator's
recommendation. The report (`list_report`, `openwiz.models-list.v1`) is
`offered`, a snapshot as above, and `saved`:
`docs/agents/models-offered.json` when `--save` wrote it, otherwise null.
The text is today's `openwiz models` text followed by one line per
recommended role:

```text
recommended Builder: Opus 5.5 Medium on Claude Code (claude-code claude-opus-5-5 medium), OpenWiz's current choice (J255)
recommended Reviewer: not offered now: Sol 6.2 Medium on Codex (codex gpt-6.2-sol medium)
```

Exits: 0 offered (and saved, with `--save`); 1 the snapshot couldn't be
written, or an output failure; 2 usage; 13 nothing offered (nothing saved).
An unreadable `fleet.md` is not an error here: the Orchestrator has no
recommendation. Fixture: `list/`.

### `openwiz models set <role> <provider> <model> <effort> [--json]`

### `openwiz models set <small-builder|fixer> follows-builder [--json]`

`<role>` is `orchestrator`, `builder`, `reviewer`, `small-builder` or
`fixer`. A choice is checked against one `cloud status` read: the provider,
model and effort must be offered together, and follow the ids rule (the
snapshot's settable choices). Then `set` rewrites that role's
row (inserting the Models table first when only the older tables exist),
keeps the older tables in step, writes the snapshot, and prints what
changed, so the agent can commit it. `follows-builder` makes no read and
writes no snapshot. Setting the value a role already has is `unchanged`:
the file is untouched, the snapshot still written.

The report (`set_report`, `openwiz.models-set.v1`) has `outcome`
(`changed`, `unchanged`, `refused` or `no_options`), `role`, `asked`,
`before` and `after` (the role as read before and after; `after` is null
unless changed or unchanged), `models_table_added`, `older_tables` (each
older row as `changed`, `unchanged` or `kept`, in the table order above;
empty when there are none), `files` (the paths written), `warnings`,
`refusal` and `offered` (on `refused`, the harnesses the read offered).

- **Refused, not offered** (`refused`, exit 5): nothing is written; the
  refusal text is exactly
  `not offered: <provider> <model> <effort>; nothing was changed`.
- **Refused, not writable** (`refused`, exit 5): the choice is offered but
  its ids break the ids rule; nothing is written; the refusal
  reason is `not_writable` and its text exactly
  `cannot be written to the Models table: <provider> <model> <effort>; nothing was changed`.
- **Nothing offered** (`no_options`, exit 13): nothing is written; the
  refusal text is the read's own no-options text.
- **Same family:** the change is made, exit 0, with the warning.
- **Kept older rows:** the change is made, exit 0; each older row left
  differing from the Models table is `kept` in `older_tables` and named by
  its `older_table_kept` warning, as `show` names it.

Text: `<Role>: <before> -> <after>` (each as in `show`), one
`<older row>: changed|kept` line per older row not unchanged, each
warning's text (a kept row's line included), then `wrote <path>` per file; or the refusal text and the offered list.
Exits: 0 changed or unchanged; 1 `fleet.md` missing, unreadable or refused,
a failed write, or an output failure; 2 usage (an unknown role, a wrong
number of operands, `follows-builder` for a role that can't follow, any
option but `--json`); 5 not offered or not writable; 13 nothing offered. Fixtures:
`set-older/`, `set-same-family/`, `set-namespaced/`,
`set-older-namespaced/`, `unoffered/`, `unwritable/` (each `case.json` names the
starting `fleet.md`, the arguments, the exit and the expected written
files).

## The hand-off line

The app's Change button shows and copies exactly one line for the user's
agent. For a choice:

```text
Set the builder to Opus 5.5 Medium on Claude Code: run `openwiz models set builder claude-code claude-opus-5-5 medium` and commit the change.
```

`Set the <role> to <Model> <Effort> on <Harness>: run \`openwiz models set <role token> <provider> <model> <effort>\` and commit the change.`,
where `<role>` is the role in lower case (`small builder`), `<role token>`
its command-line spelling, and the names are the ones `set` writes. For
following:

```text
Set the fixer to follow the builder: run `openwiz models set fixer follows-builder` and commit the change.
```

The app offers only the snapshot's settable choices. Fixture: `handoff.json`.

## Taking effect

Readers read `fleet.md` from the default branch at each agent start, so a
merged change reaches the next agent started; agents already running keep
their model. Nothing restarts.

## Gaps

- The fixtures prove that the contract's cases agree with its rules and its
  schema, read by the reference reader in
  [`fixtures/models/models_contract.rs`](fixtures/models/models_contract.rs);
  they prove no reader. Each reader's own tests must run them.
- The text forms are pinned by the lines above; only the JSON reports have
  fixtures.
- No live read is in this contract: the snapshot is only as fresh as its
  `read_at`, and CloudRoom may answer from a saved copy.
