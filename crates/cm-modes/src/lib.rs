//! Port of the CodeMirror 5 (5.65.21) mode API — `src/modes.js` — plus the
//! tokenizing loops that drive modes: `runMode` from `addon/runmode/runmode.node.js`
//! and `readToken` from `src/line/highlight.js`.
//!
//! Copyright (C) 2017 by Marijn Haverbeke <marijn@haverbeke.berlin> and others
//! (original JavaScript, MIT). Rust port, MIT.
//!
//! JS modes are objects with optional methods and an untyped state. Here a mode is
//! a [`Mode`] trait object and its state is a [`State`]: either `True` (the JS
//! `startState` fallback returns `true` when a mode has no `startState`) or a boxed
//! value of the mode's own state type.
//!
//! Modes ported so far: [`toml`] (`mode/toml/toml.js`).

use std::any::Any;
use std::collections::HashMap;
use std::fmt::Debug;
use std::rc::Rc;

use cm_core::text::{from_u16, Text};
use cm_core::util::string_stream::{LineOracle, StringStream};

pub mod toml;

// --------------------------------------------------------------------------------
// State

/// A mode state value: anything `Clone + Debug + 'static`.
pub trait AnyState: Any + Debug {
    fn clone_box(&self) -> Box<dyn AnyState>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Any + Clone + Debug> AnyState for T {
    fn clone_box(&self) -> Box<dyn AnyState> {
        Box::new(self.clone())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// A mode's state.
#[derive(Debug)]
pub enum State {
    /// `startState(mode)` returns `true` for modes without `startState`.
    True,
    Value(Box<dyn AnyState>),
}

impl State {
    pub fn new<T: AnyState>(v: T) -> State {
        State::Value(Box::new(v))
    }
    /// The state as the mode's own type. Panics on a type mismatch (in JS a mode
    /// handed a foreign state would misbehave instead).
    pub fn get<T: 'static>(&self) -> &T {
        match self {
            State::Value(b) => (**b).as_any().downcast_ref().expect("mode state of another type"),
            State::True => panic!("mode state is `true`"),
        }
    }
    pub fn get_mut<T: 'static>(&mut self) -> &mut T {
        match self {
            State::Value(b) => (**b).as_any_mut().downcast_mut().expect("mode state of another type"),
            State::True => panic!("mode state is `true`"),
        }
    }
}

impl Clone for State {
    fn clone(&self) -> State {
        match self {
            State::True => State::True,
            State::Value(b) => State::Value((**b).clone_box()),
        }
    }
}

// --------------------------------------------------------------------------------
// Mode

/// Editor options a mode factory receives (`CodeMirror.defaults` / `cm.options`).
/// Only the fields modes commonly read are modeled.
#[derive(Clone, Debug)]
pub struct ModeOptions {
    pub indent_unit: i64,
    pub tab_size: i64,
}

impl Default for ModeOptions {
    /// `runmode.node.js` sets `CodeMirror.defaults = { indentUnit: 2 }`.
    fn default() -> Self {
        ModeOptions { indent_unit: 2, tab_size: 8 }
    }
}

/// A configuration value inside a mode spec (`{name: "...", someOption: value}`).
#[derive(Clone, Debug, PartialEq)]
pub enum ConfigValue {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
}

/// A resolved mode spec: `{name, ...options}`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModeSpec {
    pub name: String,
    pub helper_type: Option<String>,
    pub options: HashMap<String, ConfigValue>,
}

impl ModeSpec {
    pub fn named(name: &str) -> ModeSpec {
        ModeSpec { name: name.to_string(), ..Default::default() }
    }
}

/// What can be passed where the JS accepts a mode spec: a MIME type or name, or a
/// spec object. (`null`/`undefined` is `Spec::Null`.)
#[derive(Clone, Debug, PartialEq)]
pub enum Spec {
    Null,
    Str(String),
    Obj(ModeSpec),
}

