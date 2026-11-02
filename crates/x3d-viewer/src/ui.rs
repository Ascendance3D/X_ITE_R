//! The native browser UI: X_ITE's ContextMenu and Notification (`Browser/Core/ContextMenu.js`,
//! `Notification.js`, pinned 16.5.0), drawn by a software canvas that the window composites over
//! the world ([`crate::ui_text`]).
//!
//! The menu model follows `ContextMenu.build`: the same entries, texts and conditions, the same
//! rules for closing (a press on the layer or a second `contextmenu` hides it; an entry that is not
//! a checkbox or radio hides it after its callback; the browser has no menu keys), and the same
//! descriptions. The look is a plain light theme: no fade, blur, icons or animation (the web build
//! has them).
//!
//! Differences from X_ITE (each is also a line of the PR that added this module):
//!
//! * The look: Ubuntu (X_ITE's default sans font) instead of `X_ITE PT Sans`, no logo or icons, no fade, blur or
//!   scroll bars; English texts only; the menu title and the About entry name this port and its repository.
//! * The menu opens on the host's `contextmenu` event: the right button's press (Linux and Windows browsers), Ctrl+click
//!   (macOS, [`crate::keys::Platform::press_events`]), and PointingDevice's long tap (1200 ms, 7 px, no sensor;
//!   [`crate::viewer::Viewer3d::input`] with the touch events of [`crate::touch`]).
//! * The Notification grows over 300 ms with a smoothstep curve, where CSS `ease-in-out` is a cubic Bezier.
//! * The Browser Timings entry and Ctrl+Plus switch the `Timings` option; the panel is [`crate::ui_timings`].
//! * The Fullscreen entry asks the window for borderless fullscreen; About opens the system's URL opener.
//! * A description that the world itself sets to the text it already has (a Script's `browser.description`) shows
//!   again only through [`crate::viewer::Viewer3d::set_description`]; the per-frame check of the runtime's description
//!   sees changes only.

use crate::ui_text::{Canvas, Color, Text};
use x3d_fields::NodeId;

/// Font size of the menu and the notification (CSS `10pt` is 13.3 px).
const FONT: f64 = 13.0;
const ITEM_H: f64 = 26.0;
const SEP_H: f64 = 9.0;
const TITLE_H: f64 = 32.0;
const PAD_L: f64 = 30.0;
const PAD_R: f64 = 26.0;
const MIN_W: f64 = 150.0;
/// Overlap of a submenu with its parent (`ul .clientWidth - 28`).
const OVERLAP: f64 = 8.0;
/// Notification: `setTimeout (…, 5000)` and `transition: width 300ms`.
pub const NOTIFICATION_MS: f64 = 5.0;
pub const NOTIFICATION_FADE: f64 = 0.3;

const BG: Color = [244, 244, 244, 238];
const BORDER: Color = [150, 150, 150, 255];
const TEXT: Color = [36, 36, 36, 255];
const TEXT_DIM: Color = [110, 110, 110, 255];
const HOVER: Color = [196, 218, 250, 255];
const MARK: Color = [40, 90, 170, 255];

/// What a browser option is set to.
#[derive(Clone, Debug, PartialEq)]
pub enum OptionSet {
    Bool(bool),
    Str(&'static str),
}

/// What an entry does when it is clicked.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// A radio entry: set the option; the description is `"{label}: {value}"`.
    SetOption {
        name: &'static str,
        value: OptionSet,
        description: String,
    },
    /// A checkbox entry: flip the boolean option. `label`: the description is `"{label}: on|off"`.
    Toggle {
        name: &'static str,
        label: Option<&'static str>,
    },
    /// `browser .bindViewpoint (activeLayer, viewpoint)`.
    Viewpoint(NodeId),
    /// `browser ._viewer = viewer`, then the description is the viewer's name.
    Viewer(String),
    Fullscreen,
    WorldInfo,
    About,
    /// A TimeSensor of the exported `Animations` node: start it (looping) or stop it.
    Animation(NodeId),
    AnimationsAll,
    AnimationsNone,
    /// An entry of the host's user menu ([`crate::viewer::Viewer3d::set_user_menu`]).
    User(u32),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// The first row: `X_ITE v16.5.0`; not selectable.
    Title,
    Separator,
    Plain,
    Check(bool),
    /// Radio entries of one `group` exclude each other.
    Radio {
        group: &'static str,
        selected: bool,
    },
    Submenu(Vec<Item>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub text: String,
    pub kind: Kind,
    pub action: Option<Action>,
}

impl Item {
    pub fn plain(text: &str, action: Action) -> Item {
        Item {
            text: text.to_string(),
            kind: Kind::Plain,
            action: Some(action),
        }
    }

