# Promotion review of 13 node entries (owner-approved review, combined candidate 0023b6b)

Rule used: COMPLETE only when field and event evidence AND picture/runtime evidence cover the node's own fields and its consumers.
Field/event = `rea_batch09_sai.rs::rea_batch09_sai_cases_match_x_ite` (exact vs 3 X_ITE recordings). Pictures = `render_rea_b09_visual.rs`
(MSAA 0/4, bound unchanged). Runtime = `rea_batch09_inputs.rs`. The SAI test sets only non-node fields.

| REA ID | decision | reason |
| --- | --- | --- |
| node:Color | PASS | IFS per-vertex+colorIndex, per-face; PointSet; IndexedLineSet per-vertex |
| node:Coordinate | PASS | IFS, IndexedLineSet, PointSet |
| node:TextureTransform | PASS | translation, rotation+center, scale, combined |
| node:TextureCoordinate3D | PASS | 3-component into PixelTexture3D |
| node:TextureCoordinate4D | PASS | w != 1 into PixelTexture3D |
| node:FontStyle | PASS | family/fallback, style, size, spacing, justify, direction fields |
| node:TextureBackground | PASS | faces, transparency, sky/ground + background-runtime incl. texture node replaced |
| node:LocalFog | HOLD | render-inputs lane recommended COMPLETE only "geometry only; not volume draws" and the owner kept it PARTIAL: LocalFog on VolumeData draws is not compared |
| node:ColorRGBA | PASS (2nd review: vis-colorrgba) | alpha per-face (colorPerVertex FALSE) and alpha on lines are not pictured |
| node:IndexedLineSet | PASS (2nd review: vis-linecolor, vis-linecolor-invalid) | colorPerVertex FALSE, explicit colorIndex, and ColorRGBA on lines are not pictured |
| node:Material | PASS (2nd review: vis-material-ambient, vis-material-ambient-tex) | ambientIntensity pictured only as 0 with no ambient light; a non-default value under an ambient light is not compared |
| node:LayoutGroup | HOLD | layout, viewport, addChildren, removeChildren never set at run time (SAI skips node fields; no runtime layout case) |
| node:OrthoViewpoint | HOLD | viewAll and lookAt overrides are not compared (4.1 SFVec4f set at run time not either) |

Evidence that would flip a HOLD: a picture of each missing case (ColorRGBA, IndexedLineSet, Material), a stepped runtime case
(LayoutGroup, OrthoViewpoint), a volume-draw fog case (LocalFog). The other 20 node entries and the 25 DIFFERENT entries are unchanged.
