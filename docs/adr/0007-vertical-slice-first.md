# ADR-0007: Build a vertical slice before building breadth

Status: **Accepted** · Date: 2026-09-06

## Context

Three of the four source plans scheduled M0 by layer — a week on the domain, a
week on storage, a week on the TUI, a week on polish. `ai/4.md` argued instead
for a "Golden Path": one end-to-end scenario, working, before anything is made
broad.

Layered schedules have a specific failure mode with agent-driven work. Weeks of
infrastructure get built against assumptions no running system has tested, and
the assumptions are wrong in ways nobody discovers until integration — at which
point the affected code is large, written by an agent, and unfamiliar.

## Decision

M0 is ordered as a **vertical slice**. Tasks T1–T9 take the narrowest possible
path from `pressa.yaml` to a saved record that survives a restart. Breadth
(create/delete UX, search, help) comes after, in T10–T12.

The slice is defined as an executable acceptance scenario in
[`specs/000-m0-golden-path.md`](../../specs/000-m0-golden-path.md).

## Rationale

- Integration risk is paid down first. The first time storage, services and UI
  meet is task 9 of 12, not task 12 of 12.
- Every layer gets a real consumer immediately, which is the only reliable test
  of whether an API is usable.
- There is something to run and judge early. For a product whose premise is a
  feel — "is a terminal CMS actually pleasant?" — an opinion at week two is
  worth more than a better-factored core at week four.
- Progress is legible. "The Golden Path passes" is a fact; "the domain layer is
  done" is an assertion.

## Consequences

- Some code written early is deliberately narrow (single-column sort, no
  pagination UI) and gets widened in T10–T12. That is accepted rework, and it
  is smaller than the rework caused by integrating three untested layers.
- The `MemoryRepository` must exist by T4, so that T5 and the UI tasks are not
  blocked on SQL details.
- Tasks are not reordered for convenience. Pulling a T11 feature forward
  because it is "easy while we are here" is exactly the failure this ADR
  prevents.
