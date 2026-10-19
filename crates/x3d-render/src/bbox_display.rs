//! bboxDisplay: Browser/Grouping/X3DGroupingContext.js getBBoxShape () and
//! Browser/Grouping/X3DBBoxNode.js (REA batch 08, lane net).
//!
//! X3DGroupingNode adds `childNode .getBBoxNode ()` to its visible objects when a child that is an
//! X3DBoundedObject has `bboxDisplay` TRUE. The bbox node draws the shared bbox shape (a Shape without an
//! Appearance whose IndexedLineSet is the twelve edges of the unit cube) with the matrix
//! `setTransform (bbox .center, null, bbox .size)` of the bounded object's getBBox. This module has the unit
//! cube and the node types the renderer draws it for.

use crate::geometry::{self, Geometry};

/// The X3DBoundedObjects whose bboxDisplay the renderer draws. ReceiverPdu, SignalPdu and TransmitterPdu
/// (getBBox is bboxSize / bboxCenter, empty at the default). They are not "bounded objects" of their parent
/// (X3DChildNode.setBoundedObject is never called), so the parent's bbox ignores them; only this box is drawn.
pub const DISPLAYED_TYPES: &[&str] = &["ReceiverPdu", "SignalPdu", "TransmitterPdu"];

/// The coordIndex and points of getBBoxShape ().
const COORD_INDEX: [i32; 24] = [0, 1, 2, 3, 0, -1, 4, 5, 6, 7, 4, -1, 0, 4, -1, 1, 5, -1, 2, 6, -1, 3, 7, -1];
const POINTS: [f32; 24] = [0.5, 0.5, 0.5, -0.5, 0.5, 0.5, -0.5, -0.5, 0.5, 0.5, -0.5, 0.5, 0.5, 0.5, -0.5, -0.5, 0.5, -0.5, -0.5, -0.5, -0.5, 0.5, -0.5, -0.5];

/// IndexedLineSet.build of the bbox shape.
pub fn unit_box_lines() -> Geometry {
    geometry::indexed_line_set(Some(&POINTS), &COORD_INDEX, None, &[], true, &Default::default())
}
