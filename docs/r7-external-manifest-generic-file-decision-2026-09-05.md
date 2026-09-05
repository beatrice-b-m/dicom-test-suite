# R7 external manifest generic-file decision — 2026-09-05

Status: accepted implementation direction; execution verification pending.

US loader108a2cc exposed an external publication bug: a complete typed US
recipe using the exact historical PET case identity passes planning but fails
manifest validation because manifest-v2 references legacy manifest file rules.
Read-only review finds one generic validity/evidence branch and36 case-ID
policies in that legacy file definition. Caller case identity must not select
embedded corpus policy for an independently verified external definition.

Add a file definition inside manifest-v2.schema.json, preserving every base
property, required field, unknown-field restriction and generic validity/evidence
branch. Exclude only the historical case-ID conditionals. Rebase copied local
references to the unchanged legacy schema ID so all field/evidence definitions
retain their semantics. Preserve legacy manifest.schema.json and manifest-v1
bytes; their historical case policies remain required for their own readers.
A structural equality regression must guard exact preservation of the generic
contract, not merely accept one happy-path output.

Under compatibility policy section2 this fixes rejection of input the external
corpus contract promises to accept. Keep manifest2.0.0 shape and recipe/template
versions unchanged in the unreleased product0.2.0 source. No required field or
accepted value meaning changes. Schema/resource identities will change; no
manifest byte identity or pin update is implied. Historical parity artifacts
remain bound to their original executable/schema domains.

Run a separate sequential schema unit after accepted loader verification.
Prove exact historical PET/VL caller-ID publication, reopened validation and
reporting; reject malformed structural/evidence fields. Prove legacy PET
rejection remains intact and freeze old schema bytes. Update exact ownership/
routing records and current compatibility guidance after verification. Public
US CLI/SDK standalone proof follows. No unrelated schema cleanup, permissive
normalization or release qualification belongs in this unit. Remaining legacy
schema dependency removal stays with the later R9 ownership cleanup.
