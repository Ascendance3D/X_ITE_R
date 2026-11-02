//! The viewer's side of the native browser UI ([`crate::ui`]): the context menu's entries and actions
//! (`ContextMenu.build`), the Notification, and the browser's own keys of `X3DCoreContext [_keydown]`
//! (Ctrl+Plus, the Debug shading keys, Ctrl+S, Shift+F8).

use super::*;
use crate::ui::{
    Action, Item, Kind, Menu, MenuOutcome, OptionSet, Overlay, UiRequest, WorldInfoPanel,
};
use crate::ui_timings::{Counts, FrameTimes, Hit};
use x3d_runtime::browser_options::OptionValue;

/// `browser .getName ()` and `getProviderURL ()` of this port.
pub const BROWSER_NAME: &str = "X_ITE_R";
pub const PROVIDER_URL: &str = "https://github.com/Ascendance3D/X_ITE_R";

impl Viewer3d {
    // --- options and descriptions --------------------------------------------------------------

    fn option_bool(&self, name: &str) -> bool {
        self.rt
            .browser_options
            .get_option(name)
            .is_some_and(|v| v.as_bool())
    }

    fn option_str(&self, name: &str) -> String {
        self.rt
            .browser_options
            .get_option(name)
            .map(|v| v.as_str().to_string())
            .unwrap_or_default()
    }

    /// The Debug browser option.
    pub fn debug_option(&self) -> bool {
        self.option_bool("Debug")
    }

    /// `browser .setBrowserOption (name, value)`: the option is set and its interest runs with the events.
    pub fn set_browser_option(&mut self, name: &str, value: OptionValue) {
        self.rt.browser_options.set_option(name, value);
        self.rt.process_events();
        self.browser_requests();
        self.sync_timings();
    }

    /// `browser .setDescription (text)`: the status line and the Notification.
    pub fn set_description(&mut self, text: &str) {
        self.rt.description = text.to_string();
        self.status_description = text.to_string();
        self.status = text.to_string();
        self.notify(text);
    }

    /// Notification.set_string__: shown unless the Notifications option is off.
    pub(super) fn notify(&mut self, text: &str) {
        if self.native_ui && self.option_bool("Notifications") {
            self.ui.notification.set(text, self.date_now);
            self.ui.revision += 1;
        }
    }

    /// The window state changed (the Fullscreen entry's text).
    pub fn set_fullscreen(&mut self, on: bool) {
        self.fullscreen = on;
    }

    pub fn take_ui_requests(&mut self) -> Vec<UiRequest> {
        std::mem::take(&mut self.ui.requests)
    }

    /// BrowserTimings.set_enabled__: the panel follows the `Timings` browser option.
    pub(super) fn sync_timings(&mut self) {
        let on = self.native_ui && self.option_bool("Timings");

        self.ui.timings.set_enabled(on);
    }

    /// BrowserTimings.update, at each frame (`prepareEvents`).
    pub(super) fn timings_update(&mut self, now: f64) {
        self.sync_timings();
        self.ui.timings.update(now);
    }

    /// The times of the frame that the host just drew: the whole tick, `step` and the build plus the draw (ms).
    pub fn record_frame_times(&mut self, system_ms: f64, step_ms: f64, display_ms: f64) {
        self.ui.timings.record(FrameTimes {
            system: system_ms,
            step: step_ms,
            display: display_ms,
        });
    }

    /// Whether the Timings panel needs the counts of the frame.
    pub(super) fn timings_count(&mut self, frame: &x3d_render::traverse::Frame) {
        if !self.ui.timings.visible {
            return;
        }

        let mut c = Counts::default();

        fn walk(frame: &x3d_render::traverse::Frame, c: &mut Counts) {
            for item in &frame.items {
                // BrowserTimings.countPrimitives: the vertices of the geometry, times the instances.
                let n = item
                    .instances
                    .as_ref()
                    .map_or(1, |i| i.matrices.len().max(1)) as f64;
                let vertices = item.geometry.vertices.len() as f64 / 4.0 * n;

                match item.geometry.geometry_type {
                    0 => c.points += vertices,
                    1 => c.lines += vertices / 2.0,
                    2 | 3 => c.triangles += vertices / 3.0,
                    _ => {}
                }

                if item.transparent {
                    c.transparent += 1;
                } else {
                    c.opaque += 1;
                }
            }

            for layer in &frame.layers {
                walk(layer, c);
            }
        }

        walk(frame, &mut c);
        self.ui.timings.set_counts(c);
    }

