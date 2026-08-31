# Structural assembly request and manifest design

**Status:** accepted design; implementation is unavailable until S4 is
qualified

**Version baseline:** assembly request `1.0.0`, structural-assembly manifest
`1.0.0`

## 1. Purpose and evidence ceiling

`assemble` is the expert structural route for a caller who needs deterministic,
bounded DICOM Part 10 but cannot use a qualified composition template. It
accepts an explicit SOP Class identity, a caller-owned data-element tree, typed
bulk declarations, and optional logical cross-instance references. It resolves
those declarations into the existing `CorpusPlan` and uses the shared executor
and `Part10Materializer`; it does not introduce another writer or publication
transaction.

Successful assembly proves only:

- internally consistent DICOM Part 10 File Meta Information;
- encodable tags, explicit VRs, values, private blocks, and Sequences;
- protected identity, transfer-syntax, and typed-bulk consistency;
- declared bulk shape, length, hash, padding, and source provenance;
- reference closure for the generic roles the request expresses;
- resource, path, determinism, cleanup, and atomic-publication contracts; and
- strict reopen and structural data-element validation.

It does **not** infer an IOD, insert missing Type 1 or Type 2 attributes,
evaluate conditional modules, select a clinical content model, or claim that
the caller's element set is valid for the supplied SOP Class. Every result and
manifest records `iod_conformance = "not_assessed"`. Structural output has no
template qualification, registry case, profile, coverage, or independent-
conformance credit.

Caller-defined malformed bytes, contradictory file meta, invalid explicit-VR
encodings, and post-serialization corruption are not supported assembly
features. A future mutation surface must be separately designed and retain
negative-profile isolation.

## 2. Request boundary

The top-level assembly request is a JSON object with no unknown properties:

```json
{
  "assembly_request_schema_version": "1.0.0",
  "instances": [],
  "defaults": {},
  "limits": {}
}
```

`instances` is a non-empty, bounded ordered list. Each instance contains:

- a path-safe logical `instance_id`;
- `sop_class_uid` and optional `modality`;
- one qualified assembler `transfer_syntax_uid`;
- deterministic identity declarations, explicit UID declarations, or named
  identity-sharing scopes;
- ordered standard, unknown explicit-VR, private, and Sequence elements;
- zero or more typed bulk declarations;
- zero or more generic logical references; and
- a path-safe output-relative path or a deterministic default path.

`defaults` may provide non-identity values, identity-sharing scopes, and
resource-safe common element declarations. Instance declarations take
precedence only where the same field is not protected. `limits` may lower but
never raise product ceilings. The CLI/SDK supplies seed, caller-asset root,
output root, dry-run, parallelism, cancellation, and resource-root controls
outside the document so host-specific absolute paths never enter canonical
request or plan hashes.

All local content paths are relative slash-separated paths beneath one explicit
caller-asset root. Inline binary uses a bounded canonical base64 representation.
Network URLs and implicit CWD-relative assets are invalid.

## 3. Element declarations

An element identifies exactly one of:

- a bundled-dictionary keyword;
- a canonical hexadecimal tag `GGGG,EEEE`; or
- a private-element declaration with a managed creator.

A standard dictionary element may omit `vr` only when the bundled dictionary
has one unambiguous VR for the tag in this structural context. An unknown
standard or private tag requires an explicit VR. If a known tag supplies an
explicit VR, it must be one of the dictionary-permitted VRs. The parser does
not use a host dictionary or network lookup.

Primitive values are typed rather than inferred from JSON formatting:

- `string` and `strings` for text, UID, date/time, and decimal/integer-string
  VRs, preserving the DICOM lexical representation;
- `integer` and `integers` for binary integer VRs with checked signedness and
  width;
- `float` and `floats` for finite binary floating VRs;
- `tag` and `tags` for AT;
- `bytes` for bounded inline or file-backed OB/OD/OF/OL/OV/OW/UN-compatible
  values;
- `empty` for an explicitly present zero-length value; and
- `sequence` for an ordered list of items, each containing the same recursive
  element model.

Exactly one value form is present. VM, lexical syntax, numeric range, UID
length, text encoding, even-length padding, and VR/value compatibility are
checked before publication. SQ recursion has explicit depth, item-count,
per-item attribute-count, and aggregate expansion ceilings. An empty Sequence
is distinct from an absent element.

### Managed private blocks

A private declaration supplies an odd group, `private_creator`, an element
offset in `00-FF`, explicit VR, and typed value. Planning deterministically
allocates a creator slot from `0010-00FF` within that group and encodes the data
element in the corresponding `xx00-xxFF` block. Requests cannot directly seize
a creator slot, alias two creator strings to one slot, use group `0001` or
`0003`, write a private element without its creator, or collide with another
managed block. Allocation and resolved tags are recorded in the manifest.