    pub fn check(text: &str, selected: bool, action: Action) -> Item {
        Item {
            text: text.to_string(),
            kind: Kind::Check(selected),
            action: Some(action),
        }
    }

    pub fn radio(text: &str, group: &'static str, selected: bool, action: Action) -> Item {
        Item {
            text: text.to_string(),
            kind: Kind::Radio { group, selected },
            action: Some(action),
        }
    }

    pub fn submenu(text: &str, items: Vec<Item>) -> Item {
        Item {
            text: text.to_string(),
            kind: Kind::Submenu(items),
            action: None,
        }
    }

    pub fn separator() -> Item {
        Item {
            text: String::new(),
            kind: Kind::Separator,
            action: None,
        }
    }

    fn selectable(&self) -> bool {
        !matches!(self.kind, Kind::Title | Kind::Separator)
    }

    fn height(&self) -> f64 {
        match self.kind {
            Kind::Title => TITLE_H,
            Kind::Separator => SEP_H,
            _ => ITEM_H,
        }
    }
}

/// A request for the window.
#[derive(Clone, Debug, PartialEq)]
pub enum UiRequest {
    /// The Fullscreen entry.
    ToggleFullscreen,
    /// The About entry (`window .open (browser .getProviderURL ())`).
    OpenUrl(String),
    /// Shift+F8: the text for the clipboard.
    Clipboard(String),
}

/// The geometry of one open list, in canvas pixels from the top left.
#[derive(Clone, Debug)]
struct Level {
    /// The path of the list's owner item (empty for the root).
    owner: Vec<usize>,
    x: f64,
    y: f64,
    w: f64,
    rows: Vec<(f64, f64)>,
}

impl Level {
    fn height(&self) -> f64 {
        self.rows.last().map(|(y, h)| y + h).unwrap_or(0.0)
    }

    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.height()
    }

    fn row_at(&self, y: f64) -> Option<usize> {
        self.rows
            .iter()
            .position(|(top, h)| y >= self.y + top && y < self.y + top + h)
    }
}

/// An open context menu.
#[derive(Clone, Debug)]
pub struct Menu {
    pub items: Vec<Item>,
    /// Where the menu was asked for (the event's page position), top-left canvas pixels.
    origin: (f64, f64),
    /// The canvas the menu is laid out in.
    canvas: (f64, f64),
    /// The row under the pointer at each level.
    hover: Vec<usize>,
    /// The entry that the left button went down on.
    pressed: Option<Vec<usize>>,
    /// The widths of the lists, by owner path.
    widths: Vec<(Vec<usize>, f64)>,
}

fn list_at<'a>(items: &'a [Item], owner: &[usize]) -> Option<&'a [Item]> {
    let mut list = items;

    for i in owner {
        match &list.get(*i)?.kind {
            Kind::Submenu(sub) => list = sub,
            _ => return None,
        }
    }

    Some(list)
}

fn item_at<'a>(items: &'a [Item], path: &[usize]) -> Option<&'a Item> {
    let (last, owner) = path.split_last()?;

    list_at(items, owner)?.get(*last)
}

fn item_at_mut<'a>(items: &'a mut [Item], path: &[usize]) -> Option<&'a mut Item> {
    let (last, owner) = path.split_last()?;
    let mut list = items;

    for i in owner {
        match &mut list.get_mut(*i)?.kind {
            Kind::Submenu(sub) => list = sub,
            _ => return None,
        }
    }

    list.get_mut(*last)
}

impl Menu {
    pub fn new(items: Vec<Item>, origin: (f64, f64), canvas: (f64, f64), text: &Text) -> Menu {
        let mut widths = vec![];

        fn measure(
            items: &[Item],
            owner: &mut Vec<usize>,
            text: &Text,
            out: &mut Vec<(Vec<usize>, f64)>,
        ) {
            let w = items
                .iter()
                .map(|i| text.width(&i.text, FONT))
                .fold(0.0, f64::max);

            out.push((owner.clone(), (w + PAD_L + PAD_R).max(MIN_W)));

            for (k, item) in items.iter().enumerate() {
                if let Kind::Submenu(sub) = &item.kind {
                    owner.push(k);
                    measure(sub, owner, text, out);
                    owner.pop();
                }
            }
        }

        measure(&items, &mut vec![], text, &mut widths);

        Menu {
            items,
            origin,
            canvas,
            hover: vec![],
            pressed: None,
            widths,
        }
    }

