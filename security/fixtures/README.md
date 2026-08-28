# Public synthetic PKI fixtures

These certificates and private keys are intentionally public test fixtures for
`dicom-test-suite`. They protect nothing, are trusted by no production system,
and must never be reused outside this repository's synthetic tests.

The fixture set contains a test root CA plus separate dataset-signing, TLS
server, and TLS client identities. Subjects, serial numbers, key usage,
extended key usage, and certificate bytes are fixed in git. Corpus generation
copies or references these source fixtures; it never creates replacement keys.

The OpenSSL configuration documents the requested extensions. Certificate
fingerprints and validity intervals are locked in `fixtures.lock.json` after
independent parsing. Private-key files are committed only under the explicit
approval recorded in `docs/coverage-expansion-decisions-2026-08-28.md`.

Never add a real key, real identity, or organization trust anchor here.
