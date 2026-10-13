# Historical license material inspection

This is technical evidence for GH-239, not a redistribution decision or final RC acceptance. All approvals remain pending. Engineering Policy semantics are unchanged.

Both archives were read without extraction. Every regular file was checked against the collector manifest or bootstrap input map; unexpected, missing, duplicate and unsafe members are rejected. Bootstrap input metadata itself is separately hashed. These are internal consistency checks, not independent source authentication.

Historical source: `48099c48f1fded35af5d71638bf57160cb8e0b31`. Collector manifest SHA-256: `ee184213fbf96ccb8781a6fa0eafdd7a3512c40321a80917ba8a85e8bf1158a0`.

| Archive | Files | Bytes | SHA-256 |
| --- | ---: | ---: | --- |
| collector | 1102 | 1623490560 | `3143043f3093a97a6aaebcaac964eb27f9db2f41d9e013cad92a514b9ba18bfa` |
| bootstrap | 688 | 53780480 | `e5e2029ec7b33493ca2ef05e89eba3630c40a4b4c4dd9ed53958d0fbf1a72408` |

## Crate declarations

The following declarations and notice candidates were read from each embedded Cargo archive. Both distributions contain the same 21 archive digests. Candidate texts are not automatically applicable to all linked binaries. Registry checksum authentication and human applicability review remain pending.

| Package | Version | Declared license | Notice candidates |
| --- | --- | --- | ---: |
| block-buffer | 0.10.4 | MIT OR Apache-2.0 | 2 |
| cfg-if | 1.0.4 | MIT OR Apache-2.0 | 2 |
| cpufeatures | 0.2.17 | MIT OR Apache-2.0 | 2 |
| crypto-common | 0.1.7 | MIT OR Apache-2.0 | 2 |
| digest | 0.10.7 | MIT OR Apache-2.0 | 2 |
| generic-array | 0.14.7 | MIT | 1 |
| itoa | 1.0.18 | MIT OR Apache-2.0 | 2 |
| libc | 0.2.189 | MIT OR Apache-2.0 | 2 |
| memchr | 2.8.3 | Unlicense OR MIT | 2 |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 | 2 |
| quote | 1.0.47 | MIT OR Apache-2.0 | 2 |
| serde | 1.0.229 | MIT OR Apache-2.0 | 2 |
| serde_core | 1.0.229 | MIT OR Apache-2.0 | 2 |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 | 2 |
| serde_json | 1.0.151 | MIT OR Apache-2.0 | 2 |
| sha2 | 0.10.9 | MIT OR Apache-2.0 | 2 |
| syn | 3.0.5 | MIT OR Apache-2.0 | 2 |
| typenum | 1.20.1 | MIT OR Apache-2.0 | 3 |
| unicode-ident | 1.0.24 | (MIT OR Apache-2.0) AND Unicode-3.0 | 3 |
| version_check | 0.9.5 | MIT/Apache-2.0 | 2 |
| zmij | 1.0.23 | MIT | 1 |

Preserve the additional Unicode-3.0 declaration for unicode-ident. The version_check declaration is retained verbatim as MIT/Apache-2.0, without silently normalizing it into an approved expression.

## Remaining evidence

- Bind compiler, LLVM, Python, OS libraries and linker tools to exact source/package/build provenance and applicable notices. Directory names alone do not establish applicability.
- Retain required corresponding source and build/patch materials, then obtain the owner’s actual redistribution decisions.
- Review bootstrap-specific installer code and independently distributed launcher/capsule. This archive inspection does not certify an external launcher.
- Re-run against final source-bound rebuilt artifacts before final approval; historical hashes cannot approve replacement bytes.

## Reproduction

```sh
python3 docs/quality/gh-239/prerequisites/inspect_license_materials.py \
  /mnt/dev-ssd/artifacts/harness-gate/gh-231-preparation-20260911 \
  /tmp/license-materials-inspection.json
python3 -m unittest discover -s docs/quality/gh-239/prerequisites \
  -p 'test_inspect_license_materials.py' -v
```
