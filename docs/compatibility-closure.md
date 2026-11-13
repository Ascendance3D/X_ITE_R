# Compatibility closure report: the 36 PARTIAL ledger entries

Date: 2026-11-13. Source: `ledger/ledger.json` (see the sha256 in `ledger/SUMMARY.md`), the `missing` and `notes` fields of
each PARTIAL entry, and the merged evidence they name. This report did not do a new upstream audit and did not repeat earlier
investigations. It changes no status. `ledger/SUMMARY.md` (generated) has the full `missing` text of each entry. This file
sorts that text into requirement kinds.

## Requirement kinds

| code | kind | meaning |
| --- | --- | --- |
| F | missing functionality | the port does not do something that X_ITE or the standard does; work in this repository can close it |
| P | platform verification | the code exists; a run on hardware or a platform that this host does not have is missing |
| D | recorded difference | an owner-approved or ADR-recorded difference, an upstream X_ITE defect that the port does not copy, or a behavior that cannot have a deterministic reference. No work unless the owner reopens it |
| T | missing test coverage | the behavior is probably correct, but no comparison with pinned X_ITE (or a reference) exists |
| U | upstream or dependency blocker | needs a change in a dependency (wgpu, QuickJS, a platform decoder, the browser) |
| O | owner review | the entry has no open item of its own; only an owner decision keeps it PARTIAL |

## Totals

110 requirement items in 36 entries (an item that two entries share is counted in each entry):

| kind | items | unique items |
| --- | ---: | ---: |
| F missing functionality | 14 | 11 |
| P platform verification | 15 | 13 |
| D recorded difference | 64 | — |
| T missing test coverage | 11 | 11 |
| U upstream or dependency blocker | 5 | 5 |
| O owner review | 1 | 1 |

12 entries have only D or O items: node:ParticleSystem, node:WindPhysicsModel, node:Script, node:BlendedVolumeStyle,
node:ShadedVolumeStyle, node:Inline, sub:parse.glb2 (no item of its own; it shares the glTF decoders), sub:exec.context-api (O),
sub:proto.externproto, sub:io.file-loader, sub:nav.collision-gravity, sub:pointer.picking. No code work remains in them.
A promotion needs the normal ledger review and, where an owner decision keeps the entry PARTIAL, the owner.

## Per entry

