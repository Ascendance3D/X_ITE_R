//! EventUtilities component: BooleanFilter, BooleanToggle, BooleanTrigger, IntegerTrigger,
//! TimeTrigger, IntegerSequencer, BooleanSequencer (X3DSequencerNode.js).

use super::State;
use crate::*;

impl Runtime {
    /// `executionContext .getSpecificationVersion () <= 3.3` (string compared to a number).
    fn spec_le_33(&self) -> bool {
        self.scene.specification_version.trim().parse::<f64>().map(|v| v <= 3.3).unwrap_or(false)
    }

    fn seq_index(&mut self, node: NodeId) -> &mut i64 {
        match &mut self.nrt_mut(node).state {
            State::Sequencer { index } => index,
            _ => panic!("not a sequencer"),
        }
    }

    fn seq_size(&self, node: NodeId) -> i64 {
        self.pre_value(node, "keyValue").map(|v| v.len() as i64).unwrap_or(0)
    }

    fn seq_sequence(&mut self, node: NodeId, index: i64) {
        let value = match self.pre_value(node, "keyValue") {
            Some(FieldValue::MFInt32(v)) => FieldValue::SFInt32(usize::try_from(index).ok().and_then(|i| v.get(i)).copied().unwrap_or(0)),
            Some(FieldValue::MFBool(v)) => FieldValue::SFBool(usize::try_from(index).ok().and_then(|i| v.get(i)).copied().unwrap_or(false)),
            _ => return,
        };

        if let Some(k) = self.pre(node, "value_changed") {
            self.set_value(k, value);
        }
    }

    pub(crate) fn event_utils_call(&mut self, node: NodeId, cb: Cb) {
        let set = |rt: &mut Runtime, name: &str, v: FieldValue| {
            if let Some(k) = rt.pre(node, name) {
                rt.set_value(k, v);
            }
        };

        match cb {
            Cb::BooleanFilterSetBoolean => {
                let value = self.get_bool(node, "set_boolean");

                if value {
                    set(self, "inputTrue", FieldValue::SFBool(true));
                } else {
                    set(self, "inputFalse", FieldValue::SFBool(false));
                }

                set(self, "inputNegate", FieldValue::SFBool(!value));
            }
            Cb::BooleanToggleSetBoolean => {
                if self.get_bool(node, "set_boolean") {
                    let t = self.get_bool(node, "toggle");
                    set(self, "toggle", FieldValue::SFBool(!t));
                }
            }
            Cb::BooleanTriggerSetTriggerTime => {
                set(self, "triggerTrue", FieldValue::SFBool(true));
                set(self, "triggerFalse", FieldValue::SFBool(false));
            }
            Cb::IntegerTriggerSetBoolean => {
                if self.get_bool(node, "set_boolean") || self.spec_le_33() {
                    let v = self.get_f64(node, "integerKey") as i32;
                    set(self, "triggerValue", FieldValue::SFInt32(v));
                }
            }
            Cb::TimeTriggerSetBoolean => {
                if self.get_bool(node, "set_boolean") || self.spec_le_33() {
                    let now = self.now;
                    set(self, "triggerTime", FieldValue::SFTime(now));
                }
            }
            Cb::SequencerSetFraction => {
                let fraction = self.get_f64(node, "set_fraction");
                let key: Vec<f64> = match self.pre_value(node, "key") {
                    Some(FieldValue::MFFloat(v)) => v.iter().map(|x| *x as f64).collect(),
                    _ => vec![],
                };
                let length = key.len();

                if length == 0 {
                    return;
                }

                let i: i64 = if length == 1 || fraction <= key[0] {
                    0
                } else if fraction >= key[length - 1] {
                    self.seq_size(node) - 1
                } else {
                    super::interp::upper_bound(&key, fraction) as i64 - 1
                };

                if i != *self.seq_index(node) && i < self.seq_size(node) {
                    *self.seq_index(node) = i;
                    self.seq_sequence(node, i);
                }
            }
            Cb::SequencerSetPrevious => {
                if self.get_bool(node, "previous") {
                    let size = self.seq_size(node);
                    let index = self.seq_index(node);

                    if *index <= 0 {
                        *index = size - 1;
                    } else {
                        *index -= 1;
                    }

                    let i = *index;

                    if i < size {
                        self.seq_sequence(node, i);
                    }
                }
            }
            Cb::SequencerSetNext => {
                if self.get_bool(node, "next") {
                    let size = self.seq_size(node);
                    let index = self.seq_index(node);

                    if *index >= size - 1 {
                        *index = 0;
                    } else {
                        *index += 1;
                    }

                    let i = *index;

                    if i < size {
                        self.seq_sequence(node, i);
                    }
                }
            }
            Cb::SequencerSetKey | Cb::SequencerSetKeyValue => *self.seq_index(node) = -1,
            _ => {}
        }
    }
}
