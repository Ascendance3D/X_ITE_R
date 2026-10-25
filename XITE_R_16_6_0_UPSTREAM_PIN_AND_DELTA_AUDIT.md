# X_ITE_R — 16.6.0 Upstream Pin and Delta Audit

**Audit only. No Rust behavior was changed, no oracle capture was re-recorded, the active
oracle version is untouched, and nothing is merged.** The audit itself, the inert 16.6.0
reference pin, its checker and the re-blessed 16.5.0 digest *are* committed and pushed, as
PR #44 for owner review; §7 lists exactly what the PR carries.

| | |
| --- | --- |
| date | 2026-10-24 |
| X_ITE_R master | `f7a67cdcb817b9d7cecdbcb8419844f252160eec` (merge of PR #41) |
| target upstream SHA | `5f282709c124ffa4e2bbd57d9b199893bf94bd32` |
| target identity | tag `16.6.0`, `package.json` `16.6.0`, `BROWSER_VERSION.js` `16.6.0`, Holger Seelig, 2026-10-21 16:03:21 +0200 |
| existing oracle pin | `34cdf9f97145b3cd2b8e250f8c88dfdd083c0517`, declared 16.5.0 |
| B25-2 branch in flight | `perf/b25-2-gpu-instanced-picking` — **not touched** |

---

## 1. Finding that changes the scope of the brief

**The brief's "49 upstream commits" is the tag-to-tag range `16.5.0..16.6.0`. The range
X_ITE_R actually has to absorb is 57 commits, because the existing oracle pin is 8
commits *behind* the 16.5.0 release tag.**

```
34cdf9f97  <- oracle/x_ite-pin/PINNED_COMMIT  (X_ITE_R's reference)
   ... 8 commits ...
1956793db  <- tag 16.5.0
   ... 49 commits ...
5f282709c  <- tag 16.6.0  (the audit target)
```

Verified: `34cdf9f97` is an ancestor of `1956793db`, which is an ancestor of `5f282709c`.
`git rev-list --count 1956793db..5f282709c` = **49**;
`git rev-list --count 34cdf9f97..5f282709c` = **57**.

`34cdf9f97` is *not* an upstream release point at all — it is the head of our own fork
branch `fix/firefox-vrml-xml-parser`, which upstream merged as `514520077`, two commits
before cutting 16.5.0. That is why the pin sits off-tag. The pin is internally consistent
(`package.json` says 16.5.0, and 16.5.0 had already been released as a version string on
`development`), so nothing is *wrong* with it — but a migration planned against the
49-commit range would silently skip 8 commits of real delta.

The extra 8 commits are listed in §3.8. One of them is a genuine behavior change.

---

## 2. Phase 1 — references preserved

### 2.1 What was recorded, and what was left alone

| item | state |
| --- | --- |
| `oracle/x_ite-pin/` (16.5.0) | **not modified** — read only, verified below |
| `tools/candidate.lock` | **reference target not modified** — still `X_ITE_PIN=34cdf9f97…`, `X_ITE_VERSION=16.5.0`, still `X_ITE_REF_DIR=oracle/x_ite-pin/src`. Its two *stale measurements* are re-blessed by this PR: `X_ITE_REF_FILES` 854 → 857 and `X_ITE_REF_DIGEST` `88c5d8da…` → `83fb7cae…`, with the cause recorded in the file. See §2.3 |
| `crates/x3d-scene/data/x_ite-nodes.json` | **not modified** — still `"pinned":"34cdf9f97","browserVersion":"16.5.0"` |
| every 16.5.0 oracle capture under `oracle/*-baseline*` | **not modified** |
| new | `oracle/x_ite-pin-16.6.0/` — inert identity record, read by nothing |

### 2.2 16.5.0 pin integrity (proof it is unchanged)

`oracle/x_ite-pin/src` was byte-compared against `git archive 34cdf9f97 src/`:

* 846 upstream files — **all identical**, no additions, no deletions, no content drift;
* plus exactly 11 files that are **ours, not upstream's**: `anim.html`, `anim-msaa.html`,
  `anim-sized.html`, `dump.html`, `extern.html`, `inline.html`, `nodes.html`, `shot.html`,
  `shot-log.html`, `shot-msaa.html`, `zfight.html` — the oracle harness pages.

Total 857 files.

### 2.3 Pre-existing defect: `candidate.lock` reference digest is stale

```
tools/candidate.lock   X_ITE_REF_FILES=854   X_ITE_REF_DIGEST=88c5d8da0abc1463afd38c33898836cfff978e48a1af104936dea7fb33f5df2c
measured now           857                   83fb7cae30ef3a9a5040acbe463024f72683ecde29d70c0d68a06165eab8506f
```

`tools/verify-candidate.sh` is fail-closed on this value, so it refuses today with
`X_ITE source digest is '857 83fb7cae…', lock says '854 88c5d8da…'`.

**Cause, proven (and larger than first reported).** An earlier pass of this audit recorded
the cause as "three harness pages were added." That is incomplete. Excluding only the three
added pages reproduces the file *count* (854) but **not** the digest
(`8957c0d1…`, not `88c5d8da…`), which proves something inside the original 854 also changed.

`git diff --stat 02361e2 HEAD -- oracle/x_ite-pin/src` gives the full cause — **nine files,
533 insertions, 0 deletions**, every one of them *ours*:

| file | change |
| --- | --- |
| `extern.html`, `inline.html`, `nodes.html` | **added** after the lock (854 → 857) |
| `anim.html`, `anim-msaa.html`, `anim-sized.html`, `dump.html`, `shot.html`, `shot-msaa.html` | **modified** after the lock |

No upstream file was added, removed or altered. Independently re-proved for this report:
`diff -r` of `oracle/x_ite-pin/src` against `git archive 34cdf9f97 src/` reports **zero**
content differences and only the 11 harness pages as extra (§2.2).

**The real defect is the digest's scope, not the digest's value.** `X_ITE_REF_DIR` is
`oracle/x_ite-pin/src`, which holds upstream's 846 files *and* our 11 mutable QA harness
pages. The gate therefore breaks every time we edit a harness page — a file upstream has
never seen. That is why it drifted, and re-blessing the value alone guarantees it drifts
again. Recommended durable fix, for owner approval: exclude the 11 harness pages from the
digest (or move them out of `src/`) so the gate covers only the X_ITE reference source.
That changes the gate's scope, so it is **not** done here.

**What this repair does, and where it is.** `X_ITE_REF_FILES` / `X_ITE_REF_DIGEST` are
updated to the proven measured values (857 / `83fb7cae…`), with the cause recorded in the
lock. No oracle source byte was touched. The edit is **in this PR**, in commit
`b7896dc` — `tools/candidate.lock` is the only pre-existing tracked file the PR changes.
Re-measure it with `python3 tools/reference-digest.py oracle/x_ite-pin/src`, which prints
`857 83fb7cae…`; the value was not copied from this report.

**This does not make the gate pass**, and that is a second, independent staleness: the lock
still names `CANDIDATE_COMMIT=a553381b9f4d950d71032096fb2a27d30cd68b93` from R0.1, while
`master` is now `f7a67cdcb817b9d7cecdbcb8419844f252160eec`, so the gate also refuses with
`HEAD … differs … in non-infrastructure file: .claude/skills/port-factory/SKILL.md`. The
R0.1 identity gate has not tracked this repository for many lanes and **nothing in CI runs
it** (`.github/workflows/rust-tests.yml` runs `cargo fmt --all -- --check` and
`cargo test --workspace --release`). Deciding whether to retire the gate or re-base it onto
current `master` is an owner call and is out of scope here.

### 2.4 The new 16.6.0 reference pin

`oracle/x_ite-pin-16.6.0/`

| file | content |
| --- | --- |
| `SOURCE_SHA` | `5f282709c124ffa4e2bbd57d9b199893bf94bd32` (full, exact) |
| `PINNED_COMMIT` | `5f282709c` (9-char prefix, matching the active pin's convention) |
| `package.json` | upstream's, verbatim at that commit — declares `16.6.0` |
| `BROWSER_VERSION.js` | upstream's, verbatim — `16.6.0` |
| `MANIFEST.sha256` | per-file SHA-256 of all 846 `src/` blobs; manifest SHA-256 `ef02ce267103193df166cff0e6f1875c9a50c0ff5d595aa629028a56e0185542` |
| `README.md` | states it is inert, and how to materialize the tree |

**`src/` is now materialized** (the B25-2/16.6.0 handoff of 2026-10-24 authorized it:
"Materialize the new upstream source in its separate directory"). 846 files, 15 MB,
extracted with `git archive 5f282709c src/` and left unmodified.

Verification run for this report:

| check | result |
| --- | --- |
| `src/` file count | **846**, matching the manifest's 846 entries |
| `sha256sum -c MANIFEST.sha256` from the extracted tree | **846 OK, 0 failed** |
| `sha256sum -c MANIFEST.sha256` from the committed `oracle/x_ite-pin-16.6.0/src` | **846 OK, 0 failed** |
| `tools/reference-digest.py oracle/x_ite-pin-16.6.0/src` | `846 1972df1f225b620acf979cde1fac1da29f11cda05b7894a10cc4832fc5960891` |
| files present in 16.6.0 `src/` but absent from the pin | **none** |

The pin stays **inert**: `tools/candidate.lock` still names `oracle/x_ite-pin/src` as
`X_ITE_REF_DIR`, so the active oracle reference is unchanged at 16.5.0 / `34cdf9f97`. The
11 harness pages are *not* copied in — a runnable 16.6.0 harness needs them re-checked
against 16.6.0's `src/x_ite.*` entry points first, which belongs to the Phase 4 oracle lane.

---

## 3. Phase 2 — full source audit, 16.5.0 → 16.6.0

### 3.1 Scope of the delta

49 commits. 21 files changed under `src/`, +599 −308. `dist/` is regenerated output of the
same change and is not treated as separate features. Outside `src/` and `dist/`: a new
4708-line `package-lock.json`, `package.json` dependency bumps, README version strings,
`.gitignore`, `.vscode/settings.json`, `tests/README.md`.

One file — `src/x_ite/Components/Rendering/X3DGeometryNode.js` — was touched twice
(`d073b82e1` added a second `getCoord`, `ccb107f52` removed it) and is **net-identical**
between the two tags.

### 3.2 HAnim and skinning — the dominant change

The architecture of skinning moved. In 16.5.0, displacer/skin state lived on the
`HAnimSegment` node and the renderer learned which HAnim node a shape belonged to from a
**traversal stack** (`renderObject.getHAnimNode()`). In 16.6.0 that state lives on the
**`X3DCoordinateNode`** and the renderer derives it from the shape's **own geometry's
coordinate node**.

| # | behavior | class | files | commits |
| --- | --- | --- | --- | --- |
| H1 | Skin/displacer state (`numJoints`, `numDisplacements`, `displacementWeights`, the three GL textures, `jointMatricesArray`, `getHAnimKey`, `getNumDisplacements`, `initHAnimDisplacers`, `add/removeHAnimDisplacers`, `set_displacement*Texture__`, `getHAnimShaderOptions`, `setHAnimShaderUniforms`, `skinning`, `updateSkin`) moved from `HAnimSegment.prototype` onto `X3DCoordinateNode.prototype`, monkey-patched by `HAnimSegment.js` as an import side effect. | **changed behavior** | `HAnim/HAnimSegment.js` | `0431d5e15`, `cd5826c47`, `990c49e59`, `39a59e28c`, `f537f1baf`, `bf46eb285` |
| H2 | One `Coordinate`/`CoordinateDouble` can now be shared by **many** `HAnimSegment` / `HAnimSite` nodes. The coord keeps a `Set` of joint nodes; `jointMatricesArray` is sized `ceil(sqrt(jointNodes.size * 8))²·4` and each joint writes its own 32-float slot. In 16.5.0 a segment had exactly one slot (joint 0). | **new behavior** | `HAnimSegment.js`, `HAnimSite.js` | `990c49e59`, `0431d5e15` |
| H3 | `HAnimSite` gained displacer/skinning behavior it did not have at all: `initialize`, `set_displacers__`, `set_coord__`, `getJointMatrix`, `getHAnimKey`, `getDisplacers`, its own `traverse`/`groupTraverse`, `getShaderOptions`, `setShaderUniforms`. In 16.5.0 `HAnimSite` was a bare `X3DTransformNode` (43 lines → 196). | **new behavior** | `HAnimSite.js` | `a989d70db`, `990c49e59`, `e0314833e`, `f537f1baf` |
| H4 | `HAnimSite` gained four fields: **`purpose`** `SFString` (after `name`), and **`meshVertexIndex`** `SFInt32(-1)`, **`displacers`** `MFNode`, **`coord`** `SFNode` (after `center`, before `visible`). **Field indices shift** for everything from `visible` onward. | **new behavior** | `HAnimSite.js` | `a989d70db`, `c2afdb4d1` |
| H5 | `HAnimHumanoid` gained a **`displacers`** `MFNode` field, inserted after `viewpoints` and before `children`. **Field indices shift** for `children` onward. | **new behavior** | `HAnimHumanoid.js` | `bf8b3ed6f` |
| H6 | `HAnimHumanoid.skeletonNode` changed from `Group` to **`HAnimJoint`**, is fed the humanoid's `displacers`, and is pushed as **`jointNodes[0]`** with an identity binding matrix. Every real joint index shifts by +1 and the joint-matrix texture gains one slot. Numerically inert for existing content (the skeleton node has no `skinCoordIndex`), but it is the mechanism that makes H5 work, and `jointNodes` is also what is handed to `HAnimPose.addJoints` and `HAnimMotion.connectJoints`. | **new behavior** | `HAnimHumanoid.js` | `40646722b`, `bf8b3ed6f` |
| H7 | `HAnimHumanoid.set_jointTextures__` skin-weight guard changed from `weight === 0` to `!weight`, so a `NaN`/`undefined` weight (an index past `skinCoordWeight`) is now skipped instead of contributing `NaN`. | **defect fix** | `HAnimHumanoid.js` | `4b2937df8`, `ade87147a` |
| H8 | `HAnimJoint.allowedTypes` widened to include **`X3DGroupingNode` and `Shape`**. A `Shape`, `Group` or `Transform` placed directly in an `HAnimJoint`'s `children` is now rendered; in 16.5.0 it was silently dropped. Applies at **every** specification version, VRML97 included. | **changed behavior** | `HAnimJoint.js` | `a989d70db` |
| H9 | `HAnimSegment` now *calls* `addAllowedTypes(HAnimSite, X3DGroupingNode)`, plus `X3DChildNode` only when `getSpecificationVersion() <= 4.0`. In 16.5.0 its `allowedTypes` was empty, i.e. everything was allowed. So for **X3D 4.1 and later** a `Shape` directly in an `HAnimSegment`'s `children` is now silently dropped. VRML97 (2.0) and X3D ≤ 4.0 are unaffected. | **changed behavior** | `HAnimSegment.js` | `a989d70db` |
| H10 | `HAnimJoint._llimit` / `._ulimit` now carry the **`angle` unit**, so `UNIT angle` declarations convert them on parse and on serialization. The 16.5.0 code had this commented out with a `TODO`. | **defect fix** | `HAnimJoint.js` | `8175b0f03` |
| H11 | `HAnimSegment`/`HAnimSite` joint matrix is now computed once per DISPLAY traversal into `this.jointMatrix` and the **texture upload is deferred and deduplicated** (`updateSkin()` sets a dirty flag, `skinning()` uploads at most once from `setHAnimShaderUniforms`). 16.5.0 uploaded in the traversal itself, every traversal. | **changed behavior** (timing/efficiency; same values) | `HAnimSegment.js`, `HAnimSite.js` | `e0314833e`, `990c49e59` |
| H12 | `HAnimSegment`'s own field definitions and the `export default` moved below the new `X3DCoordinateNode.prototype` assignment. The **field list itself is identical** to 16.5.0. | **refactor, no behavior change** | `HAnimSegment.js` | `990c49e59` |
| H13 | `HAnimJoint` has a dead branch: `else if (getSpecificationVersion() <= 4.0) addAllowedTypes(HAnimJoint, HAnimSegment)` re-adds types the constructor already added. | **upstream defect, inert** — record as reference behavior, do not "fix" | `HAnimJoint.js` | `a989d70db` |
| H14 | `HAnimSite.traverse` tests `this.coordNode?.numDisplacements` (raw property) while `HAnimSite.groupTraverse` and `HAnimSegment.traverse` test `getNumDisplacements()`. Equivalent today because the getter just returns the property, but the inconsistency is upstream's. | **upstream defect, inert** — record as reference behavior | `HAnimSite.js` | `f537f1baf` |

### 3.3 Rendering and scene traversal — how a shape learns it is skinned

| # | behavior | class | files | commits |
| --- | --- | --- | --- | --- |
| R1 | `X3DRenderObject.hAnimNode` (a traversal stack) and `getHAnimNode()` were **deleted**. All four shape collectors now take a second argument and compute `renderContext.hAnimNode = geometryContext.getCoord()?.[Symbol.for("X_ITE.HAnim.hAnimNodes")]?.at(-1)`. | **changed behavior** | `Rendering/X3DRenderObject.js`, `Shape/X3DShapeNode.js` | `d073b82e1`, `c7a928df3`, `baf9eedc5`, `5932b7b72`, `39a59e28c`, `391af57f3` |
| R2 | `addPointingShape`, `addCollisionShape`, `addDepthShape`, `addDisplayShape` signatures changed to `(shapeNode, geometryContext)`. `X3DShapeNode.traverse` passes the context it already resolved (and already guards `!geometryContext`). | **changed behavior (API)** | same | `d073b82e1` |
| R3 | `GeometryContext` gained `getCoord()` returning **`null`**. `GeometryContext` is the stand-in used by `ParticleSystem`, `GaussianSplatsShape`, `X3DBackgroundNode`, `ScreenLine`, `ScreenPoint`. Consequence: **those shapes can never be skinned in 16.6.0**, where in 16.5.0 they inherited whatever HAnim node was on the stack. | **changed behavior** | `Browser/Rendering/GeometryContext.js` | `d073b82e1` |
| R4 | Real geometry nodes return `this.coordNode` from `getCoord()`. Only geometries that *have* a `coord` field set it. So **geometry primitives** (`Box`, `Sphere`, `Cone`, `Cylinder`, `Text`, `Extrusion`, `ElevationGrid`, the 2D set) now resolve `getCoord()` to `undefined` and are **never skinned**. In 16.5.0 a `Box` inside an `HAnimSegment` with displacements *was* fed the segment's displacement tables. | **changed behavior** | consequence of R1 | `d073b82e1` |
| R5 | A shape inside an HAnim subtree whose geometry uses a **different** `Coordinate` than the segment/site's `coord`, or a shape in the `skin` group whose geometry does not use `skinCoord`, is no longer skinned. 16.5.0 skinned it with the wrong node's tables. | **defect fix** | consequence of R1 | `d073b82e1` |
| R6 | The four collectors replaced `const num = this.numXShapes++; if (num === arr.length) arr.push(ctx); const ctx = arr[num];` with `arr[this.numXShapes++] ??= {…}`. Same pooling semantics, same reuse. | **refactor, no behavior change** | `X3DRenderObject.js` | `e7a09c520`, `36be01a69` |

### 3.4 Geospatial coordinates

| # | behavior | class | files | commits |
| --- | --- | --- | --- | --- |
| G1 | `X3DGeospatialObject.getCoord(geoPoint, result)` renamed to **`getLocalCoord`**, and the rotate-Y-up override `getCoordRotateYUp` to `getLocalCoordRotateYUp`. All six call sites updated (`GeoCoordinate` ×3, `GeoElevationGrid` ×2, `GeoLOD`, `GeoPositionInterpolator` ×2, `GeoViewpoint` ×4). | **refactor, no geospatial behavior change** — but see G2 | `Geospatial/{GeoCoordinate,GeoElevationGrid,GeoLOD,GeoPositionInterpolator,GeoViewpoint,X3DGeospatialObject}.js` | `c17d40997` |
| G2 | The rename exists to **de-collide** with `X3DGeometryNode.getCoord()` / `X3DCoordinateNode`, which R1 now calls on every shape. `GeoCoordinate` is both an `X3DCoordinateNode` and an `X3DGeospatialObject`; `GeoElevationGrid` is both an `X3DGeometryNode` and an `X3DGeospatialObject`. Without the rename, R1 would have invoked the geospatial converter with `(undefined, undefined)`. | **defect fix enabling R1** | same | `c17d40997`, `ccb107f52` |
| G3 | `X3DGeospatialObject(executionContext)` → `X3DGeospatialObject(/* executionContext */)`; the parameter was unused. | **refactor, no behavior change** | `X3DGeospatialObject.js` | `de624ef58` |
| G4 | The duplicate `getCoord` added to `X3DGeometryNode.js` by `d073b82e1` was removed again. Net identical to 16.5.0. | **refactor, net zero** | `Rendering/X3DGeometryNode.js` | `ccb107f52` |

### 3.5 Imported nodes and proxy behavior

| # | behavior | class | files | commits |
| --- | --- | --- | --- | --- |
| I1 | `X3DImportedNodeInstance` constructor now **returns a `Proxy`** with `get`/`set`/`has`/`ownKeys`/`getOwnPropertyDescriptor` traps. Anything not present on the instance falls through to `importedNode.getExportedNode()`. So `instance._translation`, `instance.translation`, user-defined fields, and arbitrary properties on the exported node are now reachable and assignable through the imported instance; in 16.5.0 they were `undefined`. `set` returns `false` (→ `TypeError` in strict mode) when there is no exported node. | **new behavior** | `Core/X3DImportedNodeInstance.js` | `2d9d97a08`, `220fda493`, `7f9fb86ea` |
| I2 | **`getType` was removed from the explicit forwarding list.** `getType()` on an imported instance now returns the instance's *own* type array (`…, X3DNode, X3DImportedNodeInstance`) instead of the exported node's. This changes `X3DCast` results and any SAI `getType()` probe on an imported node. The other 14 forwarded methods (`getTypeName`, `getComponentInfo`, `getContainerField`, `getSpecificationRange`, `getFieldDefinitions`, `getPredefinedField(s)`, `getUserDefinedField(s)`, `getField(s)`, `getChangedFields`, `isDefaultValue`, `hasRoutes`) are unchanged. | **changed behavior** | same | `373fb1152` |
| I3 | `ownKeys`/`getOwnPropertyDescriptor` merge the exported node's keys, so `Object.keys`, `for…in`, spread and `JSON.stringify` over an imported instance enumerate differently. | **changed behavior** | same | `2d9d97a08` |

### 3.6 Browser and scene services

| # | behavior | class | files | commits |
| --- | --- | --- | --- | --- |
| S1 | `X3DScene.addRootNode` / `removeRootNode` now tolerate **`null`**: `node = X3DCast(…); node &&= SFNodeCache.get(node);`. In 16.5.0 `SFNodeCache.get(null)` was called unconditionally. `x_ite.d.ts` was updated to `SFNode \| null`, and `removeRootNode`'s doc corrected to "Removes **all occurrences** of *node*". | **defect fix** | `Execution/X3DScene.js`, `x_ite.d.ts` | `1444e40e3`, `094e1953d`, `086d2c80a` |
| S2 | `Inline` now truncates **`renderObject.getGlobalLightsKeys()`** back to `globalsBegin` alongside `globalLights` and `globalShadows` when unwinding its local-light scope. In 16.5.0 the keys array kept growing, so the shader cache key for shapes after an `Inline` was wrong (and unboundedly long). | **defect fix** | `Networking/Inline.js` | `0c5bca28a` |

### 3.7 Shaders, textures, geometry, dependencies, decoders

| # | behavior | class | files | commits |
| --- | --- | --- | --- | --- |
| P1 | glTF skeleton import: a non-`HAnimJoint` child of a joint is wrapped in an **`HAnimSite`** instead of an `HAnimSegment`. | **changed behavior** | `Parser/GLTF2Parser.js` | `77b4dad24`, `048fe5b5b`, `f5ba8a914` |
| P2 | Vendored `src/assets/lib/draco_decoder_gltf.js` updated (30 lines); `draco3d ^1.5.9` added as a devDependency. | **unknown** — decoder output not compared; upstream published no changelog for it | `src/assets/lib/draco_decoder_gltf.js`, `package.json` | `b6f71588a`, `0e3fc636c` |
| P3 | `eslint ^10.12.0 → ^10.13.0`, `webpack ^5.111.1 → ^5.112.0`. | **refactor, no runtime behavior** | `package.json` | `0e3fc636c`, `414f0dcf6` |
| P4 | A committed `package-lock.json` appeared (4708 lines), and `.gitignore` / `.vscode/settings.json` no longer exclude it. This **invalidates the X_ITE lane note that `npm ci` fails by design**; from 16.6.0 on, `npm ci` is the reproducible install. | **new behavior (build)** | `package-lock.json`, `.gitignore` | `2cf1a64a2`, `f4990f5ab` |
| P5 | `src/tests.js` gained `HAnim/HAnimSegment.x3d`; `tests/README.md` and `README.md` version strings updated; a docs sample viewer line changed. | **no behavior change** | `src/tests.js`, docs | `e34bad5e6`, `778d456c4`, `b4e3be24c` |
| P6 | No shader source, texture pipeline, or geometry-generation file changed between the two tags beyond what §3.2–3.4 lists. `Skin2.glsl` is unchanged; only the *host* code that feeds it moved. | — | — | — |

### 3.8 The 8 commits the brief's range omits (pin → tag 16.5.0)

14 `src/` files, +28 −20.

| # | behavior | class | files |
| --- | --- | --- | --- |
| X1 | `advanceTime` gained a **`checkFrameRate`** argument. `X3DBrowserContext` now calls `advanceTime(now >= 0)` and bails on a falsy return, where 16.5.0-pin called `advanceTime()` and bailed only `if (!result && now >= 0)`. Net: on a reshape (`now < 0`) the frame-rate gate is skipped and **time is now advanced**, where before it was not. | **changed behavior** | `Browser/Time/X3DTimeContext.js`, `Browser/X3DBrowserContext.js` |
| X2 | The `s` (dotAll) regex flag was dropped from 11 patterns. For patterns with no `.` this is a no-op. **One is not:** `FileLoader`'s `/^\s*data:(.*?)(?:;charset=(.*?))?(?:;(base64))?,/` no longer matches a `data:` URL whose header contains a newline before the comma. | mostly **refactor**; the `FileLoader` one is a **changed behavior** | `Components/Core/X3DNode.js` ×2, `CubeMapTexturing/ImageCubeMapTexture.js`, `HAnim/HAnimMotion.js` ×4, `Texturing/ImageTexture.js`, `Texturing/MovieTexture.js` ×2, `Texturing3D/ImageTexture3D.js`, `InputOutput/FileLoader.js` ×2, `InputOutput/JSONGenerator.js`, `Parser/GoldenGate.js` |
| X3 | `X3DCoreContext` skips the `x_ite.css` `integrity` attribute when the new `DEVELOPMENT` flag is set. | **new behavior (dev only)** | `Browser/Core/X3DCoreContext.js` |
| X4 | Comment-only edits. | **no behavior change** | `Browser/X3DBrowser.js`, `Parser/GoldenGate.js`, `Parser/VRMLParser.js` ×2 |

---

## 4. Phase 3 — Rust implementation map

Every entry below was read in the current tree at `f7a67cd`; none is implemented.

| ID | Rust crate / file | current state | required change |
| --- | --- | --- | --- |
| H1, H2 | `crates/x3d-render/src/hanim.rs` (`SkinTables`, `segment_tables`, `segment_matrices`, `humanoid_tables`) | **exactly 16.5.0.** `segment_tables(sg, segment)` is keyed on the *segment*; `segment_matrices` documents "joint 0 is the segment's model-view × the inverse humanoid model-view" — one slot, per segment. | re-key the tables on the **coordinate node**, keep an ordered set of contributing segment/site nodes per coord, size the matrix array to that set, and emit one slot per contributor. |
| H3, H4 | `crates/x3d-render/src/sg.rs:399` (`"HAnimSegment" \| "HAnimSite" => all children`), `crates/x3d-render/src/traverse.rs:1782-1796` | `HAnimSite` is treated as a plain transform; there is **no** site displacer path. | give `HAnimSite` the segment's traverse/skin path; add the four new fields to the node table. |
| H4, H5, H10 | `crates/x3d-scene/data/x_ite-nodes.json` | pinned `34cdf9f97` / `16.5.0`. `HAnimSite` has 15 fields, no `purpose`/`meshVertexIndex`/`displacers`/`coord`. `HAnimHumanoid` has no `displacers`. `HAnimJoint`'s `units` map has `minAngle`/`maxAngle` but **not** `llimit`/`ulimit`. | regenerate from the 16.6.0 pin. Field-order changes shift indices, so SAI field-definition evidence must be re-baselined. |
| H6 | `crates/x3d-render/src/hanim.rs:51` `humanoid_joints`, `crates/x3d-runtime/src/nodes/hanim.rs:349` `hanim_humanoid_set_joints`, `:765` `hanim_motion_connect`, `:502` `hanim_pose_add_joints` | joint list is exactly `_joints`; no skeleton node is prepended. | prepend a synthetic skeleton joint carrying `HAnimHumanoid.displacers` with an identity binding; confirm `HAnimPose`/`HAnimMotion` remain name-keyed (upstream's `getJointsIndex` is, so a nameless joint 0 should be inert — must be proven, not assumed). |
| H7 | `crates/x3d-render/src/hanim.rs:155-157` | deliberately reproduces the 16.5.0 defect: *"An index past `skinCoordWeight` reads undefined: not 0, so NaN in the Float32Array."* | match upstream's `!weight` **exactly** — skip a weight that is absent, `0.0`, `-0.0` or `NaN`, and **keep** `±∞` — then delete that comment. See the value table below: `!weight` is *not* "skip non-finite", because `!Infinity` is `false`. One line, but an `is_finite()` guard would be a new deviation rather than parity. |
| H8, H9 | `crates/x3d-render/src/sg.rs:371-402` `hanim_children` | exactly 16.5.0: joint children filtered to `HAnimJoint`/`HAnimSegment` (+`HAnimSite` for spec ≤ 3.3); segment/site children unfiltered. | add `X3DGroupingNode`/`Shape` to the joint filter; add the spec-gated `{HAnimSite, X3DGroupingNode}` (+`X3DChildNode` ≤ 4.0) filter to the segment. Needs an `is_a`-style type-hierarchy test, not a flat name list. |
| H11 | `crates/x3d-render/src/traverse.rs:3777-3796` `hanim_segment` | builds matrices eagerly per traversal. | add the dirty-flag/deferred-upload equivalent. Low priority: values identical. |
| R1–R5 | `crates/x3d-render/src/traverse.rs:1342` (`hanim: Vec<Option<usize>>`), `:1538`, `:3726`, `:3792`, `:4213` | the `hanim` stack is the 16.5.0 design, verbatim. | replace the stack lookup with a per-shape "does this geometry's coord node have a live HAnim registration" lookup. **Primitives and particle/background/screen shapes must stop being skinned.** |
| R2 | `crates/x3d-render/src/traverse.rs` display/depth/collision/pointing shape collectors | the Rust collectors already carry the geometry per shape, so the signature change is internal; no public API break expected. | confirm the geometry handle available at each collector exposes the coord node. |
| R3 | `crates/x3d-render/src/traverse.rs:796` (particle instances), particle pointing/depth paths | particle shapes are inside the HAnim stack like any other. | exclude them, matching `GeometryContext.getCoord() → null`. |
| G1–G4 | `crates/x3d-runtime/src/geo.rs:538` `get_coord`, `crates/x3d-render/src/sg.rs:360` | Rust has no name collision (`geo.get_coord` is a distinct type's method), so the rename needs **no Rust change**. | none. Verify geospatial oracle output is byte-identical across 16.5.0/16.6.0 and record that as evidence the rename is inert. |
| I1–I3 | `crates/x3d-scene/src/scene.rs`, `crates/x3d-script/src/engine.rs` (+ `ctx_api.rs`) | imported-node instances are modeled structurally; there is no property-forwarding proxy. | decide how much of the Proxy surface to reproduce. I2 (`getType`) is small and observable. I1/I3 (arbitrary property forwarding, `ownKeys`) are a JavaScript-host feature with no natural Rust analogue — likely a **documented deviation**, not an implementation. |
| S1 | `crates/x3d-script/src/engine.rs:2909` | null handling not verified in this audit. | add a `null` case to `addRootNode`/`removeRootNode` and a "removes all occurrences" check. |
| S2 | `crates/x3d-render/src/traverse.rs` (`global_lights`, `local_lights`) | **there is no `globalLightsKeys` analogue at all** — Rust tracks light indices and has no string shader-cache key. | probably **no Rust change**; the fix removes a JS-only cache-key leak. Must still be proven not to change rendered output in an Inline-with-local-lights oracle case. |
| P1 | `crates/x3d-parse/src/gltf2/scenegraph.rs:519` `self.create("HAnimSegment")` | exactly 16.5.0. | change to `HAnimSite`. One-line change, but it alters every imported-skeleton scene's text, so it is gated on new glTF oracle text. |
| P2 | — | Rust rejects/loads Draco differently already (`sub:parse.gltf2` records `KHR_draco_mesh_compression skipped`). | classify after comparing decoder output; may be a no-op for X_ITE_R. |
| X1 | `crates/x3d-viewer/src/viewer.rs:1110` `frame_due`, `:1124` `step`, `crates/x3d-viewer/src/embed.rs:515` `step`, `:548` `frame` | **located (§8.1).** There is no `now < 0` reshape sentinel in the port: `Embed::frame` resizes and then takes a normal gated step, so a resize inside the frame-rate window advances no time. Matches 16.5.0, diverges from 16.6.0 in exactly that window. | let the step after a `resize` bypass `frame_due`. Gated on the `reshape-advance-time` capture (§5.2). |
| X2 | `crates/x3d-runtime/src/loader.rs:138` `data_url_text` | **located (§8.2).** `split_once (',')` is newline-blind, so a multiline `data:` header is still accepted: matches 16.5.0, diverges from 16.6.0. Sole `data:` parse site in the tree. | reject a header that spans a line break. Gated on a capture that fixes the break set (JS `.` excludes `\n`, `\r`, `\u2028`, `\u2029`, unlike Rust `lines()`). |

**H7 in full, because `!weight` is easy to approximate wrongly.** `_skinCoordWeight` is an
`MFFloat`, which `ArrayFields.js` builds from `TypedArrayTemplate (… Float32Array, 1)`, so
`jointNode ._skinCoordWeight .getValue () [i]` is either an `f32` or `undefined` past the
end. Over exactly that domain, JavaScript's `!weight` is:

| `skinCoordWeight [i]` | `!weight` | 16.6.0 | 16.5.0 (`weight === 0`) |
| --- | --- | --- | --- |
| `0` | `true` | **skipped** | skipped |
| `-0` | `true` | **skipped** | skipped (`-0 === 0`) |
| `NaN` | `true` | **skipped** | contributed → `NaN` in the texture |
| `undefined` (index past `skinCoordWeight`) | `true` | **skipped** | contributed → `NaN` in the texture |
| `Infinity`, `-Infinity` | `false` | **contributed** | contributed |
| any other non-zero finite value | `false` | contributed | contributed |

So the 16.6.0 fix adds `NaN` and absent to the skip set and nothing else. An infinite weight
still reaches the joint texture in both versions. The Rust guard that reproduces this is
`weight == 0.0 || weight.is_nan()` plus the absent case (the index-past-the-end read, which
in Rust is a `get (i)` that returns `None`, not a `NaN`) — **not** `!weight.is_finite()`,
which would additionally drop `±∞` and make X_ITE_R diverge from both versions. The
`hanim-weight-nan` oracle case (§5.2) should therefore capture an infinite weight as well as
a missing one, so the distinction is held by evidence and not only by this table.

**Not implemented here.** H7 belongs to lane L3 (§6.1); this PR changes no Rust line.

### 4.1 REA ledger comparison

`_batch09_context/ledger/ledger.json` (346 items; 249 COMPLETE, 70 PARTIAL, 25 DIFFERENT,
2 MISSING). Every entry's `upstream_version` is `16.5.0 @ 34cdf9f97`.

| ledger entry | status today | after 16.6.0 |
| --- | --- | --- |
| `node:HAnimHumanoid` | COMPLETE | **regresses** — H5, H6, H7 unimplemented |
| `node:HAnimDisplacer` | COMPLETE | **regresses** — H1, H2, H3 move where displacers live |
| `node:HAnimJoint` | PARTIAL | stays PARTIAL, plus H8, H10 |
| `node:HAnimSegment` | PARTIAL | stays PARTIAL, plus H1, H2, H9, H12 |
| `node:HAnimSite` | PARTIAL | **largest single gap** — H3, H4 are an entirely new node behavior |
| `node:Coordinate` | COMPLETE | **regresses** — H1 puts skin state on this node |
| `node:CoordinateDouble` | PARTIAL | same, via H1 |
| `node:GeoCoordinate`, `GeoElevationGrid`, `GeoLOD`, `GeoPositionInterpolator`, `GeoViewpoint` | all COMPLETE | **stay COMPLETE** — G1 is inert for Rust (needs proof, §5.4) |
| `sub:exec.import-export` | COMPLETE | **regresses** — I1, I2, I3 |
| `sub:exec.scene` | COMPLETE | **regresses** — S1 |
| `node:Inline` | PARTIAL | S2 is likely inert for Rust; must be proven |
| `sub:parse.gltf2` | PARTIAL | plus P1 |

**No entry may be re-marked COMPLETE without 16.6.0 test evidence.** The ledger's
`upstream_version` field must be bumped per entry as each is migrated, not globally — a
global bump would assert coverage that does not exist.

Ledger gaps with no entry at all, which 16.6.0 makes material:

* no entry covers `X3DRenderObject`'s hAnim-node resolution as a subsystem (it is folded
  into the HAnim node entries), so R1–R5 have no home;
* no entry covers `GeometryContext` or the "which shapes can be skinned" rule;
* no entry covers `advanceTime`/frame pacing (X1).

---

## 5. Phase 4 — new oracle evidence required

**Nothing was captured. The active oracle version is unchanged.** These are requirements.

### 5.1 Prerequisites, in order

1. ~~Fix the stale `tools/candidate.lock` digest (§2.3)~~ — **done** (2026-10-24). The
   digest is re-blessed to the proven `857 / 83fb7cae…` and the cause is recorded in the
   lock. Note the gate still refuses for an unrelated reason (the R0.1 `CANDIDATE_COMMIT`);
   see §2.3.
2. ~~Materialize `oracle/x_ite-pin-16.6.0/src`~~ — **done** (2026-10-24). 846 files,
   `sha256sum -c MANIFEST.sha256` → 846 OK / 0 failed. See §2.4.
3. ~~Give the 16.6.0 pin an automated integrity check~~ — **done** (2026-10-24).
   `tools/verify-reference-pin.py` verifies all 846 `src/` files against
   `MANIFEST.sha256`, refuses an unlisted extra file, re-computes the recorded tree
   digest, and checks `SOURCE_SHA`, `PINNED_COMMIT`, `package.json` and both
   `BROWSER_VERSION.js` copies against the identity it expects. It runs in CI before the
   toolchain step, together with `--self-test`, which proves on a throwaway copy that all
   eight tamper cases still refuse. Pure `hashlib`: no GNU `sha256sum` dependency.
4. Copy the 11 harness pages into the 16.6.0 tree and re-verify each against 16.6.0's
   `src/x_ite.*` entry points. They are ours, so they are not in `MANIFEST.sha256`.
   **Still open** — this is what makes the 16.6.0 tree *runnable* rather than merely
   verified, and it is the first task of the Phase 4 oracle lane.
5. Keep both trees live simultaneously. Every case below must be recorded from **both**
   16.5.0 and 16.6.0, three identical recordings each, before any Rust line changes.

### 5.2 New cases, by behavior

| case | covers | what to capture |
| --- | --- | --- |
| `hanim-site-displacers` | H3, H4 | an `HAnimSite` with `coord` + `displacers`, inside and outside a humanoid; scene text, log, images MSAA 0/4 |
| `hanim-shared-coord` | H2 | two `HAnimSegment`s **and** an `HAnimSite` sharing one `Coordinate`; proves per-contributor matrix slots and the array sizing |
| `hanim-humanoid-displacers` | H5, H6 | `HAnimHumanoid.displacers` driving skin deformation via joint 0 |
| `hanim-joint-children` | H8 | `Shape`, `Group`, `Transform` directly in an `HAnimJoint`'s `children`, at VRML97, X3D 3.3, 4.0 and 4.1 |
| `hanim-segment-children-spec` | H9 | `Shape` directly in an `HAnimSegment`'s `children` at VRML97, 3.3, 4.0 **and 4.1** — the 4.1 case must show it dropped |
| `hanim-weight-nan` | H7 | `skinCoordIndex` longer than `skinCoordWeight`; 16.5.0 must show the NaN artifact, 16.6.0 must not |
| `hanim-joint-limit-units` | H10 | `UNIT angle degree` + `llimit`/`ulimit`; scene text before and after |
| `hanim-primitive-skin` | R4 | a `Box` and a `Text` inside an `HAnimSegment` with displacements; 16.5.0 skins them, 16.6.0 does not |
| `hanim-particles-skin` | R3 | a `ParticleSystem` and a `Background` inside a humanoid's skin group |
| `hanim-foreign-coord` | R5 | a `Shape` in a segment whose geometry uses a *different* `Coordinate` |
| `imported-proxy` | I1, I2, I3 | SAI probes: `getType()`, direct field access (`n.translation`, `n._translation`), a user-defined field, `Object.keys`, `for…in`, `JSON.stringify`, assignment with and without a loaded Inline |
| `scene-rootnode-null` | S1 | `addRootNode(null)`, `removeRootNode(null)`, and removing a node present twice |
| `inline-local-lights-keys` | S2 | `Inline` with local lights followed by more shapes; shader-key-sensitive rendering |
| `gltf-skeleton-site` | P1 | a glTF skin whose joint has a non-joint child; scene text |
| `reshape-advance-time` | X1 | a resize/reshape with a `TimeSensor`; `elapsedTime` across the reshape |
| `dataurl-newline` | X2 | a `data:` URL with a newline in its header |
| existing `hanim-humanoid`, `hanim-sites-33`, `hanim-sites-41` | regression | re-record under 16.6.0 **as new baselines**; keep the 16.5.0 ones |

### 5.3 Known upstream defects to record as reference behavior, not fix

* **H13** — `HAnimJoint`'s dead `else if (… <= 4.0)` branch.
* **H14** — `HAnimSite.traverse` reads `numDisplacements` where its siblings call
  `getNumDisplacements()`.
* **H9 as applied to 4.1** — dropping a `Shape` from an `HAnimSegment` at X3D 4.1 is almost
  certainly not what a content author expects, and it silently deletes geometry. It is
  nonetheless what 16.6.0 does. Reproduce it; do not "improve" it. If it is worth raising,
  that is a separate upstream-issue lane with its own authorization.

### 5.4 Behavior that must be proven *inert* before being skipped

Three items are classified "no Rust change required" on reading alone. Each needs a
16.5.0-vs-16.6.0 oracle comparison showing byte-identical output before that
classification is accepted: **G1** (geospatial rename), **S2** (global lights keys),
**H11** (deferred texture upload).

---

## 6. Phase 5 — delivery

### 6.1 Proposed implementation lanes, hardest first

| lane | scope | why it ranks here |
| --- | --- | --- |
| **L1 — skin ownership moves to the coordinate node** | H1, H2, H3, H4, plus the node-table regeneration | Rewrites `crates/x3d-render/src/hanim.rs` and the `HAnimSite` path in `sg.rs`/`traverse.rs`. Changes a data model, not a value. Touches four COMPLETE/PARTIAL ledger entries. Everything else in HAnim depends on it. |
| **L2 — hAnimNode resolution moves to the geometry** | R1, R2, R3, R4, R5 | Deletes the `hanim` traversal stack in `traverse.rs` and re-derives it per shape. Changes which shapes are skinned at all, so it moves rendered output in ways L1 does not. **Collides directly with `perf/b25-2-gpu-instanced-picking`,** which is rewriting the pointing-shape collector — the same function that gains the `geometryContext` argument. Schedule after B25-2 merges. |
| **L3 — humanoid displacers and the skeleton joint** | H5, H6, H7 | Depends on L1. Shifts every joint index by one and adds a real new feature. H7 is a one-line defect fix that belongs here because it is in the same function. |
| **L4 — allowed-types and spec gating** | H8, H9, H13 | Small code, large blast radius: it changes which children exist in the scene graph, so it moves scene text, bounding boxes, and images for all three existing B04 HAnim cases. Needs a real type-hierarchy test in `sg.rs`, not a name list. |
| **L5 — imported-node proxy** | I1, I2, I3 | Independent of HAnim. I2 is a small observable fix; I1/I3 need a design decision (reproduce vs. document as a deviation) before any code. |
| **L6 — glTF skeleton sites** | P1 | One line in `gltf2/scenegraph.rs`, but it re-baselines every imported-skeleton scene's text across `gltf_rea`'s 287 inputs. Cheap to write, expensive to verify. |
| **L7 — scene and browser services** | S1, S2, X1, X2 | Four unrelated small fixes. X1 and X2 are the ones the brief's 49-commit range would have missed. |
| **L8 — node table and ledger bookkeeping** | regenerate `x_ite-nodes.json` against the 16.6.0 pin, bump `candidate.lock`, bump each ledger entry's `upstream_version` | Must be last, so no entry claims 16.6.0 coverage before its lane lands. |
| **L9 — inert-by-reading confirmations** | G1, S2, H11 (§5.4) | Oracle work only; no Rust change expected. Can run in parallel with anything. |

A note on **GPU-first**: `crates/x3d-render/src/hanim.rs` applies `Skin2.glsl` on the **CPU**
to a copy of the geometry. L1 rewrites exactly that code. If skinning is ever to move to
the GPU, L1 is the cheapest moment to do it — but that is a scope expansion and is not part
of this migration unless you ask for it.

### 6.2 Risks to existing B01–B25 tests

**Confirmed, by reading the oracle inputs:**

* **`oracle/synthetic-rea-b04/hanim-sites-41.wrl`** (`#X3D V4.1`) changes in two opposite
  directions under 16.6.0. Line 11, `Shape { geometry Cone { } }` directly under
  `HAnimJoint`, is dropped at 16.5.0 and **rendered** at 16.6.0 (H8). Line 10,
  `HAnimSegment { … children Shape { … Sphere } }`, is rendered at 16.5.0 and **dropped**
  at 16.6.0 because the spec is 4.1 (H9). Affects `rea_batch04.rs`, `render_rea_b04.rs`,
  `rea_b04_reload.rs`, `rea_b04_render_reload.rs`.
* **`oracle/synthetic-rea-b04/hanim-sites-33.wrl`** (`#X3D V3.3`): line 11's `Cone` is newly
  rendered (H8). The segment's `Shape` survives, because 3.3 ≤ 4.0 (H9).
* **`oracle/synthetic-rea-b04/hanim-humanoid.wrl`** (`#X3D V4.1`): the skinned
  `HAnimSegment` `SEG` at line 22 holds its displaced geometry in a `Shape` child. At X3D
  4.1 that `Shape` is **dropped** under 16.6.0 (H9), so the segment's contribution
  disappears entirely. This is the single most disruptive case in the suite.

**Likely:**

* `crates/x3d-viewer/tests/rea_batch09_sai.rs` `node-hanim` — field definitions, VRML/XML/JSON
  text and per-field event callbacks for every HAnim node. H4, H5 and H10 all change it.
* `crates/x3d-viewer/tests/rea_batch09_sai.rs` `node-rendering` — `Coordinate` /
  `CoordinateDouble` acquire outputOnly `displacementsTexture` / `displacementWeightsTexture`
  fields under H1. Whether those are SAI-visible must be checked; if they are, the field
  inventory changes.
* `gltf_rea` (287 inputs, 980 text checks), `render_rea_b08_gltf`, `gltf_inline_b08`,
  `rea_b08_gltf_reload` — all re-baseline on P1.
* `rea_batch09_sai.rs` `ie-*` (14 imported/exported cases) — I1, I2, I3.
* `oracle/synthetic-rea/sai-scene.wrl` (121 checks, 52 SAI probes) — S1.

**Not expected to move, but must be confirmed (§5.4):** all Geospatial tests
(`rea_batch01.rs`, `rea_batch12.rs`), and anything exercising `Inline` with local lights.

**Particle and B13–B25 tests** are affected only through R3: particle shapes stop
inheriting an HAnim node. No current particle oracle case places a `ParticleSystem` inside
a humanoid, so the expected impact is zero — but R3 is a real semantic change and
`rea_b13`…`rea_b22` plus `particle_pick_viewers.rs` should be re-run as a gate, not assumed.

### 6.3 What this audit did not establish

* **P2**, **X1** and **X2** were open when §4 was written and are **closed in §8**, which
  was added later in the same audit. §4's rows now carry the located Rust counterparts.
  None of the three is implemented; each still needs the capture §8 names.
* No Rust code was built, run, or tested during this audit. No oracle capture was taken, so
  every "16.6.0 differs here" claim in §3 is from reading upstream source, not from a
  recorded comparison. §5 exists to convert them into evidence.

---

## 7. Artifacts

Everything below is **tracked and committed on `ref/x-ite-16-6-0-pin-and-audit`** (PR #44),
not a local artifact.

| path | state |
| --- | --- |
| `oracle/x_ite-pin-16.6.0/` | **added**, 852 files; inert at run time — no crate and no oracle tool reads it |
| `XITE_R_16_6_0_UPSTREAM_PIN_AND_DELTA_AUDIT.md` | **added**, this file |
| `tools/verify-reference-pin.py` | **added** — the pin's integrity check, with `--self-test` (§5.1) |
| `.github/workflows/rust-tests.yml` | **modified** — one step, before the toolchain, running the two commands above |
| `tools/candidate.lock` | **modified** — the stale 16.5.0 measurements only (854 → 857, `88c5d8da…` → `83fb7cae…`); the pinned commit, version and `X_ITE_REF_DIR` are untouched. §2.1, §2.3 |
| `oracle/x_ite-pin/`, `crates/x3d-scene/data/x_ite-nodes.json`, all `oracle/*-baseline*` | **unchanged** — the active 16.5.0 oracle is exactly as it was |
| `~/Projects/cybertown/X_ITE/x_ite` | `git fetch upstream --tags` only; HEAD still on `fix/firefox-vrml-xml-parser` at `34cdf9f97`, working tree clean, nothing pushed |


---

## 8. Closed gaps (2026-10-24)

The B25-2/16.6.0 handoff asked for three specific investigations to be finished. All three
are now closed; none of them changes a Rust line, and none is implemented here.

### 8.1 X1 — frame-time advance during reshape: Rust counterpart located

**Upstream change.** `advanceTime` gained a `checkFrameRate` argument and
`X3DBrowserContext` calls `advanceTime (now >= 0)`. A reshape passes `now < 0`, so in
16.6.0 the frame-rate gate is skipped and time **is** advanced; the 16.5.0 pin ran the gate
on a reshape and so did **not** advance time when the gate said skip.

**Rust counterpart.** `crates/x3d-viewer/src/viewer.rs`:

| X_ITE | X_ITE_R |
| --- | --- |
| `X3DTimeContext.advanceTime`'s frame-rate limit | `Viewer3d::frame_due (now)` — `viewer.rs:1110`, a *separate predicate* the caller drives |
| `X3DBrowserContext [_traverse]` | `Viewer3d::step (now)` — `viewer.rs:1124`, which always sets `self .time = now` |
| the gate's caller | `Embed::step (now)` — `embed.rs:515`: `if !c .frame_due (now) { return; }` then `c .step (now)` |
| `reshape` | `Embed::resize (width, height)`, called from `Embed::frame` — `embed.rs:548` — *before* `self .step (now)` |

**Finding: X_ITE_R has no `now < 0` reshape sentinel at all.** A resize is not a kind of
frame in the Rust port; `Embed::frame` resizes and *then* takes a normal step at the real
`now`. There is no code path that steps with a negative time, so `checkFrameRate` has no
direct analogue to port.

**The divergence is therefore narrow but real.** Because a resize frame still goes through
`frame_due`, a resize that arrives sooner than `1 / MaximumFrameRate` after the previous
frame is skipped entirely in X_ITE_R — time does not advance. Under 16.6.0 that same
reshape advances time. So:

* X_ITE_R matches **16.5.0** for a resize inside the frame-rate window;
* X_ITE_R diverges from **16.6.0** in exactly that window;
* outside the window (the common case) both advance time and X_ITE_R already agrees.

**Migration shape**, when the lane is authorized: let `Embed::frame` bypass `frame_due` for
the step that follows a `resize`, which is the Rust-shaped equivalent of
`advanceTime (now >= 0)`. One oracle case is still required — `reshape-advance-time` in
§5.2 — because the observable is `elapsedTime` across a resize and only a capture settles
it. **The REA ledger gap noted in §4 stands: no entry covers `advanceTime`/frame pacing.**

### 8.2 X2 — multiline `data:` URL header: Rust counterpart located

**Upstream change.** The `s` (dotAll) flag was dropped from
`/^\s*data:(.*?)(?:;charset=(.*?))?(?:;(base64))?,/`, so in 16.6.0 a `data:` URL whose
header contains a newline before the comma **no longer matches** and is not treated as an
inline document.

**Rust counterpart.** `crates/x3d-runtime/src/loader.rs:138`, `data_url_text (candidate)`.
Its doc comment already quotes the 16.5.0 regex *including the `/s`*, so the intent is
documented and the pin is explicit.

It is not a regex port — it is:

```rust
let rest = candidate.trim_start().strip_prefix("data:")?;
let (header, data) = rest.split_once(',')?;
```

**Finding: `split_once (',')` is newline-blind, so X_ITE_R matches 16.5.0 and diverges from
16.6.0.** A header containing a newline is accepted today, exactly as the dotAll regex did.
`trim_start()` and `^\s*` agree (both consume newlines), so the leading-whitespace half
needs no change.

**Migration shape:** reject the candidate when the header spans a newline — one condition
on `header`, in that one function. It is the only `data:` parse site in the tree. An oracle
case is needed to fix which characters count as the line break (the JS `.` excludes
`\n`, `\r`, `\u2028`, `\u2029`, which is *not* the same set as Rust's `lines()`).

### 8.3 P2 — Draco decoder: investigation complete, classified **no-op for X_ITE_R**

The 30-changed-line figure came from a textual diff, which is misleading: the file is a
single minified emscripten bundle, so line counts track the build's formatting, not its
behavior. Measured properties instead:

| | 16.5.0 pin | 16.6.0 |
| --- | --- | --- |
| `assets/lib/draco_decoder_gltf.js` | 519,471 bytes, 30 lines | 479,094 bytes, 18 lines |
| `.wasm` files the bundle references | `draco_decoder_gltf.wasm` | **none** |
| supported bitstream range (`isVersionSupported`) | **1.0 – 1.5** | **1.0 – 1.5**, unchanged |
| `draco3d` in `package.json` | absent | `^1.5.9` (devDependency) |
| `draco_decoder_gltf.wasm` shipped in `src/` | no | no |

**What actually changed:** the vendored decoder was rebuilt from a WASM-loading emscripten
build into a self-contained **JS-only** build, and `draco3d ^1.5.9` was added as a
devDependency so the vendored file is now generated from the npm package instead of
hand-copied (the 16.5.0 file still carries the hand-edit banner
`// * Changed ENVIRONMENT_IS_NODE to always be false.`, which 16.6.0 has dropped). **The
accepted bitstream version range is identical**, which is the property that would change
decoded output.

**Why it is a no-op here:** X_ITE_R does not implement Draco at all.

* `crates/x3d-parse/src/gltf2/mod.rs:10` — `KHR_draco_mesh_compression` is listed as missing;
* `crates/x3d-parse/src/gltf2/geometry.rs:201` — the primitive extension is skipped;
* `crates/x3d-parse/tests/gltf_rea.rs:52` — the oracle records it as missing.

**Conclusion: nothing to port, no behavior difference to capture, and no new oracle case
required.** P2 is closed as not-applicable rather than unknown. If Draco is ever
implemented, target the 16.6.0 JS-only build and the unchanged 1.0–1.5 range; the decoder
rebuild does not itself create a parity obligation.