    /// The overlay for the window, if it changed since the last call.
    pub fn overlay(&mut self) -> Option<Overlay> {
        self.sync_timings();
        self.ui.overlay(self.width, self.height, self.date_now)
    }

    /// The world went (or the viewer is closing): no menu or dialog stays.
    pub(super) fn close_ui(&mut self) {
        if self.ui.menu.is_some() || self.ui.world_info.is_some() {
            self.ui.menu = None;
            self.ui.world_info = None;
            self.ui.revision += 1;
        }
    }

    // --- the user menu ---------------------------------------------------------------------------

    /// `browser .getContextMenu () .userMenu`: entries (key, text) added after the first separator group; a click is
    /// reported by [`Viewer3d::take_user_menu_clicks`] (the keys, `user-` prefixed like `createUserMenu`).
    pub fn set_user_menu(&mut self, entries: Vec<(String, String)>) {
        self.ui.user_menu = entries;
    }

    pub fn take_user_menu_clicks(&mut self) -> Vec<String> {
        std::mem::take(&mut self.user_clicks)
    }

    // --- context menu ----------------------------------------------------------------------------

    pub fn context_menu(&self) -> Option<&Menu> {
        self.ui.menu.as_ref()
    }

    /// ContextMenu.hide ().
    pub fn hide_context_menu(&mut self) {
        self.ui.close_menu();
    }

    /// ContextMenu.show (event): at the position of a `contextmenu` event, bottom-left origin like the
    /// other mouse events. Nothing opens when the ContextMenu option is off.
    pub fn show_context_menu(&mut self, x: f64, y: f64) {
        self.ui.close_menu();

        let Some(items) = self.build_menu() else {
            return;
        };
        let canvas = (self.width as f64, self.height as f64);
        let menu = Menu::new(items, (x, canvas.1 - y), canvas, &self.ui.text);

        self.ui.menu = Some(menu);
        self.ui.revision += 1;
    }