    fn width_of(&self, owner: &[usize]) -> f64 {
        self.widths
            .iter()
            .find(|(o, _)| o == owner)
            .map(|(_, w)| *w)
            .unwrap_or(MIN_W)
    }

    fn rows_of(list: &[Item]) -> Vec<(f64, f64)> {
        let mut y = 0.0;

        list.iter()
            .map(|i| {
                let row = (y, i.height());

                y += i.height();
                row
            })
            .collect()
    }

    /// The open lists: the root, and a submenu for each hovered entry that has one.
    fn levels(&self) -> Vec<Level> {
        let (cw, ch) = self.canvas;
        let rows = Menu::rows_of(&self.items);
        let w = self.width_of(&[]);
        let h = rows.last().map(|(y, h)| y + h).unwrap_or(0.0);

        // "Reposition menu if to right or to low."
        let x = if self.origin.0 + w > cw {
            (cw - w).max(0.0)
        } else {
            self.origin.0
        };
        let y = if self.origin.1 + h > ch {
            (ch - h).max(0.0)
        } else {
            self.origin.1
        };
        let mut levels = vec![Level {
            owner: vec![],
            x,
            y,
            w,
            rows,
        }];

        for depth in 0..self.hover.len() {
            let owner: Vec<usize> = self.hover[..=depth].to_vec();
            let Some(item) = item_at(&self.items, &owner) else {
                break;
            };
            let Kind::Submenu(sub) = &item.kind else {
                break;
            };
            let parent = &levels[depth];
            let rows = Menu::rows_of(sub);
            let sw = self.width_of(&owner);
            let sh = rows.last().map(|(y, h)| y + h).unwrap_or(0.0);
            // "Display submenus on the left or right side."
            let sx = if parent.x + parent.w - OVERLAP + sw > cw {
                parent.x - sw + OVERLAP
            } else {
                parent.x + parent.w - OVERLAP
            };
            let mut sy = parent.y + parent.rows[owner[depth]].0;

            // "Submenu goes out of page."
            if sy + sh > ch {
                sy = (ch - sh).max(0.0);
            }

            levels.push(Level {
                owner,
                x: sx,
                y: sy,
                w: sw,
                rows,
            });
        }

        levels
    }

    /// The entry under a point: its path.
    pub fn hit(&self, x: f64, y: f64) -> Option<Vec<usize>> {
        let levels = self.levels();

        for (depth, level) in levels.iter().enumerate().rev() {
            if level.contains(x, y) {
                let row = level.row_at(y)?;
                let mut path = level.owner.clone();

                path.push(row);
                debug_assert_eq!(path.len(), depth + 1);
                return Some(path);
            }
        }

        None
    }

    fn inside(&self, x: f64, y: f64) -> bool {
        self.levels().iter().any(|l| l.contains(x, y))
    }

    /// The pointer moved: the hover path follows (CSS `:hover` opens the submenus).
    pub fn pointer(&mut self, x: f64, y: f64) -> bool {
        let path = match self.hit(x, y) {
            Some(p) if item_at(&self.items, &p).is_some_and(Item::selectable) => p,
            _ => vec![],
        };
        let changed = path != self.hover;

        self.hover = path;
        changed
    }

    pub fn hover(&self) -> &[usize] {
        &self.hover
    }

    /// The path of the entry with this text, in the list of `owner`.
    pub fn find(&self, owner: &[usize], text: &str) -> Option<Vec<usize>> {
        let list = list_at(&self.items, owner)?;
        let i = list.iter().position(|i| i.text == text)?;
        let mut path = owner.to_vec();

        path.push(i);
        Some(path)
    }

    /// The center of an entry (for the pointer), when its list is open.
    pub fn center_of(&self, path: &[usize]) -> Option<(f64, f64)> {
        let levels = self.levels();
        let (last, owner) = path.split_last()?;
        let level = levels.iter().find(|l| l.owner == owner)?;
        let (top, h) = level.rows.get(*last)?;

        Some((level.x + level.w / 2.0, level.y + top + h / 2.0))
    }

