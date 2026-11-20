# Fleet settings

Synthetic fixture for the models contract (`openwiz.models.v1`).

## Models

| Role | Harness | Model | Effort | `room-cli thread spawn` flags |
| --- | --- | --- | --- | --- |
| Orchestrator | Codex | Sol 6.2 | High | `--provider codex --model gpt-6.2-sol --reasoning-level high` |
| Builder | Claude Code | Opus 5.5 | Medium | `--provider claude-code --model claude-opus-5-5 --reasoning-level medium` |
| Reviewer | Codex | Sol 6.2 | Medium | `--provider codex --model gpt-6.2-sol --reasoning-level medium` |
| Small builder | Claude Code | Sonnet 5.5 | Low | `--provider claude-code --model claude-sonnet-5-5 --reasoning-level low` |
| Fixer | Claude Code | Opus 5.5 | High | `--provider claude-code --model claude-opus-5-5 --reasoning-level high` |

## Loop

Not a model table: readers skip it.
