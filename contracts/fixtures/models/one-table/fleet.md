# Fleet settings

Synthetic fixture for the models contract (`openwiz.models.v1`).

## Models

| Role | Harness | Model | Effort | `room-cli thread spawn` flags |
| --- | --- | --- | --- | --- |
| Orchestrator | Claude Code | Opus 5.5 | Medium | `--provider claude-code --model claude-opus-5-5 --reasoning-level medium` |
| Builder | Claude Code | Opus 5.5 | Medium | `--provider claude-code --model claude-opus-5-5 --reasoning-level medium` |
| Reviewer | Codex | Sol 6.2 | Medium | `--provider codex --model gpt-6.2-sol --reasoning-level medium` |
| Small builder | follows Builder | | | |
| Fixer | follows Builder | | | |

## Loop

Not a model table: readers skip it.