    /// The text of the entries of a list.
    pub fn texts(&self, owner: &[usize]) -> Vec<String> {
        list_at(&self.items, owner)
            .map(|l| l.iter().map(|i| i.text.clone()).collect())
            .unwrap_or_default()
    }

    /// An entry by path.
    pub fn item(&self, path: &[usize]) -> Option<&Item> {
        item_at(&self.items, path)
    }

    /// The pointer is over a checkbox/radio state, or a plain entry: after a click the checkbox flips and the
    /// radio excludes its group.
    fn click(&mut self, path: &[usize]) -> Option<(Action, bool)> {
        let kind = item_at(&self.items, path)?.kind.clone();
        let action = item_at(&self.items, path)?.action.clone()?;

        match kind {
            Kind::Check(on) => {
                if let Some(i) = item_at_mut(&mut self.items, path) {
                    i.kind = Kind::Check(!on);
                }

                Some((action, false))
            }
            Kind::Radio { group, .. } => {
                let (last, owner) = path.split_last()?;

                if let Some(list) = list_at_mut(&mut self.items, owner) {
                    for (k, i) in list.iter_mut().enumerate() {
                        if let Kind::Radio { group: g, selected } = &mut i.kind {
                            if *g == group {
                                *selected = k == *last;
                            }
                        }
                    }
                }

                Some((action, false))
            }
            Kind::Plain => Some((action, true)),
            _ => None,
        }
    }
}

fn list_at_mut<'a>(items: &'a mut [Item], owner: &[usize]) -> Option<&'a mut [Item]> {
    let mut list = items;

    for i in owner {
        match &mut list.get_mut(*i)?.kind {
            Kind::Submenu(sub) => list = sub,
            _ => return None,
        }
    }

    Some(list)
}

/// What the menu did with a mouse event.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuOutcome {
    /// The event went to the menu (or its layer) and nothing else happens.
    Consumed,
    /// The menu is closed.
    Closed,
    /// An entry was clicked; `close`: the menu closes after the callback.
    Action { action: Action, close: bool },
}

impl Menu {
    /// `mousedown` on the layer or the menu. The layer hides the menu on any button; the menu itself on the context
    /// menu event (the right button).
    pub fn down(&mut self, button: u8, x: f64, y: f64) -> MenuOutcome {
        if !self.inside(x, y) || button == 2 {
            return MenuOutcome::Closed;
        }

        self.pressed = if button == 0 { self.hit(x, y) } else { None };
        self.pointer(x, y);
        MenuOutcome::Consumed
    }

    /// `click`: the button went up on the entry it went down on.
    pub fn up(&mut self, button: u8, x: f64, y: f64) -> MenuOutcome {
        let pressed = self.pressed.take();

        if button != 0 {
            return MenuOutcome::Consumed;
        }

        match (pressed, self.hit(x, y)) {
            (Some(a), Some(b)) if a == b => match self.click(&a) {
                Some((action, close)) => MenuOutcome::Action { action, close },
                None => MenuOutcome::Consumed,
            },
            _ => MenuOutcome::Consumed,
        }
    }

