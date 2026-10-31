//! The operations the engine makes on its Web Audio nodes, as plain data.
//!
//! X_ITE's Sound nodes call the browser's Web Audio API. Here the same calls go to the engine's own graph (`web-audio-api` with
//! the Chromium-style kernels), which renders on a native machine. A page of X_ITE_R must reach the page's AudioContext, so the
//! engine can also record every call it makes as an [`Op`] (`AudioEngine::record_ops`). The page drains the log (`take_ops`) and
//! makes the same call on a real browser node of its AudioContext (`web/audio_mirror.mjs`). The log holds no browser types, so
//! the native tests can replay it into a model and compare the model with `AudioEngine::graph`.
//!
//! Nodes are named by their arena slot ([`super::AId`]). A slot is reused after a `release`; the log keeps the order, so the
//! release comes before the node that takes the slot.

use std::sync::Arc;

/// An argument of an [`Op`].
#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    Int(usize),
    Num(f64),
    Bool(bool),
    Str(&'static str),
    /// A list of numbers (a wave table, a curve).
    List(Vec<f32>),
    /// Planar sample data and its sample rate (an AudioBuffer).
    Buffer(Arc<Vec<Vec<f32>>>, f32),
}

/// One call on a Web Audio node: its name (`"connect"`, `"param"`, ...) and arguments.
#[derive(Clone, Debug, PartialEq)]
pub struct Op {
    pub name: &'static str,
    pub args: Vec<Arg>,
}

fn number(v: f64) -> String {
    if v.is_finite() {
        format!("{v:?}")
    } else if v.is_nan() {
        "null".into()
    } else if v > 0.0 {
        "1e999".into()
    } else {
        "-1e999".into()
    }
}

/// Planar samples and the sample rate of an AudioBuffer argument.
pub type BufferData = (Arc<Vec<Vec<f32>>>, f32);

/// The ops as JSON for the page: `[["connect", 1, 0, 0, 0], ...]`. A buffer argument is `{"buffer": n}`, `n` indexing
/// `buffers`, to which each buffer of the ops is appended; the page reads them one by one.
pub fn to_json(ops: &[Op], buffers: &mut Vec<BufferData>) -> String {
    let mut out = String::from("[");

    for (i, op) in ops.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }

        out.push_str(&format!("[\"{}\"", op.name));

        for arg in &op.args {
            out.push(',');

            match arg {
                Arg::Int(v) => out.push_str(&v.to_string()),
                Arg::Num(v) => out.push_str(&number(*v)),
                Arg::Bool(v) => out.push_str(if *v { "true" } else { "false" }),
                Arg::Str(v) => out.push_str(&format!("\"{v}\"")),
                Arg::List(v) => {
                    out.push('[');
                    out.push_str(&v.iter().map(|&x| number(f64::from(x))).collect::<Vec<_>>().join(","));
                    out.push(']');
                }
                Arg::Buffer(data, rate) => {
                    out.push_str(&format!("{{\"buffer\":{}}}", buffers.len()));
                    buffers.push((Arc::clone(data), *rate));
                }
            }
        }

        out.push(']');
    }

    out.push(']');
    out
}
