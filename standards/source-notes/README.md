# Source Notes

This directory holds narrow standards evidence notes for cases where the pinned
2026b `dicom-standard-kb` MCP does not yet provide enough project-usable
coverage.

Do not commit official DICOM PDFs, HTML snapshots, DocBook XML, generated
full-standard JSON, full-text indexes, SQLite KB files, or other generated
standards artifacts here.

## Note Template

```markdown
# <Topic>

Checked: YYYY-MM-DD
Standards baseline: 2026b, `standards.lock.json`

## Affected Project Surface

- Case IDs:
- Recipe IDs:
- Schema fields or validation rules:

## Required Decision

Describe the implementation invariant or recipe decision that needs standards
evidence.

## KB Query

- Tool:
- Input:
- Edition returned:
- Source manifest SHA-256:
- Result:
- Why insufficient:

## Official Source Evidence

- Part:
- Section, table, or anchor:
- Source artifact identity from `standards.lock.json`:
- Concise evidence summary:

## Project Action

- Registry status: planned | implemented | blocked | skipped
- Registry reason or linked issue:
- Should become KB patch: yes | no
- Expected cleanup after KB coverage exists:
```

Keep source notes short and factual. A note should cite official anchors and
summarize the project decision rather than reproduce standard text.