impl From<&str> for Spec {
    fn from(s: &str) -> Spec {
        Spec::Str(s.to_string())
    }
}

/// The JS mode object. Every method but `token` is optional in JS; the defaults
/// here stand for "property absent".
pub trait Mode {
    /// `mode.name`, set by `getMode`.
    fn name(&self) -> &str;
    /// `mode.startState(indentUnit / baseIndent...)`; `None` = no `startState`.
    fn start_state(&self, _a1: i64, _a2: i64) -> Option<State> {
        None
    }
    /// `mode.token(stream, state)`: the style, `None` for null/undefined.
    fn token(&self, stream: &mut StringStream<'_>, state: &mut State) -> Option<String>;
    /// `mode.blankLine`: `None` when the mode has none.
    fn blank_line(&self, _state: &mut State) -> Option<()> {
        None
    }
    fn has_blank_line(&self) -> bool {
        false
    }
    /// `mode.copyState`; `None` = absent, so [`copy_state`] copies generically.
    fn copy_state(&self, _state: &State) -> Option<State> {
        None
    }
    /// `mode.innerMode(state)`; `None` = absent or returned nothing.
    fn inner_mode(&self, _state: &State) -> Option<(Rc<dyn Mode>, State)> {
        None
    }
    /// `mode.indent(state, textAfter, line)`; `None` = absent or `CodeMirror.Pass`.
    fn indent(&self, _state: &State, _text_after: &[u16], _line: &[u16]) -> Option<i64> {
        None
    }
    fn electric_chars(&self) -> Option<&str> {
        None
    }
    fn line_comment(&self) -> Option<&str> {
        None
    }
    fn block_comment_start(&self) -> Option<&str> {
        None
    }
    fn block_comment_end(&self) -> Option<&str> {
        None
    }
    fn helper_type(&self) -> Option<&str> {
        None
    }
}

/// A `defineMode` factory: `function(config, modeConfig) -> mode`.
pub type ModeFactory = Rc<dyn Fn(&ModeOptions, &ModeSpec) -> Rc<dyn Mode>>;

/// A `defineMIME` target: a name, or a spec object.
#[derive(Clone, Debug, PartialEq)]
pub enum MimeTarget {
    Name(String),
    Spec(ModeSpec),
}

/// The `modes` / `mimeModes` tables. JS keeps them global; here they are a value.
#[derive(Clone)]
pub struct Registry {
    modes: HashMap<String, ModeFactory>,
    mime_modes: HashMap<String, MimeTarget>,
}

impl Default for Registry {
    fn default() -> Self {
        Registry::new()
    }
}

/// The `null` mode: `{token: stream => stream.skipToEnd()}`.
struct NullMode {
    name: String,
}

impl Mode for NullMode {
    fn name(&self) -> &str {
        &self.name
    }
    fn token(&self, stream: &mut StringStream<'_>, _state: &mut State) -> Option<String> {
        stream.skip_to_end();
        None
    }
}

/// Wraps a mode to override `name` / `helperType` as `getMode` does.
struct Named {
    inner: Rc<dyn Mode>,
    name: String,
    helper_type: Option<String>,
}

impl Mode for Named {
    fn name(&self) -> &str {
        &self.name
    }
    fn start_state(&self, a1: i64, a2: i64) -> Option<State> {
        self.inner.start_state(a1, a2)
    }
    fn token(&self, stream: &mut StringStream<'_>, state: &mut State) -> Option<String> {
        self.inner.token(stream, state)
    }
    fn blank_line(&self, state: &mut State) -> Option<()> {
        self.inner.blank_line(state)
    }
    fn has_blank_line(&self) -> bool {
        self.inner.has_blank_line()
    }
    fn copy_state(&self, state: &State) -> Option<State> {
        self.inner.copy_state(state)
    }
    fn inner_mode(&self, state: &State) -> Option<(Rc<dyn Mode>, State)> {
        self.inner.inner_mode(state)
    }
    fn indent(&self, state: &State, text_after: &[u16], line: &[u16]) -> Option<i64> {
        self.inner.indent(state, text_after, line)
    }
    fn electric_chars(&self) -> Option<&str> {
        self.inner.electric_chars()
    }
    fn line_comment(&self) -> Option<&str> {
        self.inner.line_comment()
    }
    fn block_comment_start(&self) -> Option<&str> {
        self.inner.block_comment_start()
    }
    fn block_comment_end(&self) -> Option<&str> {
        self.inner.block_comment_end()
    }
    fn helper_type(&self) -> Option<&str> {
        self.helper_type.as_deref().or_else(|| self.inner.helper_type())
    }
}

/// `/^[\w\-]+\/[\w\-]+\+(xml|json)$/` with the given suffix.
fn is_plus_type(s: &str, suffix: &str) -> bool {
    let Some(body) = s.strip_suffix(suffix) else { return false };
    let word = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    match body.split_once('/') {
        Some((a, b)) => word(a) && word(b),
        None => false,
    }
}

impl Registry {
    /// A registry with what `runmode.node.js` (and `codemirror.js`) define up front:
    /// the `null` mode and `text/plain`.
    pub fn new() -> Registry {
        let mut r = Registry { modes: HashMap::new(), mime_modes: HashMap::new() };
        r.define_mode("null", Rc::new(|_, spec| Rc::new(NullMode { name: spec.name.clone() }) as Rc<dyn Mode>));
        r.define_mime("text/plain", MimeTarget::Name("null".into()));
        r
    }
    /// A registry with every mode this crate ports.
    pub fn with_all_modes() -> Registry {
        let mut r = Registry::new();
        toml::register(&mut r);
        r
    }
    /// `defineMode(name, factory)`.
    pub fn define_mode(&mut self, name: &str, factory: ModeFactory) {
        self.modes.insert(name.to_string(), factory);
    }
    /// `defineMIME(mime, spec)`.
    pub fn define_mime(&mut self, mime: &str, spec: MimeTarget) {
        self.mime_modes.insert(mime.to_string(), spec);
    }
    pub fn has_mode(&self, name: &str) -> bool {
        self.modes.contains_key(name)
    }

