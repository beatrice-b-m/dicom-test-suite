# R7 US genericity preparation audit — 2026-09-05

Read-only preparation while CR public proof was completed. No US genericity,
import, native execution or qualification is claimed. Corpus US source commit
518f9c7 and helper preparation36dece8/ae4c14a remain source-only prerequisites;
US baseline, import, selected availability and migrated parity precede this
future compatibility change.

## Bounded US tuple

Add a US-only inspector in `src/recipes/classic_nuclear.rs`, detecting intent
through `classic/ultrasound/single-frame` or typed family
`ultrasound_single_frame`. The shared nuclear algorithm/projection cannot
identify US by itself. Require native Explicit VR Little Endian, template@1,
`content.native_pixels`, `algorithm.classic_nuclear`, nuclear projection,
shared validation/curated projection rules, one instance/order0, explicit safe
caller path, planning/projection order and empty unsupported extension maps.
Keep dependencies empty and provider modality US.

Preserve the evidenced single-frame U8/OB MONOCHROME2 form, nonzero dimensions,
checked sample counts, byte range, actual extrema and frame hash. Enforce
`ORIGINAL\PRIMARY`, lossy compression `00` and color-data-present0; existing
single-frame semantic validation alone does not establish this contract.
Initially retain absent optional body-part/series metadata, no standards append
or implementation-version projection, no multiframe fields and no calibration
region. The template's broader calibration evidence does not prove this case
emits a region. Exempt only the exact historical RLE binding; PET, NM and
multiframe cases remain on their existing paths.

## Shared-dispatch boundary

The current nuclear planner uses historical case IDs and planning orders400–404.
Typed US selection must return before historical MR/CR and VL/name matchers in
`src/curated_plan.rs`; otherwise misleading caller recipe/case names can cause
another planner to reject or claim the request. The same issue discovered for
CR is separately fixed by ba97c38 and does not itself implement US support.
Shared US validation and manifest projection already discriminate the typed
UltrasoundSingleFrame enum; no US-prefix validation rewrite was identified.

## Sequential ownership and verification

1. Nuclear inspector/planner and existing pure nuclear tests.
2. Loader, shared planning dispatch, bundle/shared tests and ownership/routing.
3. Separate caller fixture/public CLI and SDK proof after accepted US parity.
4. Current guides and dated migration evidence.

Cover independent and misleading identities/orders/paths, partial/crossed tuples,
zero/max dimensions, sample count/range/extrema/hash corruption, forbidden
multiframe fields and historical families. The CT negative control currently
uses unqualified US and must remain meaningful when that capability is admitted.
Read-only source audit found no other required compatibility file. No build,
native execution or standards lookup ran; existing source-pinned evidence is
preserved. Diff check passed.

## Preimplementation review refinements — 2026-09-05

A second read-only review confirms that single-frame parameter validation
currently checks counts without proving the complete bounded pixel contract.
Require frames1, stored typeu8, positive checked dimensions, byte-range samples,
matching extrema and exactly one computed frame hash. Preserve logical instance
with source role `primary` (CR instead uses role `instance`). Require absent
fragments_per_frame and encoding-provider override alongside native policies.

Explicitly exclude classic_projection MR, ICC and semantic_labels fields,
nonempty standards append, implementation-version projection, public profile
membership, qualification/mutation payloads, attribute operations and unrelated
artifact projections. Test absent/null and populated overrides independently.
Provider optional series/body-part fields remain absent. Shared typed validation
is already suitable; early typed shared planning dispatch is still required.
No implementation or execution is claimed by this refinement.

## Loader integration preparation after parity — 2026-09-05

Read-only review identifies loader validate_classic_capability_contract and
registry migrated_classic admission as the two typed-US seams. In curated_plan,
return inspected US after typed CT/DX/CR but before historical MR/CR, nuclear
and VL matchers. Cover PET/VL case names and MR recipe names. Existing typed
nuclear execution validation needs no change. Bundle tests must prove actual
generation/reopened validation plus partial/crossed rejection; replace the CT
name-only US negative control with still-unqualified native NM multiframe.
Ownership and exact bundle routing records follow changed test entries. This
read-only mapping does not implement the loader; planner acceptance remains its
prerequisite. Corpus US parity is independently accepted atb41443d.

## Public caller proof preparation — 2026-09-05

Read-only preparation selects neutral caller/acquisition/ultrasound, recipe
caller_ultrasound, planning900/projection901 and independent/ultrasound.dcm.
Keep core, logical instance/order0/roleprimary and all qualified source US
provider/pixel parameters. After loader acceptance, a closed three-member
fixture, targeted semantic oracle and SDK-only support will compare empty-PATH
CLI and SDK capabilities, complete manifests/reports and actual payload hashes.
Freeze caller hash only after observation; separately preserve original US1
1006-byte e616b8c983c59640a62fa081f636acea554ce681edde3fc8dd53a3c15098c30b.
Retained baseline/parity authenticate source fields; no ordinary manifest or
ignored runtime dependency belongs in the fixture. Exact tests, ownership,
routing, spelling and Fast complete that later boundary. No files or generated
evidence were created by this read-only preparation.
