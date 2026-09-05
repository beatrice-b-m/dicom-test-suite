# R7 accepted SC slices: remaining genericity audit — 2026-09-05

Source-only audit at generator `ea4d151`, reviewed by root. Existing smoke/core
migration acceptance is unchanged. This is neither new runtime evidence nor
completion of R7.2/R7.3.

## Finding and next sequential boundary

`src/recipes/loader.rs` still admits `native.sc_plan` registry bindings by the
`classic/sc/` namespace or exact EOT case. The SC planner and shared dispatch
already use typed contracts. Thus imported ownership and accepted historical
parity have not established caller-name independence for these earlier slices.
After metadata genericity is accepted, complete this SC boundary before the next
CR1 import. Do not overlap changes to the shared loader/public corpus contract.

The bounded candidate tuple is one explicit-path, single-frame artifact using
`native.sc_plan`, parameter-free `content.sc.pixel_pattern`, the matching
monochrome or RGB Secondary Capture template@1, and native Explicit VR Little
Endian encoding. Preserve qualified pixel, padding, palette, color, validation
and projection semantics. Exclude metadata, classic projection, nonsquare
geometry, algorithm providers and arbitrary attribute overrides. Exceptional
codecs, EOT, multiframe, paired geometry and stress retain their existing paths;
the current prefix must not become unrestricted provider admission.

The ordinary SC bit check also uses unchecked `high_bit + 1`; replace it with
checked arithmetic and exercise maximum typed input during that boundary.

## Bounded ownership and acceptance

1. Loader and bundle tests: conjunctive admission, renamed/misleading positives,
   crossed/partial tuples, maximum high bit, unsupported topology/encoding and
   preserved historical specialized admission. Update exact routing/ownership.
2. Standalone generic SC fixture/support and separate public CLI/SDK test:
   monochrome, signed, padding, palette, RGB planar and YBR representatives;
   complete manifest/report equality, actual payload hashes, output closure and
   original smoke/core byte oracles. No internal product imports.
3. Root-owned current guides and dated status after independent review and the
   smallest affected Subsystem/Fast/public-consumer checks.

The existing historical source inventory remains authoritative for exact cases,
recipes and bytes. Query it before assigning the final fixture closure; this
proposal introduces no case count invariant or fresh corpus generation.

Verification was read-only source and status inspection, without Cargo, native
execution, generated artifacts or build storage. Root confirmed namespace
admission and the unchecked high-bit expression; `git diff --check` passed.
