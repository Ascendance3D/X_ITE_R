# Navigation Coverage

This document outlines the item-level coverage for the navigation subsystems against the pinned X_ITE 16.5.0 source (`oracle/x_ite-pin/src/x_ite/Browser/Navigation/` and `oracle/x_ite-pin/src/x_ite/Rendering/X3DRenderObject.js`).

## sub:nav.examine (`ExamineViewer.js`)
* **Drag to Rotate (`mousemove` with button 0)**
  * **MATCHES**: `x10_rotate_zoom`, `viewer.rs:153` (examine_drag_rotates_and_wheel_zooms)
* **Pan (`mousemove` with button 1 / middle click)**
  * **MATCHES**: `x10_rotate_zoom`
* **Zoom (`wheel`)**
  * **MATCHES**: `x10_rotate_zoom`, `viewer.rs:153`
* **LookAt Shape (`dblclick`)**
  * **MATCHES**: `rea_batch09_inputs`
* **LookAt Point (`dblclick` + alt)**
  * **MATCHES**: `rea_batch09_inputs`

## sub:nav.walk-fly (`WalkViewer.js`, `FlyViewer.js`, `X3DFlyViewer.js`)
* **Drag to move/rotate (`mousemove` with button 0)**
  * **MATCHES**: `x10_rotate_zoom`, `x10_plane_lookat`, `rea_batch09_nav`
* **Scroll to move/zoom (`wheel`)**
  * **MATCHES**: `x10_rotate_zoom`, `nav.rs:1210-1215` (wheel event processing in Rust)
* **`isCollidable()`**
  * **MATCHES**: WalkViewer returns `true` (JS `WalkViewer.js:14`, Rust checked by `nav.rs` trait behavior), Fly returns `false` (JS `FlyViewer.js:12`).

## sub:nav.plane (`PlaneViewer.js`)
* **Pan (`mousemove` with button 1 / Alt+left)**
  * **MATCHES**: `x10_plane_lookat`, Rust: `nav/plane.rs:107-117`
* **Zoom (`wheel`)**
  * **MATCHES**: `x10_plane_lookat`, Rust: `nav/plane.rs:119-145`
* **LookAt (`dblclick`)**
  * **MATCHES**: `x10_plane_lookat`, Rust: `nav/plane.rs:90-101`

## sub:nav.lookat (`LookAtViewer.js`)
* **Look around (`mousemove` with button 0)**
  * **MATCHES**: `x10_plane_lookat`, Rust: `nav/lookat.rs:81-87`
* **LookAt Shape (`dblclick`)**
  * **MATCHES**: `x10_plane_lookat`, Rust: `nav/lookat.rs:71-80`
* **LookAt Point (`dblclick` + Alt)**
  * **MATCHES**: `x10_plane_lookat`, Rust: `nav/lookat.rs:71-80`
* **Zoom (`wheel`)**
  * **MATCHES**: `x10_plane_lookat`, Rust: `nav/lookat.rs:89-108`

## sub:nav.collision-gravity (`X3DViewer.js`, `X3DRenderObject.js`, `collision.rs`)
* **Collision shape building**
  * **MATCHES**: `rea_batch09_inputs` (collision-walk), `rea_batch09_nav`. Rust: `collision.rs:105-120`
* **Non-particle collision stop**
  * **MATCHES**: `rea_nav_collision_items`. JS: `X3DRenderObject.js:379-388`. Rust: `collision.rs:980-990`
* **Gravity fall onto a floor**
  * **MATCHES**: `rea_nav_collision_items`. JS: `X3DRenderObject.js:1011-1033`. Rust: `collision.rs:1096-1114`
* **Step up below step height**
  * **MATCHES**: `rea_nav_collision_items`. JS: `X3DRenderObject.js:1040-1047`. Rust: `collision.rs:1119-1123`
* **Step up above step height (fail)**
  * **MATCHES**: `rea_nav_collision_items`. JS: `X3DRenderObject.js:1040-1047`. Rust: `collision.rs:1119-1123`
* **JS BrowserOptions Gravity Math**
  * **MATCHES**: Constant `9.80665` (JS `BrowserOptions.js:430`, Rust `collision.rs:1103`)