    /// Draw the menu.
    pub fn paint(&self, canvas: &mut Canvas, text: &mut Text) {
        let levels = self.levels();

        for (depth, level) in levels.iter().enumerate() {
            let list = list_at(&self.items, &level.owner).unwrap_or(&[]);
            let h = level.height();

            canvas.fill_rect(level.x, level.y, level.w, h, [6.0; 4], BG);
            canvas.stroke_rect(level.x, level.y, level.w, h, BORDER);

            for (k, item) in list.iter().enumerate() {
                let (top, rh) = level.rows[k];
                let y = level.y + top;
                let hovered = self.hover.get(depth) == Some(&k) && item.selectable();

                if hovered {
                    canvas.fill_rect(level.x + 2.0, y, level.w - 4.0, rh, [4.0; 4], HOVER);
                }

                if matches!(item.kind, Kind::Separator) {
                    canvas.fill_rect(
                        level.x + 8.0,
                        y + rh / 2.0,
                        level.w - 16.0,
                        1.0,
                        [0.0; 4],
                        BORDER,
                    );
                    continue;
                }

                let baseline = y + (rh - text.line_height(FONT)) / 2.0 + text.ascent(FONT);
                let color = if matches!(item.kind, Kind::Title) {
                    TEXT_DIM
                } else {
                    TEXT
                };

                text.draw(canvas, &item.text, level.x + PAD_L, baseline, FONT, color);

                let cy = y + rh / 2.0;

                match &item.kind {
                    Kind::Check(on) => {
                        canvas.stroke_rect(level.x + 9.0, cy - 6.0, 12.0, 12.0, TEXT);

                        if *on {
                            canvas.fill_rect(level.x + 12.0, cy - 3.0, 6.0, 6.0, [0.0; 4], MARK);
                        }
                    }
                    Kind::Radio { selected, .. } => {
                        canvas.fill_rect(level.x + 9.0, cy - 6.0, 12.0, 12.0, [6.0; 4], TEXT);
                        canvas.fill_rect(level.x + 10.0, cy - 5.0, 10.0, 10.0, [5.0; 4], BG);

                        if *selected {
                            canvas.fill_rect(level.x + 12.0, cy - 3.0, 6.0, 6.0, [3.0; 4], MARK);
                        }
                    }
                    Kind::Submenu(_) => {
                        text.draw(
                            canvas,
                            "›",
                            level.x + level.w - 16.0,
                            baseline,
                            FONT + 3.0,
                            TEXT,
                        );
                    }
                    _ => {}
                }
            }
        }
    }
}

/// The World Info dialog of the Show World Info entry.
#[derive(Clone, Debug)]
pub struct WorldInfoPanel {
    pub title: String,
    pub info: Vec<String>,
    pub meta: Vec<(String, String)>,
    /// 0: World Info, 1: Metadata.
    pub tab: usize,
}

impl WorldInfoPanel {
    /// `hasWorldInfo`: a title or info lines.
    pub fn has_world_info(&self) -> bool {
        !self.title.is_empty() || !self.info.is_empty()
    }

    pub fn new(title: String, info: Vec<String>, meta: Vec<(String, String)>) -> WorldInfoPanel {
        let mut p = WorldInfoPanel {
            title,
            info,
            meta,
            tab: 0,
        };

        // `if (hasWorldInfo) worldInfoButton .click (); else metaDataButton .click ()`.
        p.tab = if p.has_world_info() { 0 } else { 1 };
        p
    }

    /// The tabs that are shown (a button is hidden when it has nothing).
    pub fn tabs(&self) -> Vec<(usize, &'static str)> {
        let mut t = vec![];

        if self.has_world_info() {
            t.push((0, "World Info"));
        }

        if !self.meta.is_empty() {
            t.push((1, "Metadata"));
        }

        t
    }

    fn rect(canvas: (f64, f64)) -> (f64, f64, f64, f64) {
        let w = (canvas.0 * 0.7).clamp(200.0, 760.0).min(canvas.0);
        let h = (canvas.1 * 0.7).min(canvas.1);

        ((canvas.0 - w) / 2.0, (canvas.1 - h) / 2.0, w, h)
    }

    fn tab_rect(&self, canvas: (f64, f64), k: usize) -> (f64, f64, f64, f64) {
        let (x, y, ..) = WorldInfoPanel::rect(canvas);

        (x + 12.0 + k as f64 * 112.0, y + 10.0, 104.0, 24.0)
    }

    /// A click on the overlay: a tab button switches the tab, anything else closes the dialog.
    /// Returns whether the dialog stays.
    pub fn click(&mut self, canvas: (f64, f64), x: f64, y: f64) -> bool {
        for (k, (tab, _)) in self.tabs().iter().enumerate() {
            let (rx, ry, rw, rh) = self.tab_rect(canvas, k);

            if x >= rx && x < rx + rw && y >= ry && y < ry + rh {
                self.tab = *tab;
                return true;
            }
        }

        false
    }