    /// `resolveMode(spec)`.
    pub fn resolve_mode(&self, spec: &Spec) -> ModeSpec {
        match spec {
            Spec::Str(s) if self.mime_modes.contains_key(s) => match &self.mime_modes[s] {
                // `spec = mimeModes[spec]`, then a string becomes `{name}`.
                MimeTarget::Name(n) => ModeSpec::named(n),
                MimeTarget::Spec(o) => o.clone(),
            },
            Spec::Obj(o) if self.mime_modes.contains_key(&o.name) => {
                // `createObj(found, spec)`: spec's own properties over found's.
                let found = match &self.mime_modes[&o.name] {
                    MimeTarget::Name(n) => ModeSpec::named(n),
                    MimeTarget::Spec(f) => f.clone(),
                };
                let mut out = found.clone();
                for (k, v) in &o.options {
                    out.options.insert(k.clone(), v.clone());
                }
                if o.helper_type.is_some() {
                    out.helper_type = o.helper_type.clone();
                }
                out.name = found.name;
                out
            }
            Spec::Str(s) if is_plus_type(s, "+xml") => self.resolve_mode(&Spec::from("application/xml")),
            Spec::Str(s) if is_plus_type(s, "+json") => self.resolve_mode(&Spec::from("application/json")),
            Spec::Str(s) => ModeSpec::named(s),
            Spec::Obj(o) => o.clone(),
            Spec::Null => ModeSpec::named("null"),
        }
    }