## 4. Protected fields

The assembler owns fields whose contradiction would make its identity,
encoding, or bulk evidence false. Raw element declarations cannot set or remove
them.

### Always protected

- every group `0002` File Meta Information element;
- SOP Class UID `(0008,0016)` and SOP Instance UID `(0008,0018)`;
- any Study, Series, Frame of Reference, Dimension Organization,
  Concatenation, Tracking, or other UID field allocated by an identity scope;
- Specific Character Set `(0008,0005)` when the request's selected text policy
  owns it;
- transfer-syntax encoding mechanics, sequence/item delimitation, group
  lengths, padding, and implementation identities; and
- output path, artifact hash, plan hash, and manifest/publication metadata.

Explicit caller UIDs are accepted through typed identity declarations, never
through raw overrides. The resolved dataset SOP identities and file-meta media
storage identities are generated from that single declaration and must match.

### Protected by typed bulk

When a typed bulk declaration owns a payload, raw declarations cannot set that
payload tag or any shape/encoding field named by the bulk kind. Native integer
Pixel Data protects, as applicable, Rows, Columns, Number of Frames, Samples
per Pixel, Photometric Interpretation, Planar Configuration, Bits Allocated,
Bits Stored, High Bit, Pixel Representation, and Pixel Data. Float and Double
Float Pixel Data protect their payload tag, dimensions/frame/sample fields,
and Bits Allocated. Waveform bulk protects Waveform Data and the declared
multiplex/sample/channel/bit fields. Document and mesh adapters protect their
payload tag, MIME/type identity, and length-bearing fields. A general bulk
declaration protects only its exact tag and declared length/VR contract.

Protected-field collisions, duplicate tags within an item, incompatible bulk
owners, or contradictory references fail during planning with stable request
or planning codes. They are never resolved by last-write-wins behavior.

## 5. Typed bulk content

The initial assembler capability catalog distinguishes supported from planned
bulk kinds and transfer syntaxes. Discovery, not this document, is the runtime
authority.

Supported initial structural kinds are designed as:

- native integer Pixel Data with 1-bit or byte-aligned 8/16/32-bit samples;
- native Float Pixel Data with finite little-endian IEEE-754 binary32 values;
- native Double Float Pixel Data with finite little-endian IEEE-754 binary64
  values;
- Waveform Data with explicit signedness, sample width, channel count, sample
  count, and interleaving;
- Encapsulated Document and selected mesh payloads with declared media type and
  structural signature checks; and
- general top-level OB/OD/OF/OL/OV/OW/UN bulk at an explicit non-protected tag.

Each declaration uses one source: inline typed values, inline base64, or a file
beneath the explicit asset root. It records source kind, logical relative path
when applicable, media type, source byte count and SHA-256, resolved value byte
count and SHA-256, padding, shape, per-frame or per-value hashes, and exact
placement tag/VR. Numeric input is converted with checked arithmetic and a
fixed byte order. Non-finite floating values are rejected unless a separately
versioned kind explicitly qualifies them.

The first qualified transfer-syntax set is limited to assembler-advertised
native syntaxes. Encapsulated frame assembly is added only when its frame
boundary, fragment, offset-table, codec, decode-validation, and determinism
contracts are independently qualified. A codec available to curated generation
does not automatically become an assembler capability.

## 6. Identity and references

Every instance gets deterministic Study, Series, and SOP Instance identities
from the product UID root, request seed, schema version, logical instance ID,
and declared sharing scopes unless explicit UIDs are supplied through the typed
identity object. Explicit UIDs must be valid, unique where required, and stable
request content. Canonical identity inputs exclude output root, CWD, staging
path, worker count, and execution order.

Generic references name a source instance ID, relationship label, target
instance ID, and target identity role such as SOP, Series, Study, or Frame of
Reference. Optional referenced frame numbers are one-based and checked against
a target's declared frame count. Planning rejects missing targets, incompatible
identity roles, cycles where the selected relationship forbids them, and
references whose owning element declarations contradict the logical graph.

The generic model only writes reference structures explicitly declared by an
assembler capability. It does not infer the IOD location of a reference from
the supplied SOP Class. Unknown domain-specific reference shapes remain
unavailable rather than being guessed.

## 7. Plan, execution, and dry-run

