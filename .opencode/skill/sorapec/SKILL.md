---
name: sorapec
description: Spec-driven development workflow using Sorapec CLI (runs, evidence, re-entry)
---

# Sorapec (Runtime Orchestration)

## agentNix (Nix flake)

If `sorapec` is not available in `PATH`, run it via agentNix:

```bash
# Inside the agentNix repo
nix run .#sorapec -- <args>

# From anywhere
nix run github:Kh05ifr4nD/agentNix#sorapec -- <args>
```

Sorapec runs are a strict, interruption-safe workflow. Treat Sorapec as the control plane:

- State lives under `.sorapec/.runs/<run-id>/`.
- Progress is derived from filesystem evidence + artifact DAG.
- Re-entry is deterministic (auto-observe + auto-rewind for invariant violations).

## Quick Reference

| Command | Purpose |
|---------|---------|
| `sorapec run init <change-id> --schema <schema> --json` | Initialize a run and print run id |
| `sorapec run status <run-id> --json` | Show derived status + allowed actions |
| `sorapec run observe <run-id> --json` | Record completion observation (and optional input progress) |
| `sorapec run approve <run-id>` | Approve plan (enables dispatch when approval gate is on) |
| `sorapec run veto <run-id>` | Deny current plan version (clears in-flight) |
| `sorapec run dispatch <run-id> --auto` | Dispatch up to capacity from ready set |
| `sorapec run dispatch <run-id> --artifact <id>` | Dispatch specific ready artifact(s) |
| `sorapec run rewind <run-id>` | Fault-rewind (clears volatile in-flight) |
| `sorapec run abort <run-id>` | Abort run (rewind + release lease) |
| `sorapec run finish <run-id>` | Finish run (requires clean workspace; releases lease) |
| `sorapec run plan update <run-id> --goal <artifact-id>` | Update plan goals (dependency-closed) |

## How to Use This Skill

1. Run `sorapec run init <change-id> --schema spec-driven --json` and keep the `run_id` in the conversation.
1. Before any action, run `sorapec run status <run-id> --json` and only do actions listed in `allowed_actions`.
1. Only work on artifacts that are currently in `in_flight`.
1. After completing an artifact (files exist at expected paths), run `sorapec run observe <run-id>` to record evidence and refresh derived sets.

## TLA+ Kernel Invariants (CLI MUST preserve)

- `in_flight ∩ completed = ∅`
- `in_flight ⊆ plan.goals`
- `|in_flight| ≤ max_parallel`
- `¬dispatch_allowed ⇒ in_flight = ∅`
- `plan.goals` must be dependency-closed

## Status Check Snippet

```bash
sorapec run status <run-id> --json
```
