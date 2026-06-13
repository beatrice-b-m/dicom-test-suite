# Standards Gap and Patch Workflow

Use this workflow when the pinned 2026b `dicom-standard-kb` MCP does not cover
the standards content needed for a case, recipe, validation rule, or schema.

## When This Applies

A standards gap exists when:

- an MCP lookup returns no result for required 2026b content;
- an MCP result lacks the module, attribute, term, table, or section needed for
  a recipe decision;
- the needed content is outside the current KB parser surface;
- the MCP result conflicts with an official DICOM source artifact;
- exact official wording is needed for a conformance rule.

Do not fill a gap from memory or viewer behavior. Viewer observations may
explain why a case is useful, but they do not establish DICOM validity.

## Decision Path

Handle each gap in this order:

1. Re-run the relevant `dicom-standard-kb` query and record the exact tool/input,
   edition, refs, warnings, and source manifest SHA-256 when present.
2. Check official DICOM source artifacts using the authority hierarchy in
   `SYSTEM_SPEC.md`.
3. If the gap is systematic and reusable, create or plan a KB patch.
4. If the gap is narrow, add a local source note under
   `standards/source-notes/`.
5. If evidence or implementation support is still insufficient, keep the case in
   `cases/registry.json` with status `blocked` or `skipped` and a clear reason.

Prefer a KB patch for repeatable extraction problems such as missing module
tables, missing defined terms, missing enumerated values, or repeated SOP Class
relationships. Prefer a local source note for one-off anchors, wording checks,
or content outside the current project implementation phase.

## Local Source Notes

Local notes are concise hand-authored evidence records. They may cite official
sections, tables, anchors, and short excerpts when necessary, but they must not
redistribute official DICOM source artifacts or copy long passages of standard
text.

Each note must include:

- affected case IDs, recipe IDs, or schema fields;
- the required decision or implementation invariant;
- the failed or insufficient `dicom-standard-kb` query;
- DICOM part, section, table, or anchor used as fallback evidence;
- source artifact identity from `standards.lock.json` when available;
- whether the gap should become a KB patch;
- date checked;
- resulting registry action: planned, implemented, blocked, or skipped.

Use lowercase, hyphenated filenames such as
`standards/source-notes/part10-file-meta.md`.

## Registry Actions

Use `blocked` when the case remains desirable but cannot be implemented with
current standards evidence or generator support.

Use `skipped` when the case is intentionally unavailable for a profile or local
environment, for example because a required optional codec is not enabled.

Every blocked or skipped case must include a machine-readable reason in
`cases/registry.json` and should reference the source note or KB patch issue
that explains the decision.

## KB Patch Notes

When a KB patch is the right outcome, record the gap in the source note until
the patch lands. Include:

- the missing content class, such as module table, macro expansion, UID mapping,
  defined term, or enumerated value;
- the official source anchor used to verify the content;
- the recipe or validation logic currently waiting on the patch;
- the expected project behavior after the patch is available.

After the KB is patched and the project updates `standards.lock.json`, remove
temporary fallback assumptions from recipe logic and replace local evidence with
normal `dicom-standard-kb` evidence where practical.
