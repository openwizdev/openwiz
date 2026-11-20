# Fleet settings

Synthetic fixture for the models contract (`openwiz.models.v1`).

## Models

| Role | Harness | Model | Effort | `room-cli thread spawn` flags |
| --- | --- | --- | --- | --- |
| Orchestrator | Claude Code | Opus 5.5 | Medium | `--provider claude-code --model claude-opus-5-5 --reasoning-level medium` |
| Builder | Claude Code | Opus 5.5 | High | `--provider claude-code --model claude-opus-5-5 --reasoning-level high` |
| Reviewer | Codex | Sol 6.2 | Medium | `--provider codex --model gpt-6.2-sol --reasoning-level medium` |
| Small builder | Claude Code | Opus 5.5 | Medium | `--provider claude-code --model claude-opus-5-5 --reasoning-level medium` |
| Fixer | Claude Code | Opus 5.5 | Medium | `--provider claude-code --model claude-opus-5-5 --reasoning-level medium` |

## Builder

One per ticket, on its own branch.

| Setting | Value | `room-cli thread spawn` flag |
| --- | --- | --- |
| Harness | Claude Code | `--provider claude-code` |
| Model | Opus 5.5 | `--model claude-opus-5-5` |
| Effort | High | `--reasoning-level high` |

## Builder for small tickets

| Setting | Value | `room-cli thread spawn` flag |
| --- | --- | --- |
| Harness | Claude Code | `--provider claude-code` |
| Model | Opus 5.5 | `--model claude-opus-5-5` |
| Effort | Medium | `--reasoning-level medium` |

## Reviewer for Claude-built

| Setting | Value | `room-cli thread spawn` flag |
| --- | --- | --- |
| Harness | Codex | `--provider codex` |
| Model | Sol 6.2 | `--model gpt-6.2-sol` |
| Effort | Medium | `--reasoning-level medium` |

## Reviewer for GPT-built

| Setting | Value | `room-cli thread spawn` flag |
| --- | --- | --- |
| Harness | Claude Code | `--provider claude-code` |
| Model | Opus 5.5 | `--model claude-opus-5-5` |
| Effort | High | `--reasoning-level high` |

## Fixer

| Builder family | Harness | Model | Effort | `room-cli thread spawn` flags |
| --- | --- | --- | --- | --- |
| Claude | Claude Code | Opus 5.5 | Medium | `--provider claude-code --model claude-opus-5-5 --reasoning-level medium` |
| GPT | Codex | Sol 6.2 | Medium | `--provider codex --model gpt-6.2-sol --reasoning-level medium` |
## Loop

Not a model table: readers skip it.