    /// `getMode(options, spec)`: unknown modes fall back to `text/plain`.
    pub fn get_mode(&self, options: &ModeOptions, spec: &Spec) -> Rc<dyn Mode> {
        let spec = self.resolve_mode(spec);
        let Some(factory) = self.modes.get(&spec.name) else {
            return self.get_mode(options, &Spec::from("text/plain"));
        };
        let mode = factory(options, &spec);
        // `modeObj.name = spec.name; if (spec.helperType) modeObj.helperType = ...`
        Rc::new(Named { inner: mode, name: spec.name.clone(), helper_type: spec.helper_type.clone() })
    }
}

/// `copyState(mode, state)`. The generic JS copy is shallow apart from arrays;
/// `State` values are cloned, which equals that for the plain-data states of the
/// ported modes.
pub fn copy_state(mode: &dyn Mode, state: &State) -> State {
    if let State::True = state {
        return State::True;
    }
    mode.copy_state(state).unwrap_or_else(|| state.clone())
}

/// `innerMode(mode, state)`.
pub fn inner_mode(mode: Rc<dyn Mode>, state: State) -> (Rc<dyn Mode>, State) {
    let mut cur = (mode, state);
    let mut info = None;
    loop {
        let next = cur.0.inner_mode(&cur.1);
        match next {
            Some((m, s)) if !Rc::ptr_eq(&m, &cur.0) => {
                info = Some((m.clone(), s.clone()));
                cur = (m, s);
            }
            _ => break,
        }
    }
    info.unwrap_or(cur)
}

/// `startState(mode, a1, a2)`.
pub fn start_state(mode: &dyn Mode, a1: i64, a2: i64) -> State {
    mode.start_state(a1, a2).unwrap_or(State::True)
}

/// Raised where the JS throws `"Mode " + name + " failed to advance stream."`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailedToAdvance(pub String);

impl std::fmt::Display for FailedToAdvance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Mode {} failed to advance stream.", self.0)
    }
}

/// `readToken(mode, stream, state, inner)` from `src/line/highlight.js`: up to ten
/// tries for the mode to consume something.
pub fn read_token(mode: &Rc<dyn Mode>, stream: &mut StringStream<'_>, state: &mut State, mut inner: Option<&mut Option<Rc<dyn Mode>>>) -> Result<Option<String>, FailedToAdvance> {
    for _ in 0..10 {
        if let Some(slot) = inner.as_deref_mut() {
            *slot = Some(inner_mode(mode.clone(), state.clone()).0);
        }
        let style = mode.token(stream, state);
        if stream.pos > stream.start {
            return Ok(style);
        }
    }
    Err(FailedToAdvance(mode.name().to_string()))
}

// --------------------------------------------------------------------------------
// runMode

/// `CodeMirror.splitLines` in `runmode.node.js`: `string.split(/\r?\n|\r/)`.
pub fn split_lines_runmode(s: &[u16]) -> Vec<Text> {
    let mut out = Vec::new();
    let mut cur = Vec::new();
    let mut i = 0;
    while i < s.len() {
        match s[i] {
            0x0d if s.get(i + 1) == Some(&0x0a) => {
                out.push(std::mem::take(&mut cur));
                i += 2;
            }
            0x0d | 0x0a => {
                out.push(std::mem::take(&mut cur));
                i += 1;
            }
            c => {
                cur.push(c);
                i += 1;
            }
        }
    }
    out.push(cur);
    out
}

/// One `callback(text, style, lineNo, start, state, mode)` call of `runMode`.
/// Line breaks arrive as `callback("\n")`: `line`/`start` are `None` then.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunToken {
    pub text: Text,
    pub style: Option<String>,
    pub line: Option<usize>,
    pub start: Option<usize>,
}

struct LinesOracle<'a> {
    lines: &'a [Text],
    i: usize,
}

