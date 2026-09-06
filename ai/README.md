# Source material

Four independent design plans for a terminal CMS, produced by different AI
agents. They are kept unedited as the provenance of the project.

**These are not the current design.** The synthesis lives in `docs/` and
`specs/`; where these documents disagreed, the resolution is recorded as an ADR
and summarised in [`docs/roadmap.md`](../docs/roadmap.md) §4.

Do not implement from these files.

| File | Emphasis |
|---|---|
| `1.md` | TEA architecture, 5-crate workspace, RON config, tokio + sqlx, agent specialisation |
| `2.md` | The most complete feature-level plan: config format, SQLite schema, screens, milestones, CI |
| `3.md` | The tightest architecture: 3 crates, sync rusqlite, keymap registry, snapshot testing as a first-class concern |
| `4.md` | The strongest product argument: generic data manager over "CMS", vertical slice over layered milestones, the Golden Path |