| entry | F | P | D | T | U | open items (F, P, T, U, O) |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| node:GaussianSplats | 0 | 1 | 4 | 1 | 0 | P: low-power machines and displays above 60 Hz not measured. T: the sort transient at SH degree above 0 |
| node:Inline | 0 | 0 | 6 | 0 | 0 | — (the ledger calls c07 overlap-frames an "open difference", but ADR 0014 section 4 keeps its step 5 by owner decision of 2026-11-04: a D item) |
| node:BoundedPhysicsModel | 1 | 1 | 2 | 1 | 0 | F: a bare Runtime without the viewer's geometry provider has empty geometry. P: the ARRAY_SIZE clamp is measured on one GPU; no pinned Metal measurement (`docs/particle-array-size-backends.md`; #89 is closed, the item is not). T: the start timer only for an event at frame 1 |
| node:ParticleSystem | 0 | 0 | 2 | 0 | 0 | — |
| node:VolumeEmitter | 1 | 1 | 2 | 0 | 0 | F and P: as node:BoundedPhysicsModel (shared) |
| node:WindPhysicsModel | 0 | 0 | 1 | 0 | 0 | — |
| node:Script | 0 | 0 | 3 | 0 | 0 | — |
| node:ShaderPart | 1 | 0 | 2 | 0 | 0 | F: GLSL ES type checks cover the recorded patterns only (float loop condition, int expression into float, matrix and struct constructors, constructor argument of unknown size) — **owned by the active Quick Wins lane** |
| node:AudioClip | 2 | 1 | 3 | 0 | 0 | F: the StreamAudioDestination MediaStream is not mirrored in the browser; Opus is rejected natively. P: real-device output (#88) |
| node:AudioDestination | 1 | 1 | 1 | 0 | 0 | F: one native context serves every AudioDestination, so the last mediaDeviceID decides the device. P: a host with more than one output |
| node:BufferAudioSource | 2 | 0 | 1 | 0 | 0 | F: Opus (shared with AudioClip); a resampled file is within 5e-5 of Chromium, not exact |
| node:Sound | 1 | 1 | 2 | 0 | 0 | F: MediaStream (shared). P: real-device output (shared) |
| node:FontLibrary | 0 | 0 | 2 | 2 | 0 | T: a CID-keyed seac world; name records other than en, an fvar or avar that opentype.js rejects, gvar fonts |
| node:MovieTexture | 1 | 1 | 4 | 2 | 2 | F: MPEG-1 in the browser and the MPEG-1 audio/video start-offset alignment (ISO/IEC 11172 is not in the local library). P: Windows has no MP4 backend (no approved test environment). T: the browser MP4/WebM/Ogg sound path against a pinned recording; container RGB, FCC, YCoCg and YDZDX matrices. U: AV1 in MP4 (GStreamer 1.24.2 and AVFoundation on the M1 cannot decode it); Chromium decodes no Theora picture |
| node:BlendedVolumeStyle | 0 | 0 | 2 | 0 | 0 | — |
| node:ShadedVolumeStyle | 0 | 0 | 1 | 0 | 0 | — |
| sub:parse.gltf2 | 0 | 0 | 5 | 1 | 1 | T: no glTF MP4 case. U: `pointer/roughness-step.gltf` fails in both (upstream defect) |
| sub:parse.glb2 | 0 | 0 | 0 | 0 | 0 | none of its own (shares sub:parse.gltf2) |
| sub:parse.svg | 1 | 0 | 2 | 0 | 0 | F: achromatic and exact-zero noise after conversions and var() origins is not reproduced |
| sub:parse.golden-gate | 0 | 0 | 1 | 1 | 0 | T: addParsers / removeParsers / getParsers are checked against the pinned source, not a recorded run |
| sub:fields.types | 0 | 0 | 2 | 0 | 1 | U: engine-level Intl and Error.stack (QuickJS) |
| sub:exec.context-api | 0 | 0 | 0 | 0 | 0 | O: the class chain is ported (#77, `5ec9ae2b`) and compared; the owner decision of 2026-10-20 keeps the entry PARTIAL and lists no remaining requirement |
| sub:proto.proto | 1 | 0 | 3 | 0 | 0 | F: createInstance with the non-public setup argument is not ported |
| sub:proto.externproto | 0 | 0 | 1 | 0 | 0 | — |
| sub:io.file-loader | 0 | 0 | 2 | 0 | 0 | — |
| sub:browser.multi-viewer-thread | 0 | 1 | 0 | 1 | 0 | P: a second GPU device (max_line_width and metal_native_lines are process-global per backend). T: audible sound of two viewers |
| sub:browser.keyboard-ui | 0 | 2 | 0 | 0 | 0 | P: a real macOS Ctrl+click and a real touch screen (#287); Wayland and macOS keyboard (#88) |
| sub:browser.webxr | 1 | 1 | 1 | 0 | 1 | F: native OpenXR (#83; needs an approved implementation plan). P: a physical headset. U: XRGPUBinding is not in the stable browser |
| sub:nav.examine | 0 | 1 | 0 | 0 | 0 | P: physical touch screens (#143) |
| sub:nav.walk-fly | 0 | 1 | 1 | 0 | 0 | P: the pose bound for GPUs other than the RTX 3060 Ti |
| sub:nav.collision-gravity | 0 | 0 | 2 | 0 | 0 | — |
| sub:render.core | 0 | 0 | 1 | 1 | 0 | T: two private worlds exceed the bound and were not re-read, so their cause is not proven |
| sub:render.transparency-sorting | 0 | 1 | 1 | 0 | 0 | P: OIT at MSAA 4 on other GPUs and sample counts |
| sub:text.fonts | 0 | 0 | 0 | 1 | 0 | T: only the 12 answer-key fonts and one authored CFF font are compared |
| sub:sound.engine | 1 | 1 | 1 | 0 | 0 | F: the macOS Accelerate arithmetic (15 rea_b05 worlds within 5.4e-7 of the Mac recording, not exact). P: other Linux audio stacks and Wayland-only hosts |
| sub:pointer.picking | 0 | 0 | 3 | 0 | 0 | — |

The D items of each entry are in its `missing` field (`ledger/SUMMARY.md`) and in the ADRs it names.

## Highest-priority executable work

Work that an agent can do on this host, with no new hardware and no new owner decision:

1. **node:MovieTexture: MPEG-1 audio/video start-offset alignment (native).** The decoders exist; the streams both start at 0
   (`docs/mpeg1-implementation.md`, "Not done"). The pinned page cannot play MPEG-1, so the reference is the stream time stamps
   (ffmpeg's start times), not an X_ITE recording.
2. **node:AudioClip / node:Sound: the StreamAudioDestination MediaStream in the browser.** Low observable value: X_ITE makes a
   `MediaStreamAudioDestinationNode` that it keeps private and never plays, and the engine's graph (`audioGraph ()`) already has
   the node. The page mirror has no node for it (`stream_destination` emits no op when `to_output` is false). Only the page's own
   graph differs; nothing is audible.
3. **node:BufferAudioSource: exact SincResampler output** (now within 5e-5 of Chromium).
4. **sub:parse.svg: achromatic and exact-zero noise** after conversions and var() origins.
5. **Test coverage** (T items that need only this host and the reference Chrome 156): the browser MP4/WebM/Ogg sound path against a pinned recording; a glTF MP4 case (Linux GStreamer); the BoundedPhysicsModel start timer after frame 1; a CID-keyed seac world for FontLibrary.

Not in this list: the GLSL ES type checks (the active Quick Wins lane owns them), native Opus (needs a decoder admission
record, `docs/decoder-admission.md`), native OpenXR (needs an approved plan), the per-node audio device and the bare-Runtime
geometry provider (design changes; they need a plan first), and the macOS Accelerate arithmetic (needs the M1 host).

## Blocked items and their owners

- Hardware or a person at hardware (P): #88, #143, #287, a second GPU vendor, a headset, a Windows test environment. Owner.
- Upstream (U): wgpu compatibility mode (#142, not a ledger item), XRGPUBinding, AV1 in platform MP4 decoders, Theora in Chromium, QuickJS Intl and Error.stack, the pinned roughness-step.gltf defect.
- Owner review (O): sub:exec.context-api.
