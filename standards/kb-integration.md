# DICOM Standard KB Integration

This project uses the pinned 2026b `dicom-standard-kb` MCP reference as the
canonical machine-queryable standards interface. `standards.lock.json` records
the active baseline and must be updated deliberately before recipes or schemas
use a different DICOM edition.

## Query Order

Use `dicom-standard-kb` before adding or changing any recipe, registry entry,
IOD builder, transfer syntax assumption, data element rule, enumerated value, or
defined term.

Preferred MCP queries by decision type:

| Decision | Preferred query |
|---|---|
| UID value, UID keyword, UID retirement state | `dicom_lookup_uid` |
| SOP Class to IOD relationship | `dicom_lookup_sop_class` |
| IOD identity | `dicom_lookup_iod` |
| IOD module list | `dicom_list_modules_for_iod` |
| Module attributes | `dicom_list_attributes_for_module` |
| Attribute tag, VR, VM, retirement state | `dicom_lookup_data_element` |
| Attribute defined terms | `dicom_lookup_defined_terms` |
| Attribute enumerated values | `dicom_lookup_enumerated_values` |
| Standard text excerpt or anchor lookup | `dicom_search_standard_text` or `dicom_retrieve_standard_text` |

Treat the MCP result as edition-pinned evidence only when the response reports
`edition: "2026b"`. If a query returns another edition, no edition, or an
unexpected source manifest, stop and update the standards lock decision before
using the result.

## Evidence To Record

Every planned or implemented recipe needs durable standards evidence in
`cases/registry.json`, recipe metadata, or a generated manifest sidecar. At
minimum, record:

- `source`: `dicom-standard-kb`
- `edition`: `2026b`
- the exact query or MCP tool/input used
- whether the content was covered by the KB
- relevant official reference part, table, section, or anchor from the MCP refs
- the source manifest SHA-256 when available

Example evidence shape:

```json
{
  "source": "dicom-standard-kb",
  "edition": "2026b",
  "query": "dicom_lookup_uid ExplicitVRLittleEndian",
  "covered": true,
  "refs": [
    {
      "part": "PS3.6",
      "table": "UID Values",
      "anchor": "table_A-1"
    }
  ],
  "source_manifest_sha256": "9959bee76fd293c7eda3fc81ce2ced7528612faa1b2df28cccd01504a83f54b0"
}
```

Use concise evidence. Do not copy large portions of standard text into project
artifacts.

## Fallback Path

The KB parser surface is currently focused on PS3.3, PS3.4, and PS3.6. For
content outside that surface, or when exact wording/conflict resolution matters,
use official DICOM source artifacts according to `SYSTEM_SPEC.md`:

1. Official DICOM PDFs are authoritative for exact text.
2. Official HTML, CHTML, DocBook XML, and TargetDB artifacts are acceptable for
   lookup, citation, and machine extraction.
3. `dicom-standard-kb` remains the default interface for covered content.

When falling back, add a small source note under `standards/source-notes/` or
mark the case blocked/skipped in `cases/registry.json`. Do not commit official
standard PDFs, generated full-standard JSON, full-text indexes, SQLite KB files,
or other generated standards artifacts.

## Recipe Workflow

For each new case or builder:

1. Look up the SOP Class UID and IOD relationship with `dicom_lookup_sop_class`.
2. Look up the Transfer Syntax UID with `dicom_lookup_uid`.
3. Resolve the IOD and module requirements with `dicom_lookup_iod` and
   `dicom_list_modules_for_iod`.
4. Resolve required module attributes with
   `dicom_list_attributes_for_module`; expand macros only when the recipe needs
   the expanded attribute rows.
5. Look up each non-obvious attribute with `dicom_lookup_data_element`.
6. Look up enumerated values and defined terms before constraining recipe values.
7. Record evidence and any unresolved gap before implementing generator logic.

The generator may encode implementation constraints from DICOM-rs and feature
flags, but conformance decisions must come from the standards evidence path
above.
