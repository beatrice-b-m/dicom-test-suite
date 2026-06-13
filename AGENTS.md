# Repository Guidelines

## Project Overview

`dicom-test-suite` is a Rust project for generating a comprehensive local corpus of synthetic DICOM files for viewer compatibility testing.

---

## Implementation Progress Tracking

Implementation progress **MUST** be tracked in `IMPLEMENTATION_PROGRESS.md`.
This document is the durable hand-off reference between coding agents.

**Rules:**

- Review `IMPLEMENTATION_PROGRESS.md` before starting implementation work.
- Update `IMPLEMENTATION_PROGRESS.md` whenever a task changes project status,
  phase status, completed checklist items, blockers, open decisions, or the
  recommended next step.
- Commit progress updates in the same granular commit as the implementation or
  documentation change that caused the status change.
- Do not treat `SYSTEM_SPEC.md` as a progress tracker; it is the architecture
  and requirements source of truth.

---

## Git Commit Policy

Every completed task **MUST** be tracked in a descriptive, granular git commit.
This requirement is **absolutely critical** and must be followed under all
circumstances - no exceptions.

**Rules:**

- Commit after every distinct logical unit of work, not at the end of a session.
- Each commit covers exactly one coherent change (one module, one component, one
  test suite, one docs section). Do not batch unrelated changes into a single
  commit.
- Commit messages must be informative: use `type(scope): subject` format,
  include a blank line, then a body describing *what* changed and *why*.
  - Types: `feat`, `fix`, `test`, `docs`, `refactor`, `chore`
  - Scope: the module, file, or subsystem affected, such as `backend`,
    `frontend`, `pixels`, `server`, `types`, or `tests`
  - Subject: imperative mood, 72 characters or fewer
  - Body: explain the design decision, the invariant being established, or the
    behavior being changed, not a restatement of the diff
- Stage files selectively (`git add <file>`) rather than `git add -A`. Only
  commit files that belong to the current logical unit.
- Never amend or force-push commits that have been logged here.

**Verification:** After each task, run `git log --oneline -3` to confirm the
commit was recorded before moving to the next task.