Parsing produces a typed request. Resolution produces a canonical
`CorpusPlan` before retained file creation. The shared executor owns the DAG,
bounded content services, sole Part 10 materializer, validation evidence,
private staging, cleanup, and atomic no-overwrite publication.

Dry-run performs resource, schema, tag/VR/value, identity, reference, bulk-shape,
path, capability, and plan validation. It returns the same outcome type as
publication with `published = false`, no manifest path, zero published bytes,
and canonical instance/path/identity/plan previews. It creates no requested
output root and does not read content beyond the bounded metadata/hash work
explicitly documented by the request contract.

## 8. Structural validation

Generation-time validation and `validate` reopen each assembled artifact and
check:

- Part 10 preamble, prefix, File Meta Information encoding and required fields;
- dataset transfer syntax and file/dataset SOP identity consistency;
- tag ordering, explicit/implicit VR encoding as selected, value lengths,
  padding, Sequence/item closure, and parse completeness;
- standard dictionary VR where known and request-declared explicit VR where
  unknown;
- managed private creator reservation and resolved private tags;
- exact primitive, multi-value, empty, binary, and recursive Sequence
  projections against the resolved plan;
- typed bulk placement, shape, length, source/resolved/per-frame hashes, and
  numeric reconstruction;
- generic reference closure and identity targets;
- artifact path, size, SHA-256, plan hash, resource evidence, cleanup, and
  publication transitions; and
- the mandatory absence of curated/template evidence claims.

Validation does not compare the dataset with an IOD module table, synthesize
missing attributes, or treat external parser acceptance as conformance. The
manifest warning set names dictionary-unavailable tags and other intentionally
unassessed semantics. An external validator result, if a future command allows
one, is separate evidence and cannot change `iod_conformance`.

## 9. Manifest projection

The structural manifest is a discriminated versioned shape with common product
fields and an assembly-only branch. At minimum it records:

- `manifest_schema_version = "1.0.0"` and
  `run.kind = "structural_assembly"`;
- product/CLI/resource versions and resource hashes;
- request schema version, canonical request SHA-256, seed, caller-asset-root
  identity policy, corpus-plan SHA-256, dry-run/publication state, and resource
  totals;
- `iod_conformance = "not_assessed"` at the run and instance projections;
- instance ID, output path, size, SHA-256, resolved-plan SHA-256, SOP/transfer-
  syntax identities, deterministic or explicit identity provenance, and
  generic references;
- resolved element records with tag, keyword when known, VR, value form,
  cardinality, canonical value hash/projection, source, private creator and
  resolved block where applicable, and recursive item paths;
- typed bulk kind, tag/VR, shape, source/resolved sizes and hashes, per-frame or
  per-value hashes, padding, and provenance;
- structural validation checks and warnings;
- unavailable capability records with stable reason codes; and
- cleanup and atomic publication transitions.

The structural branch contains no `case_id`, profiles, registry coverage,
`template_id`, template qualification, IOD validation pass, or independent-
conformance pass. Report projections group structural content and availability
separately and cannot join it into curated coverage matrices.

## 10. Failure and resource contract

Syntax, JSON, schema, version, tag/VR/value, and protected-field errors use exit
2. Missing assembler content kinds, transfer syntaxes, features, or runtimes use
exit 3. Unsafe paths, destination conflicts, and caller-controlled ceilings use
exit 4. Planning, materialization, provider, cancellation, or structural
validation failures use exit 5. Unexpected I/O and internal invariant failures
use exit 6. Machine errors use codes from `product/cli-error-codes.json`; S4 may
append assembly-specific codes but cannot reuse existing meanings.

Resource accounting covers request bytes, decoded inline bytes, external asset
files and bytes, instance expansion, attributes, Sequence depth/items, value
lengths, frames, fragments, output files and bytes, manifest allowance,
working-set estimates, diagnostics, provider time/output if enabled,
cancellation, and cleanup. Checked arithmetic precedes allocation or file
creation. Product ceilings are discoverable and request limits can only lower
them.

## 11. Promotion tests

Implementation is not promoted until positive and adversarial fixtures cover
standard and unknown explicit-VR elements; private allocation; empty, primitive,
multi-valued, binary, and recursive Sequence values; identities and references;
integer, float, double-float, waveform/document/mesh/general bulk; protected
collisions; malformed VR/VM and numeric ranges; traversal and symlinks; resource
overflow; cancellation; cleanup; races; dry-run; deterministic parallelism;
manifest/report schemas; and explicit absence of IOD/template/curated claims.

Those tests execute through the packaged public CLI and SDK in addition to
focused in-crate contracts. Until all pass, discovery reports structural
assembly as unavailable with a stable reason.
