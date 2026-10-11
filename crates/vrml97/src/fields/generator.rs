//! Output-stream state of X_ITE's generator (src/x_ite/InputOutput/X3DGenerator.js), without
//! the scene bookkeeping (names, execution contexts), which lives in `x3d-scene`.

use crate::fields::format;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Clean,
    Small,
    Compact,
    Tidy,
}

#[derive(Clone, Debug)]
pub struct Generator {
    pub string: String,
    pub indent: String,
    pub list_indent_str: String,
    pub needs_space: bool,
    pub precision: usize,
    pub double_precision: usize,
    comma: &'static str,
    brk: &'static str,
    tidy_break: &'static str,
    tidy_space: &'static str,
    indent_char: &'static str,
    list_enclosure: &'static str,
    list_break: &'static str,
    list_indent_char: &'static str,
    attrib_break: &'static str,
}

impl Default for Generator {
    fn default() -> Self {
        Self::new(Style::Tidy)
    }
}

impl Generator {
    pub fn new(style: Style) -> Self {
        let mut g = Generator {
            string: String::new(),
            indent: String::new(),
            list_indent_str: String::new(),
            needs_space: false,
            precision: format::FLOAT_PRECISION,
            double_precision: format::DOUBLE_PRECISION,
            comma: ",",
            brk: "\n",
            tidy_break: "\n",
            tidy_space: " ",
            indent_char: "  ",
            list_enclosure: "\n",
            list_break: "\n",
            list_indent_char: "  ",
            attrib_break: "\n",
        };

        match style {
            Style::Clean => {
                g.comma = " ";
                g.brk = " ";
                g.tidy_break = "";
                g.tidy_space = "";
                g.indent_char = "";
                g.list_enclosure = "";
                g.list_break = "";
                g.list_indent_char = "";
                g.attrib_break = " ";
            }
            Style::Small => {
                g.tidy_space = "";
                g.indent_char = "";
                g.list_enclosure = "";
                g.list_break = "";
                g.list_indent_char = "";
                g.attrib_break = " ";
            }
            Style::Compact => {
                g.list_enclosure = " ";
                g.list_break = " ";
                g.list_indent_char = "";
                g.attrib_break = " ";
            }
            Style::Tidy => {}
        }

        g
    }

    pub fn float_format(&self, v: f64) -> String {
        format::format_number(v, self.precision)
    }

    pub fn double_format(&self, v: f64) -> String {
        format::format_number(v, self.double_precision)
    }

    pub fn push(&mut self, s: &str) {
        self.string.push_str(s);
    }

    pub fn comma(&mut self) {
        self.string.push_str(self.comma);
    }

    pub fn brk(&mut self) {
        self.needs_space = false;
        self.string.push_str(self.brk);
    }

    pub fn tidy_break(&mut self) {
        self.needs_space = self.needs_space && self.tidy_break.is_empty();
        self.string.push_str(self.tidy_break);
    }

    pub fn space(&mut self) {
        self.needs_space = false;
        self.string.push(' ');
    }

    pub fn tidy_space(&mut self) {
        self.needs_space = self.needs_space && self.tidy_space.is_empty();
        self.string.push_str(self.tidy_space);
    }

    /// The side effect of `generator .TidySpace ()` when it is used as the second argument of
    /// `padEnd` inside `generator .string += …`: the appended text is overwritten by the outer
    /// assignment, only `needsSpace` changes (and `padEnd (n, undefined)` pads with spaces).
    pub fn tidy_space_lost(&mut self) {
        self.needs_space = self.needs_space && self.tidy_space.is_empty();
    }

    pub fn list_start(&mut self) {
        self.string.push_str(self.list_enclosure);
    }

    pub fn list_end(&mut self) {
        self.string.push_str(self.list_enclosure);
    }

    pub fn list_break(&mut self) {
        self.string.push_str(self.list_break);
    }

    pub fn attrib_break(&mut self) {
        self.string.push_str(self.attrib_break);
    }

    pub fn indent(&mut self) {
        let i = self.indent.clone();
        self.string.push_str(&i);
    }

    pub fn list_indent(&mut self) {
        let i = self.list_indent_str.clone();
        self.string.push_str(&i);
    }

    pub fn inc_indent(&mut self) {
        self.indent.push_str(self.indent_char);
        self.list_indent_str.push_str(self.list_indent_char);
    }

    pub fn dec_indent(&mut self) {
        let n = self.indent.len().saturating_sub(self.indent_char.len());
        self.indent.truncate(n);
        let n = self.list_indent_str.len().saturating_sub(self.list_indent_char.len());
        self.list_indent_str.truncate(n);
    }

    pub fn set_needs_space(&mut self) {
        self.needs_space = true;
    }

    pub fn check_space(&mut self) {
        if self.needs_space {
            self.string.push(' ');
        }
        self.needs_space = false;
    }

    /// VRMLGenerator.NULL.
    pub fn null(&mut self) {
        self.check_space();
        self.string.push_str("NULL");
        self.set_needs_space();
    }

    /// Space-separated numbers (SFVec/SFColor/SFMatrix/SFRotation toStream).
    pub fn numbers(&mut self, values: &[f64], double: bool) {
        let last = values.len() - 1;

        for (i, v) in values.iter().enumerate() {
            let s = if double { self.double_format(*v) } else { self.float_format(*v) };
            self.string.push_str(&s);

            if i != last {
                self.space();
            }
        }
    }

    /// The list layout shared by X3DTypedArrayField.toStream and X3DObjectArrayField.toStream.
    pub fn typed_list(&mut self, length: usize, mut element: impl FnMut(&mut Generator, usize)) {
        match length {
            0 => {
                self.string.push('[');
                self.tidy_space();
                self.string.push(']');
            }
            1 => element(self, 0),
            _ => {
                let last = length - 1;

                self.string.push('[');
                self.list_start();
                self.inc_indent();

                for i in 0..last {
                    self.list_indent();
                    element(self, i);
                    self.comma();
                    self.list_break();
                }

                self.list_indent();
                element(self, last);

                self.list_end();
                self.dec_indent();
                self.list_indent();
                self.string.push(']');
            }
        }
    }
}