    fn viewer_name(viewer: &str) -> &'static str {
        match viewer {
            "EXAMINE" => "Examine Viewer",
            "WALK" => "Walk Viewer",
            "FLY" => "Fly Viewer",
            "PLANE" => "Plane Viewer",
            "LOOKAT" => "Look At Viewer",
            _ => "None Viewer",
        }
    }

    fn quality_items(
        &self,
        label: &'static str,
        name: &'static str,
        group: &'static str,
    ) -> Vec<Item> {
        let current = self.option_str(name);

        [("High", "HIGH"), ("Medium", "MEDIUM"), ("Low", "LOW")]
            .into_iter()
            .map(|(text, value)| {
                Item::radio(
                    text,
                    group,
                    current == value,
                    Action::SetOption {
                        name,
                        value: OptionSet::Str(value),
                        description: format!("{label}: {}", text.to_lowercase()),
                    },
                )
            })
            .collect()
    }

    /// The TimeSensors of the exported `Animations` node: `children [0]` of each child group.
    fn animations(&self) -> Vec<NodeId> {
        let world = self.rt.scene.world;
        let Some(animations) = self.rt.scene.get_exported_node(world, "Animations") else {
            return vec![];
        };
        let Some(FieldValue::MFNode(groups)) = self.rt.pre_value(animations, "children") else {
            return vec![];
        };

        groups
            .iter()
            .flatten()
            .filter_map(|g| match self.rt.pre_value(*g, "children") {
                Some(FieldValue::MFNode(c)) => c.first().copied().flatten(),
                _ => None,
            })
            .filter(|n| self.rt.builtin_name(*n) == Some("TimeSensor"))
            .collect()
    }

    /// `ContextMenu.build ()`, without the entries it deletes. `None`: the ContextMenu option is off.
    pub fn build_menu(&self) -> Option<Vec<Item>> {
        if !self.option_bool("ContextMenu") {
            return None;
        }

        let viewer = self.viewer_field().to_string();
        let mut items = vec![
            Item {
                text: format!("{BROWSER_NAME} v{}", env!("CARGO_PKG_VERSION")),
                kind: Kind::Title,
                action: None,
            },
            Item::separator(),
        ];

        // Viewpoints.
        let bound = self.layer.viewpoint();
        let viewpoints: Vec<Item> = self
            .user_viewpoints()
            .into_iter()
            .map(|n| {
                Item::radio(
                    &self.viewpoint_description(n),
                    "viewpoints",
                    n == bound,
                    Action::Viewpoint(n),
                )
            })
            .collect();

        if !viewpoints.is_empty() {
            items.push(Item::submenu("Viewpoints", viewpoints));
        }

        // Available Viewers.
        let viewers: Vec<Item> = self
            .available_viewers()
            .into_iter()
            .map(|v| {
                Item::radio(
                    Viewer3d::viewer_name(&v),
                    "viewers",
                    v == viewer,
                    Action::Viewer(v),
                )
            })
            .collect();

        if viewers.len() >= 2 {
            items.push(Item::submenu("Available Viewers", viewers));
        }

        if matches!(viewer.as_str(), "EXAMINE" | "FLY") {
            items.push(Item::check(
                "Straighten Horizon",
                self.option_bool("StraightenHorizon"),
                Action::Toggle {
                    name: "StraightenHorizon",
                    label: Some("Straighten Horizon"),
                },
            ));
        }

        if matches!(viewer.as_str(), "WALK" | "FLY") {
            items.push(Item::check(
                "Display Rubberband",
                self.option_bool("Rubberband"),
                Action::Toggle {
                    name: "Rubberband",
                    label: Some("Rubberband"),
                },
            ));
        }

        items.push(Item::separator());
        items.push(Item::submenu(
            "Primitive Quality",
            self.quality_items("Primitive Quality", "PrimitiveQuality", "primitive-quality"),
        ));
        items.push(Item::submenu(
            "Texture Quality",
            self.quality_items("Texture Quality", "TextureQuality", "texture-quality"),
        ));

        if self.debug_option() {
            let current = self.option_str("Shading");

            items.push(Item::submenu(
                "Shading",
                [
                    ("Points", "POINT"),
                    ("Wireframe", "WIREFRAME"),
                    ("Flat", "FLAT"),
                    ("Gouraud", "GOURAUD"),
                    ("Phong", "PHONG"),
                ]
                .into_iter()
                .map(|(text, value)| {
                    Item::radio(
                        text,
                        "shading",
                        current == value,
                        Action::SetOption {
                            name: "Shading",
                            value: OptionSet::Str(value),
                            description: format!("Shading: {text}"),
                        },
                    )
                })
                .collect(),
            ));
        }

        items.push(Item::separator());

        // `createUserMenu`: the host's entries, keyed `user-<key>`.
        for (i, (_, text)) in self.ui.user_menu.iter().enumerate() {
            items.push(Item::plain(text, Action::User(i as u32)));
        }

        items.push(Item::separator());

        // Animations.
        let sensors = self.animations();

        if !sensors.is_empty() {
            let mut list: Vec<Item> = sensors
                .iter()
                .map(|n| {
                    let text = match self.rt.pre_value(*n, "description") {
                        Some(FieldValue::SFString(s)) => s.clone(),
                        _ => String::new(),
                    };

                    Item::check(
                        &text,
                        self.rt.get_bool(*n, "isActive"),
                        Action::Animation(*n),
                    )
                })
                .collect();

            list.push(Item::separator());
            list.push(Item::plain("All", Action::AnimationsAll));
            list.push(Item::plain("None", Action::AnimationsNone));
            items.push(Item::submenu("Animations", list));
        }

        items.push(Item::check(
            "Browser Timings",
            self.option_bool("Timings"),
            Action::Toggle {
                name: "Timings",
                label: None,
            },
        ));
        items.push(Item::plain(
            if self.fullscreen {
                "Leave Fullscreen"
            } else {
                "Fullscreen"
            },
            Action::Fullscreen,
        ));
        items.push(Item::separator());

        let (info, meta) = self.world_info();

        if info
            .as_ref()
            .is_some_and(|(t, i)| !t.is_empty() || !i.is_empty())
            || !meta.is_empty()
        {
            items.push(Item::plain("Show World Info", Action::WorldInfo));
        }

        items.push(Item::plain(&format!("About {BROWSER_NAME}"), Action::About));
        Some(items)
    }

    /// The first WorldInfo of the world's own scene (`getWorldInfos () [0]`): title and info lines, then the META
    /// statements of the scene.
    pub fn world_info(&self) -> (Option<(String, Vec<String>)>, Vec<(String, String)>) {
        let world = self.rt.scene.world;
        let info = self
            .rt
            .world_infos
            .get(&world)
            .and_then(|list| list.first().copied())
            .map(|n| {
                let title = match self.rt.pre_value(n, "title") {
                    Some(FieldValue::SFString(t)) => t.clone(),
                    _ => String::new(),
                };
                let lines = match self.rt.pre_value(n, "info") {
                    Some(FieldValue::MFString(l)) => l.clone(),
                    _ => vec![],
                };

                (title, lines)
            });

        (info, self.rt.scene.meta_of(world).to_vec())
    }

    // --- mouse routing -----------------------------------------------------------------------------

    fn ui_point(&self, x: f64, y: f64) -> (f64, f64) {
        (x, self.height as f64 - y)
    }

    fn timings_hit(&self, p: (f64, f64)) -> Option<Hit> {
        self.ui
            .timings
            .takes_pointer(&self.ui.text, self.height as f64, p.0, p.1)
    }

    /// A `mousedown` of the surface. Returns whether the UI took it: the layer of an open menu or dialog is over the
    /// surface; a right press goes to the viewer and then opens the menu (the `contextmenu` event follows it).
    pub(super) fn ui_mouse_down(&mut self, button: u8, x: f64, y: f64) -> bool {
        if !self.native_ui {
            return false;
        }

        let p = self.ui_point(x, y);

        if let Some(menu) = &mut self.ui.menu {
            match menu.down(button, p.0, p.1) {
                MenuOutcome::Closed => {
                    self.ui.close_menu();

                    // The layer's `mousedown` hides the menu; the `contextmenu` event that a right press brings
                    // bubbles on to the element, where ContextMenu.show opens a new menu at the new point.
                    if button == 2 {
                        self.show_context_menu(x, y);
                    }
                }
                _ => self.ui.revision += 1,
            }

            return true;
        }

        // The dialog's overlay is over the surface too; its `contextmenu` event reaches the element as well.
        if self.ui.world_info.is_some() {
            if button == 2 {
                self.show_context_menu(x, y);
            }

            return true;
        }

        // The Timings panel is over the surface: its button and its two titled cells take the pointer (the rest of the
        // panel has `pointer-events: none`); a `contextmenu` event on any part of it bubbles to the surface.
        let hit = self.timings_hit(p);

        if let Some(hit) = hit {
            if button == 0 && hit == Hit::Button {
                self.ui.timings.pressed = true;
            }

            if button == 2 && self.option_bool("ContextMenu") {
                self.show_context_menu(x, y);
            }

            return true;
        }

        if button < 3 {
            self.ui.world_buttons[button as usize] = true;
        }

        if button == 2 && self.option_bool("ContextMenu") {
            self.mouse_down(button, x, y);
            self.show_context_menu(x, y);
            return true;
        }

        false
    }

    pub(super) fn ui_mouse_up(&mut self, button: u8, x: f64, y: f64) -> bool {
        if !self.native_ui {
            return false;
        }

        let p = self.ui_point(x, y);
        let held = button < 3 && std::mem::take(&mut self.ui.world_buttons[button as usize]);

        // The button's `click`: the press and the release are on the button.
        if button == 0 && std::mem::take(&mut self.ui.timings.pressed) {
            if !self.ui.is_open() && self.timings_hit(p) == Some(Hit::Button) {
                self.ui.timings.set_type();
            }

            return true;
        }

        if !self.ui.is_open() {
            return false;
        }

        // The surface's document listeners still see the release of a button that went down before the layer.
        if held {
            self.mouse_up(button, x, y);
        }

        let outcome = match &mut self.ui.menu {
            Some(menu) => menu.up(button, p.0, p.1),
            None => {
                // The dialog.
                if let Some(panel) = &mut self.ui.world_info {
                    if button == 0
                        && !panel.click((self.width as f64, self.height as f64), p.0, p.1)
                    {
                        self.ui.world_info = None;
                    }
                }

                self.ui.revision += 1;
                return true;
            }
        };

        self.ui.revision += 1;

        if let MenuOutcome::Action { action, close } = outcome {
            self.apply_action(action);

            if close {
                self.ui.close_menu();
            }
        }

        true
    }

    pub(super) fn ui_mouse_move(&mut self, x: f64, y: f64) -> bool {
        if !self.native_ui {
            return false;
        }

        let p = self.ui_point(x, y);

        if !self.ui.is_open() {
            // The panel's button and titled cells are elements over the canvas: the world sees no move over them.
            return self.timings_hit(p).is_some();
        }

        if let Some(menu) = &mut self.ui.menu {
            if menu.pointer(p.0, p.1) {
                self.ui.revision += 1;
            }
        }

        // The document's mousemove listeners of a drag that began before the layer.
        if self.ui.world_buttons.iter().any(|b| *b) {
            self.mouse_move(x, y);
        }

        true
    }

    /// The callback of a clicked entry.
    pub fn apply_action(&mut self, action: Action) {
        match action {
            Action::SetOption {
                name,
                value,
                description,
            } => {
                let v = match value {
                    OptionSet::Bool(b) => OptionValue::Bool(b),
                    OptionSet::Str(s) => OptionValue::String(s.to_string()),
                };

                self.set_browser_option(name, v);
                self.set_description(&description);
            }
            Action::Toggle { name, label } => {
                let on = !self.option_bool(name);

                self.set_browser_option(name, OptionValue::Bool(on));

                if let Some(label) = label {
                    self.set_description(&format!("{label}: {}", if on { "on" } else { "off" }));
                }
            }
            Action::Viewpoint(n) => self.bind_viewpoint(n),
            Action::Viewer(v) => {
                self.set_viewer(&v);
                self.set_description(Viewer3d::viewer_name(&v));
            }
            Action::Fullscreen => self.ui.requests.push(UiRequest::ToggleFullscreen),
            Action::About => self
                .ui
                .requests
                .push(UiRequest::OpenUrl(PROVIDER_URL.into())),
            Action::WorldInfo => {
                let (info, meta) = self.world_info();
                let (title, lines) = info.unwrap_or_default();

                self.ui.world_info = Some(WorldInfoPanel::new(title, lines, meta));
                self.ui.close_menu();
            }
            Action::Animation(n) => {
                let now = self.date_now;

                if self.rt.get_bool(n, "isActive") {
                    self.set_time_field(n, "stopTime", now);
                } else {
                    self.set_bool_field(n, "loop", true);
                    self.set_time_field(n, "startTime", now);
                }
            }
            Action::AnimationsAll => {
                let now = self.date_now;

                for n in self.animations() {
                    self.set_bool_field(n, "loop", true);
                    self.set_time_field(n, "startTime", now);
                }
            }
            Action::AnimationsNone => {
                let now = self.date_now;

                for n in self.animations() {
                    self.set_time_field(n, "stopTime", now);
                }
            }
            Action::User(i) => {
                if let Some((key, _)) = self.ui.user_menu.get(i as usize) {
                    self.user_clicks.push(format!("user-{key}"));
                }
            }
        }
    }

    fn set_time_field(&mut self, n: NodeId, name: &str, v: f64) {
        if let Some(k) = self.rt.pre(n, name) {
            self.rt.set_value(k, FieldValue::SFTime(v));
            self.rt.process_events();
        }
    }

    fn set_bool_field(&mut self, n: NodeId, name: &str, v: bool) {
        if let Some(k) = self.rt.pre(n, name) {
            self.rt.set_value(k, FieldValue::SFBool(v));
        }
    }

    // --- the browser's own keys ----------------------------------------------------------------------

    /// X3DCoreContext `[_keydown]` for the keys after the modifiers: `keyCode`.
    pub(super) fn browser_keydown(&mut self, code: u32) {
        if !self.native_ui {
            return;
        }

        let debug = self.debug_option();

        match code {
            49..=53 if debug && self.ctrl => {
                let (value, text) = [
                    ("POINT", "Shading: Pointset"),
                    ("WIREFRAME", "Shading: Wireframe"),
                    ("FLAT", "Shading: Flat"),
                    ("GOURAUD", "Shading: Gouraud"),
                    ("PHONG", "Shading: Phong"),
                ][(code - 49) as usize];

                self.set_browser_option("Shading", OptionValue::String(value.into()));
                self.set_description(text);
            }
            83 if debug && self.ctrl => {
                self.live = !self.live;
                self.set_description(if self.live {
                    "Begin Update"
                } else {
                    "End Update"
                });
            }
            171 | 187 if self.ctrl => {
                let on = !self.option_bool("Timings");

                self.set_browser_option("Timings", OptionValue::Bool(on));
            }
            119 if self.shift => {
                if let Some(text) = self.viewpoint_text() {
                    self.ui.requests.push(UiRequest::Clipboard(text));
                }
            }
            _ => {}
        }
    }

    /// The window put the Shift+F8 text on the clipboard.
    pub fn clipboard_done(&mut self) {
        self.set_description("Viewpoint copied to clipboard.");
    }

    /// The Shift+F8 text: a Viewpoint or OrthoViewpoint with the user's position, orientation, center of rotation and
    /// field of view, in the encoding of the world's file, and a line feed. `None` for a GeoViewpoint.
    pub fn viewpoint_text(&mut self) -> Option<String> {
        let active = self.layer.viewpoint();
        let type_name = if active == crate::layer::DEFAULT_ORTHO {
            "OrthoViewpoint"
        } else if is_default(active) {
            "Viewpoint"
        } else {
            self.rt.builtin_name(active)?
        };

        if !matches!(type_name, "Viewpoint" | "OrthoViewpoint") {
            return None;
        }

        let fov = (!is_default(active))
            .then(|| self.rt.pre_value(active, "fieldOfView").cloned())
            .flatten();
        let (p, o, c) = self.user_pose();
        let ctx = self.rt.scene.world;
        let node = self.rt.create_node_setup(ctx, type_name)?;
        let set = |rt: &mut Runtime, name: &str, v: FieldValue| {
            if let Some(k) = rt.pre(node, name) {
                rt.set_raw(k, v);
            }
        };

        set(&mut self.rt, "position", FieldValue::SFVec3f(p));
        set(&mut self.rt, "orientation", FieldValue::SFRotation(o));
        set(&mut self.rt, "centerOfRotation", FieldValue::SFVec3f(c));

        if let Some(v) = fov {
            set(&mut self.rt, "fieldOfView", v);
        }

        let url = self.rt.scene.world_url_of(ctx).to_lowercase();
        let url = url.split(['?', '#']).next().unwrap_or("").to_string();
        let mut text = if url.ends_with(".x3dj") || url.ends_with(".json") {
            self.rt
                .scene
                .field_value_to_json_string(&FieldValue::SFNode(Some(node)))
        } else if url.ends_with(".wrl")
            || url.ends_with(".wrz")
            || url.ends_with(".x3dv")
            || url.ends_with(".x3dvz")
        {
            self.rt.scene.node_to_vrml_string(node)
        } else {
            self.rt
                .scene
                .field_value_to_xml_string(&FieldValue::SFNode(Some(node)))
        };

        self.rt.scene.release_node(node);
        text.push('\n');
        Some(text)
    }
}