impl LineOracle for LinesOracle<'_> {
    /// `lookAhead: n => lines[i + n]`.
    fn look_ahead(&self, n: i64) -> Option<Text> {
        let k = self.i as i64 + n;
        if k < 0 { None } else { self.lines.get(k as usize).cloned() }
    }
    /// `baseToken: function() {}`.
    fn base_token(&self, _pos: usize) -> Option<(Option<String>, i64)> {
        None
    }
}

/// `CodeMirror.runMode(string, modespec, callback, options)` from
/// `addon/runmode/runmode.node.js` (callback form). Like the JS there is no guard
/// against a mode that never advances: such a mode loops forever.
///
/// The JS passes `null` as the stream's tab size, so streams always use 8 —
/// `options.tabSize` only affects the DOM output form, which is not ported.
pub fn run_mode<F: FnMut(&RunToken)>(registry: &Registry, string: &str, spec: &Spec, callback: F, state: Option<State>) {
    run_mode_utf16(registry, &cm_core::text::to_u16(string), spec, callback, state)
}

pub fn run_mode_utf16<F: FnMut(&RunToken)>(registry: &Registry, string: &[u16], spec: &Spec, mut callback: F, state: Option<State>) {
    let options = ModeOptions::default();
    let mode = registry.get_mode(&options, spec);
    let lines = split_lines_runmode(string);
    // `(options && options.state) || CodeMirror.startState(mode)`
    let mut state = state.unwrap_or_else(|| start_state(&*mode, 0, 0));
    for i in 0..lines.len() {
        if i > 0 {
            callback(&RunToken { text: vec![0x0a], style: None, line: None, start: None });
        }
        let oracle = LinesOracle { lines: &lines, i };
        let mut stream = StringStream::new(&lines[i], None, Some(&oracle));
        if stream.string.is_empty() && mode.has_blank_line() {
            mode.blank_line(&mut state);
        }
        while !stream.eol() {
            let style = mode.token(&mut stream, &mut state);
            callback(&RunToken { text: stream.current().to_vec(), style, line: Some(i), start: Some(stream.start) });
            stream.start = stream.pos;
        }
    }
}

/// Convenience: `runMode` collected into `(text, style)` pairs, line breaks
/// included as `("\n", None)`.
pub fn run_mode_collect(registry: &Registry, string: &str, spec: &Spec) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    run_mode(registry, string, spec, |t| out.push((from_u16(&t.text), t.style.clone())), None);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_mode_rules() {
        let r = Registry::with_all_modes();
        assert_eq!(r.resolve_mode(&Spec::from("text/x-toml")).name, "toml");
        assert_eq!(r.resolve_mode(&Spec::from("text/plain")).name, "null");
        assert_eq!(r.resolve_mode(&Spec::from("toml")).name, "toml");
        assert_eq!(r.resolve_mode(&Spec::Null).name, "null");
        assert_eq!(r.resolve_mode(&Spec::from("image/svg+xml")).name, "application/xml");
        assert_eq!(r.resolve_mode(&Spec::from("application/ld+json")).name, "application/json");
        let mut o = ModeSpec::named("text/x-toml");
        o.options.insert("x".into(), ConfigValue::Bool(true));
        let res = r.resolve_mode(&Spec::Obj(o));
        assert_eq!(res.name, "toml");
        assert_eq!(res.options.get("x"), Some(&ConfigValue::Bool(true)));
        // Unknown modes fall back to text/plain, whose mode is named "null".
        assert_eq!(r.get_mode(&ModeOptions::default(), &Spec::from("nope")).name(), "null");
        assert_eq!(r.get_mode(&ModeOptions::default(), &Spec::from("text/x-toml")).name(), "toml");
    }

    #[test]
    fn null_mode_run() {
        let r = Registry::new();
        let toks = run_mode_collect(&r, "ab\r\ncd\r\n", &Spec::from("text/plain"));
        assert_eq!(toks, vec![("ab".into(), None), ("\n".into(), None), ("cd".into(), None), ("\n".into(), None)]);
    }
}
