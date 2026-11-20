# Fleet settings

Synthetic fixture for the models contract (`openwiz.models.v1`).

## Models

The Small builder and Fixer rows are absent, so both follow the Builder.

| Role | Harness | Model | Effort | `room-cli thread spawn` flags |
| --- | --- | --- | --- | --- |
| Orchestrator | Claude Code | Opus 5.5 | Medium | `--provider claude-code --model claude-opus-5-5 --reasoning-level medium` |
| Builder | Claude Code | Opus 5.5 | Medium | `--provider claude-code --model claude-opus-5-5 --reasoning-level medium` |
| Reviewer | Claude Code | Opus 5.5 | High | `--provider claude-code --model claude-opus-5-5 --reasoning-level high` |

## Loop

Not a model table: readers skip it.
