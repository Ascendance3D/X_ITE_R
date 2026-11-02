//! The touches of a native window as the DOM's `touchstart`, `touchmove` and `touchend` events of the canvas
//! (`event .touches`: the touches still down, the first two count), for [`crate::viewer::Viewer3d::input`].

use crate::nav::Touches;
use crate::viewer::UiEvent;

/// The phase of a window system touch (winit's `TouchPhase`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

/// The touches that are down.
#[derive(Clone, Debug, Default)]
pub struct TouchTracker {
    down: Vec<(u64, (f64, f64))>,
}

impl TouchTracker {
    fn touches(&self, width: f64) -> Touches {
        let mut t = Touches {
            n: self.down.len(),
            p: [(0.0, 0.0); 2],
            zoom_width: width,
        };

        for (i, (_, p)) in self.down.iter().take(2).enumerate() {
            t.p[i] = *p;
        }

        t
    }

    /// One touch event at `pos` (viewport pixels, origin bottom left) of a window `width` pixels wide.
    pub fn event(&mut self, phase: Phase, id: u64, pos: (f64, f64), width: f64) -> Option<UiEvent> {
        match phase {
            Phase::Started => {
                self.down.retain(|(i, _)| *i != id);
                self.down.push((id, pos));
                Some(UiEvent::TouchStart(self.touches(width)))
            }
            Phase::Moved => {
                self.down.iter_mut().find(|(i, _)| *i == id)?.1 = pos;
                Some(UiEvent::TouchMove(self.touches(width)))
            }
            Phase::Ended | Phase::Cancelled => {
                self.down.iter().position(|(i, _)| *i == id).map(|k| {
                    self.down.remove(k);
                    UiEvent::TouchEnd(self.touches(width))
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touches_still_down_are_reported() {
        let mut t = TouchTracker::default();

        assert!(matches!(
            t.event(Phase::Started, 1, (10.0, 20.0), 300.0),
            Some(UiEvent::TouchStart(x)) if x.n == 1 && x.p[0] == (10.0, 20.0)
        ));
        assert!(matches!(
            t.event(Phase::Started, 2, (30.0, 40.0), 300.0),
            Some(UiEvent::TouchStart(x)) if x.n == 2 && x.p[1] == (30.0, 40.0)
        ));
        assert!(matches!(
            t.event(Phase::Ended, 1, (10.0, 20.0), 300.0),
            Some(UiEvent::TouchEnd(x)) if x.n == 1 && x.p[0] == (30.0, 40.0)
        ));
        assert!(t.event(Phase::Moved, 9, (0.0, 0.0), 300.0).is_none());
        assert!(t.event(Phase::Ended, 9, (0.0, 0.0), 300.0).is_none());
    }
}
