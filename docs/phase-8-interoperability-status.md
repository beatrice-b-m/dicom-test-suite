# Phase 8 Media and Protocol Interoperability Status

Status date: 2026-08-28

## Implemented reporting substrate

Phase 8 interoperability evidence is deliberately separate from ordinary
generated-file coverage. The command-line interface exposes two bounded
commands:

```text
dicom-test-suite interoperate media-dicomdir GENERATED_ROOT --dcmmkdir PATH --dcmdump PATH --dciodvfy PATH [--dcentvfy PATH] --format json|markdown [--timeout-seconds N]
dicom-test-suite interoperate protocol-baseline GENERATED_ROOT --format json|markdown [--seed SEED] [--fixtures PATH]
```

Media qualifications serialize against
`schemas/media-report.schema.json`. Protocol transactions serialize against
`schemas/transaction-report.schema.json`. These dedicated contracts keep
media, DIMSE, DICOMweb, and TLS outcomes out of file-conformance rows while
binding source case IDs, source hashes, tool or peer fingerprints, deterministic
transaction IDs, ordered outcomes, and explicit blocker evidence.

The protocol report contains public certificate, certificate-fingerprint, and
public-key hashes only. Private-key paths and hashes from the fixture lock are
never copied into transaction reports.

## Mixed DICOMDIR qualification

A fresh extended seed-7 generated root contained 115 DICOM files. The bounded
mixed File-set selector chose exactly three existing synthetic objects:

- `enhanced/ct/multiframe_shared_perframe_explicit_le`, the image source;
- `derived/seg/binary_multiframe_explicit_le`, the derived object whose source
  reference closes over that Enhanced CT instance; and
- `non-image/waveform/general_ecg`, the non-image object.

The runner copied those objects into private staging under conforming,
extension-free File IDs, invoked DCMTK `dcmmkdir`, parsed the resulting
`DICOMDIR`, and removed staging after recording the qualification. The provider
was DCMTK `dcmmkdir` 3.7.0 with executable SHA-256
`47ed521a5fafc6d691def99caccaff8f606410da20a35185d02e5a79486ec511`.
Its version, executable hash, and exact argument vector are part of the media
report.

The final qualification passed all locally available checks:

- the Rust File ID, SOP identity, reference, and directory-record closure walk;
- dicom3tools `dciodvfy -new` on the `DICOMDIR`;
- isolated dicom3tools `dcentvfy` over the three-member File-set; and
- a second DCMTK parser pass, recorded as same-provider-family evidence.

The run reported zero provider or validation warnings after the SEG recipe was
corrected to preserve the source Enhanced CT Study ID. This proves the bounded
local construction and closure contract. It does not prove independent
interoperability: the approved dcm4che File-set peer was unavailable, so the
report records that check as unavailable,
`independent_interoperability_proven` remains `false`, and
`media/dicomdir/mixed_file_set` remains planned and non-promotable.

## Protocol availability report

The deterministic protocol-baseline command consumes the same hash-linked
generated sources and emits one transaction for each protocol family. No
network transaction is started when the required replaceable peer is absent.
The current report therefore contains three unavailable outcomes and no passes
or failures:

| Family | Registry case | Blocker code | Availability boundary |
| --- | --- | --- | --- |
| DIMSE | `protocol/dimse/storage_query_retrieve` | `independent_dcm4che_peer_unavailable` | Local DCMTK tools cannot supply independent evidence by communicating only with another DCMTK process. |
| DICOMweb | `protocol/dicomweb/stow_qido_wado` | `pinned_independent_dicomweb_server_unavailable` | No pinned, replaceable independent server is configured for STOW-RS, QIDO-RS, and WADO-RS. |
| TLS / user identity | `protocol/security/tls_user_identity` | `replaceable_tls_peer_unavailable` | The approved synthetic public PKI is fingerprinted, but no replaceable independent TLS and user-identity peer is configured. |

Each unavailable row retains its stable case ID and deterministic transaction
ID, fingerprints the harness and unavailable peer identity, links the selected
source cases, and explains that no association, HTTP exchange, handshake, or
authentication was attempted. An unavailable outcome is never counted as a
pass.

## Security-media availability

The repository contains the approved fixed synthetic PKI fixture set and its
deterministic lock. That approval resolves fixture policy; it does not create
independent security evidence by itself.

`media/security/digital_signature_instance` remains explicitly unavailable
under `security_toolchain_unselected` because no deterministic DICOM signature
creator and independent CMS verifier are integrated.
`media/security/secure_file_set` remains explicitly unavailable under the same
code because no Secure DICOM File creator and independent secure-media verifier
are integrated. Neither case emits a generated artifact or file-conformance
row.

## Completion boundary

The Phase 8 substrate, bounded DICOMDIR execution, dedicated schemas, report
separation, CLI entry points, synthetic-public-PKI handling, and explicit
unavailability behavior are complete. Promotion of the planned media and
protocol cases remains conditional on installing and pinning the independent
peers named above; their absence is a recorded coverage result, not an implied
success or a reason to substitute same-provider evidence.
