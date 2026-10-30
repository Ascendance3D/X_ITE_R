# Standards fixture library

Project-authored scenes that test one clause of a published standard, and manifests that list them and other
fixtures by path, sha256 and bytes. The manifests do not copy a fixture: an existing file stays where it is and is
listed in place.

```
python3 -I -B oracle/tools/standards_manifest.py check        # every manifest
python3 -I -B oracle/tools/test_standards_manifest.py         # the checker's own tests ("Fast checks" runs both)
```

## Layout

| path | content |
| --- | --- |
| `MANIFEST.schema.json` | the manifest schema. The checker reads its required fields and enums. |
| `manifests/iso-vrml97.json` | VRML97 candidates. All are `ISO_REVIEW_NEEDED`. |
| `manifests/regression.json` | `REGRESSION_ONLY` references: behavior tests, and the `#X3D V4.1` files. |
| `iso/vrml97/<topic>/positive/` | valid scenes, with the libraries they load |
| `iso/vrml97/<topic>/negative/` | invalid scenes. Each states the clause that it breaks. |
| `behavior/vrml97/<topic>/` | scenes for a case the standard does not decide. They assert no standards result. |

## Standards text

VRML97 is ISO/IEC 14772-1:1997. The clause numbers come from the Web3D free copy of ISO/IEC 14772-1. Equality of that
copy with the ISO publication is UNVERIFIED, so a citation names the clause of the Web3D copy. Each scene names its
clauses in a header comment, and each manifest entry has them in `spec_citations`.

## Classes

| class | meaning |
| --- | --- |
| `ISO_VERIFIED` | Verified against the standard. Not used: the checker refuses it (see the promotion rule). |
| `ISO_REVIEW_NEEDED` | A candidate whose standards result is cited but not verified. |
| `REGRESSION_ONLY` | A reference for behavior or for a version out of scope. It is not a standards test. |
| `BLAXXUN_COMPAT` | Historical browser-extension behavior. Not used yet; needs a license record first. |
| `LICENSE_REVIEW` | A donated file waiting for a written grant. Not used yet. |
| `REJECT` | A file that must not be used. Not used yet. |
| `PRIVATE_KEEP_OUT` | A private file. It never goes in a manifest; the checker refuses `origin: private`. |

`x_ite_parity` records only how the pinned X_ITE 16.5.0 compares with the Rust port (`MATCH`, `DIFFER`, `NONE` when
X_ITE gives no result, `UNKNOWN` when not checked). A scene that X_ITE loads is not proof that the scene conforms.
The pinned parser accepts any `#X3D` or `#VRML` header.

## Tags

The optional `tags` array holds lowercase words.

- `behavior`: the standard states no result for the case (for example, recursive Inline or a URL list where no
  entry loads). The entry must be `REGRESSION_ONLY` with `expected.standard: unstated`.
- `negative`: an invalid scene. It sits in a `negative/` folder and has `expected.standard: invalid`.
- `library`: a file that other scenes load.

## Promotion rule

No entry may become `ISO_VERIFIED` in this library yet, and the checker refuses the class. Promotion of one entry
needs all of these:

1. a clause-level citation of the governing standard;
2. an independent check of the semantic rules for that encoding and version;
3. the owner's decision for that entry.

XSD or DTD validation never promotes an entry by itself. It covers X3D XML only, and it checks structure, not
semantics. Loading in X_ITE or in the Rust port is not a standards check.

## Content rules

- Author each scene from the standard's text only. Do not copy or adapt historical content.
- No historical asset from a private archive goes in this library or in a manifest. Historical browser-extension
  content waits for a written license record (`license_record`) and the `LICENSE_REVIEW` class.
- The SGI VRMLScript source archive (`vrmlscript.zip`) stays private.
- The historical browser release that first supported trimmed NURBS surfaces is unresolved: the sources disagree.
  Do not state it in a fixture or a manifest. No NURBS fixture is in the library yet.
- A URL in a scene is a relative local path or a host in the reserved `.invalid` domain (RFC 2606), one host per case.
- X3D 4.1 files stay regression inputs only.
