# Cursor Rules

**DialKey rules index Rev.3** — 2026-09-13

**This folder is the latest.** Common files are copied **from here** to
`D:\Project\project_standards\.cursor\` (kit Rev.3), then to other projects.
Do not refresh DialKey from the kit, and do not let `app_spec_rules.md`
(older essay) override these files.

Public committed rules are **English**. `private_*` may be Japanese (gitignored).
Product behaviour follows `docs/spec.md`.

## Common (`alwaysApply`)

| File | Rev | Role |
|---|---|---|
| `docs-language.mdc` | 3 | Public English / private Japanese; DialKey is latest |
| `docs-layers.mdc` | 1 + DialKey overlay 1 | Spec ⊃ Help; DialKey Help budget |
| `spec-public.mdc` | 2 + DialKey overlay 2 | Public spec vs author notes; public starts at 1.0.0 |
| `spec-authoring.mdc` | 4 | Spec writing: minimum vs full set |
| `git.mdc` | 1 | Conventional Commits; no personal files |

## DialKey-only (`alwaysApply`)

| File | Rev | Role |
|---|---|---|
| `dialkey.mdc` | 1 | Invariants, architecture, i18n, tests |
| `chat-title.mdc` | 1 | `Rev.{version}` → `Rev.{version} ship` after spec, §10, PR |
| `perf-gate.mdc` | 1 | n=10 ready + footprint in spec §10 |
| `private-config.mdc` | 1 | Do not edit `privateConfig` / personal slots |

`private_*` (gitignored): spec diary, release checklist, handoff, Stamp, pre-1.0 archives.