    pub fn paint(&self, canvas: &mut Canvas, text: &mut Text) {
        let size = (canvas.width as f64, canvas.height as f64);
        let (x, y, w, h) = WorldInfoPanel::rect(size);

        canvas.fill_rect(0.0, 0.0, size.0, size.1, [0.0; 4], [0, 0, 0, 90]);
        canvas.fill_rect(x, y, w, h, [10.0; 4], [250, 250, 250, 245]);
        canvas.stroke_rect(x, y, w, h, BORDER);

        for (k, (tab, name)) in self.tabs().iter().enumerate() {
            let (rx, ry, rw, rh) = self.tab_rect(size, k);

            canvas.fill_rect(
                rx,
                ry,
                rw,
                rh,
                [6.0; 4],
                if *tab == self.tab { HOVER } else { BG },
            );
            canvas.stroke_rect(rx, ry, rw, rh, BORDER);
            text.draw(canvas, name, rx + 12.0, ry + 17.0, FONT, TEXT);
        }

        let mut line_y = y + 56.0;
        let left = x + 16.0;
        let lh = text.line_height(FONT) + 4.0;
        let mut put = |canvas: &mut Canvas, text: &mut Text, s: &str, size: f64, color: Color| {
            for line in wrap(text, s, w - 32.0, size) {
                if line_y + lh <= y + h {
                    text.draw(canvas, &line, left, line_y, size, color);
                }

                line_y += lh;
            }
        };

        if self.tab == 0 {
            put(canvas, text, "World Info", FONT + 3.0, TEXT_DIM);

            if !self.title.is_empty() {
                put(canvas, text, &self.title, FONT + 2.0, TEXT);
            }

            for line in &self.info {
                put(canvas, text, line, FONT, TEXT);
            }
        } else {
            put(canvas, text, "Metadata", FONT + 3.0, TEXT_DIM);

            for (key, value) in &self.meta {
                put(canvas, text, &format!("{key}: {value}"), FONT, TEXT);
            }
        }
    }
}

/// Break a string at spaces so that the lines fit `width`.
fn wrap(text: &Text, s: &str, width: f64, size: f64) -> Vec<String> {
    let mut lines = vec![];
    let mut line = String::new();

    for word in s.split(' ') {
        let candidate = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };

        if !line.is_empty() && text.width(&candidate, size) > width {
            lines.push(std::mem::take(&mut line));
            line = word.to_string();
        } else {
            line = candidate;
        }
    }

    lines.push(line);
    lines
}

/// The notification: the last description, for five seconds.
#[derive(Clone, Debug, Default)]
pub struct Notification {
    pub text: String,
    /// When it was set (the viewer's clock, seconds).
    pub since: f64,
}

impl Notification {
    /// `set_string__`: an empty string changes nothing.
    pub fn set(&mut self, text: &str, now: f64) {
        if !text.is_empty() {
            self.text = text.to_string();
            self.since = now;
        }
    }

    /// The share of the full width at `now`: 0 hidden, 1 full. The width transition takes 300 ms in and out, and the
    /// visibility ends after the 5000 ms timeout (`visibility 0s 300ms`).
    pub fn extent(&self, now: f64) -> f64 {
        if self.text.is_empty() {
            return 0.0;
        }

        let t = now - self.since;

        if t < 0.0 || t >= NOTIFICATION_MS + NOTIFICATION_FADE {
            return 0.0;
        }

        let ease = |x: f64| x * x * (3.0 - 2.0 * x);

        if t < NOTIFICATION_FADE {
            ease(t / NOTIFICATION_FADE)
        } else if t < NOTIFICATION_MS {
            1.0
        } else {
            ease(1.0 - (t - NOTIFICATION_MS) / NOTIFICATION_FADE)
        }
    }

    /// The pill's size and its position in a canvas of `width`: `top: 20px; right: 0`.
    pub fn rect(&self, text: &Text, width: f64, now: f64) -> Option<(f64, f64, f64, f64)> {
        let e = self.extent(now);

        if e <= 0.0 {
            return None;
        }

        let full = text.width(&self.text, FONT) + 7.0 + 22.0 + 16.0;
        let w = (full * e).max(1.0);

        Some((width - w, 20.0, w, text.line_height(FONT) + 10.0))
    }

