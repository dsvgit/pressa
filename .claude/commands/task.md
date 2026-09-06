---
description: Implement one M0 task, tests first (implementer role)
---

You are the **implementer**. Task: $ARGUMENTS

`AGENTS.md` governs everything below. Read it if it is not already in context.

## Step 0 — set up, without asking

Do these yourself. Only stop where told to stop.

1. Find the spec: `grep -l "Task: T<NN>" specs/*.md`. T1 has no spec (workspace
   skeleton); every other task does.
2. Read its header. **If the status is not `Approved`, stop.** Say the spec must
   be approved first, and that `/spec <NNN>` finishes it. Never set `Approved`
   yourself — that signature is the coordinator's, and a status you granted
   yourself is not a gate.
3. `git status --porcelain`. If the tree is dirty, stop and ask.
4. Branch:
   - on `main` → `git checkout -b feat/t<NN>-<slug>`, where `<slug>` comes from
     the spec's filename without its number (`003-storage-repository.md` →
     `storage-repository`);
   - already on this task's branch → continue on it;
   - on some other branch → stop and ask.

## Step 1 — read before writing

The spec in full, every ADR it references, and the existing code and tests you
are about to touch. Do not skim to the API section and start typing.

## Step 2 — failing tests first

Turn each acceptance criterion into a test, and **run them to watch them fail**.
This is the step that gets skipped and the one that matters: tests written after
the implementation test what the code does, not what the spec asked for.

## Step 3 — implement until green

Nothing beyond the criteria. If you notice something worth building that the
spec did not ask for, write it down and move on.

## Step 4 — verify

`just ci` — fmt, clippy with `-D warnings`, and the full test suite.

## Step 5 — self-check, then hand over

Walk the acceptance criteria one at a time against your own diff and say, for
each, which test covers it. Then set the spec's status to `Implemented` and
report:

- the branch name;
- criteria with their tests;
- anything you deliberately did not do, and why.

Do not commit or merge. The coordinator reviews with `/review T<NN>` and merges.

## Stop rule

If the architecture turns out to be wrong for what the spec asks: **stop. Do
not redesign.** Write down the problem, propose an ADR, and ask. Never change
architecture inside a feature diff — a diff where both move cannot be reviewed.
The same goes for dependencies: a new crate needs an ADR first.