    pub fn paint(&self, canvas: &mut Canvas, text: &mut Text, origin: (f64, f64), now: f64) {
        let Some((x, y, w, h)) = self.rect(text, canvas.width as f64 + origin.0, now) else {
            return;
        };
        let (x, y) = (x - origin.0, y - origin.1);

        canvas.fill_rect(x, y, w, h, [16.0, 0.0, 0.0, 16.0], BG);
        canvas.stroke_rect(x, y, w, h, BORDER);

        // The text is clipped by the growing pill: draw it on a scratch canvas of the pill and copy it.
        let mut scratch = Canvas::new(w.ceil() as u32, h.ceil() as u32);
        let baseline = 5.0 + text.ascent(FONT);

        text.draw(&mut scratch, &self.text, 7.0 + 22.0, baseline, FONT, TEXT);

        for py in 0..scratch.height {
            for px in 0..scratch.width {
                let p = scratch.pixel(px, py);

                if p[3] > 0 {
                    let (dx, dy) = (x.floor() as i64 + px as i64, y.floor() as i64 + py as i64);

                    if dx >= 0
                        && dy >= 0
                        && (dx as u32) < canvas.width
                        && (dy as u32) < canvas.height
                    {
                        let i = ((dy as u32 * canvas.width + dx as u32) * 4) as usize;
                        let a = p[3] as f64 / 255.0;

                        for c in 0..4 {
                            let dst = canvas.rgba[i + c] as f64;

                            canvas.rgba[i + c] =
                                (p[c] as f64 + dst * (1.0 - a)).round().min(255.0) as u8;
                        }
                    }
                }
            }
        }
    }
}

/// The browser UI state of a viewer.
#[derive(Default)]
pub struct BrowserUi {
    pub text: Text,
    pub notification: Notification,
    pub menu: Option<Menu>,
    pub world_info: Option<WorldInfoPanel>,
    /// The Browser Timings panel (`Browser/Core/BrowserTimings.js`).
    pub timings: crate::ui_timings::TimingsPanel,
    /// The entries of the host's user menu: (key, text).
    pub user_menu: Vec<(String, String)>,
    pub requests: Vec<UiRequest>,
    /// Bumped by every change of the menu or the dialog.
    pub revision: u64,
    /// What the last [`BrowserUi::overlay`] drew: (full, revision, text, width in pixels).
    drawn: Option<(bool, u64, String, i64)>,
    /// The buttons of the world that went down before the menu opened (their release still reaches it).
    pub world_buttons: [bool; 3],
}

/// The part of the window that the UI covers.
#[derive(Clone, Debug)]
pub struct Overlay {
    pub x: u32,
    pub y: u32,
    pub canvas: Canvas,
}

impl BrowserUi {
    pub fn is_open(&self) -> bool {
        self.menu.is_some() || self.world_info.is_some()
    }

    pub fn close_menu(&mut self) {
        if self.menu.take().is_some() {
            self.revision += 1;
        }
    }

    /// The next overlay for a window of `width` × `height`, or `None` when it is as the last one. `Some` with an empty
    /// canvas clears it.
    pub fn overlay(&mut self, width: u32, height: u32, now: f64) -> Option<Overlay> {
        let full = self.is_open() || self.timings.visible;
        let (pw, pill) = match self.notification.rect(&self.text, width as f64, now) {
            Some((x, _, w, _)) if !full => (w.ceil() as i64, Some(x)),
            Some((_, _, w, _)) => (w.ceil() as i64, None),
            None => (0, None),
        };
        let key = (
            full,
            self.revision.wrapping_add(self.timings.revision),
            if pw > 0 || full {
                self.notification.text.clone()
            } else {
                String::new()
            },
            pw,
        );

        if self.drawn.as_ref() == Some(&key) {
            return None;
        }

        let had = self.drawn.as_ref().is_some_and(|d| d.0 || d.3 > 0);

        self.drawn = Some(key);

        if full {
            let mut canvas = Canvas::new(width, height);

            self.timings.paint(&mut canvas, &mut self.text);

            if let Some(p) = &self.world_info {
                p.paint(&mut canvas, &mut self.text);
            }

            if let Some(m) = &self.menu {
                m.paint(&mut canvas, &mut self.text);
            }

            self.notification
                .paint(&mut canvas, &mut self.text, (0.0, 0.0), now);
            return Some(Overlay { x: 0, y: 0, canvas });
        }

        match (pill, pw) {
            (Some(x), w) if w > 0 => {
                let h = (self.text.line_height(FONT) + 10.0).ceil() as u32 + 1;
                let ox = (x.floor().max(0.0)) as u32;
                let mut canvas = Canvas::new(width - ox, h);

                self.notification
                    .paint(&mut canvas, &mut self.text, (ox as f64, 20.0), now);
                Some(Overlay {
                    x: ox,
                    y: 20,
                    canvas,
                })
            }
            _ if had => Some(Overlay {
                x: 0,
                y: 0,
                canvas: Canvas::new(1, 1),
            }),
            _ => None,
        }
    }
}
