//! Browser/Core/BrowserOptions.js, BrowserProperties.js and RenderingProperties.js: the values
//! `Browser.getBrowserOption`, `setBrowserOption`, `getBrowserProperty` and
//! `getRenderingProperty` read and write.
//!
//! Like X_ITE, a set writes the option at once, and its interest (set_Shading__,
//! set_Multisampling__ …) runs when the browser processes the tainted field, in
//! [`crate::Runtime::process_events`]. The interests that change the frame buffer end with
//! `browser .reshape ()`, which sets the ContentScale rendering property from the canvas size.

/// The value of one option or property field.
#[derive(Clone, Debug, PartialEq)]
pub enum OptionValue {
    Bool(bool),
    Int32(i32),
    Float(f64),
    Double(f64),
    String(String),
}

impl OptionValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            OptionValue::Bool(_) => "SFBool",
            OptionValue::Int32(_) => "SFInt32",
            OptionValue::Float(_) => "SFFloat",
            OptionValue::Double(_) => "SFDouble",
            OptionValue::String(_) => "SFString",
        }
    }

    pub fn as_bool(&self) -> bool {
        matches!(self, OptionValue::Bool(true))
    }

    pub fn as_f64(&self) -> f64 {
        match self {
            OptionValue::Int32(v) => *v as f64,
            OptionValue::Float(v) | OptionValue::Double(v) => *v,
            OptionValue::Bool(b) => *b as i32 as f64,
            OptionValue::String(_) => f64::NAN,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            OptionValue::String(s) => s,
            _ => "",
        }
    }
}

/// A field list with X_ITE's `addAlias ("AntiAliased", this ._Antialiased)`. Names are case
/// sensitive (X3DBaseNode.getField).
#[derive(Clone, Debug)]
pub struct Fields {
    fields: Vec<(&'static str, OptionValue)>,
}

impl Fields {
    fn index(&self, name: &str) -> Option<usize> {
        let name = if name == "AntiAliased" { "Antialiased" } else { name };

        self.fields.iter().position(|(n, _)| *n == name)
    }

    pub fn get(&self, name: &str) -> Option<&OptionValue> {
        self.index(name).map(|i| &self.fields[i].1)
    }

    fn get_mut(&mut self, name: &str) -> &mut OptionValue {
        let i = self.index(name).expect("field");
        &mut self.fields[i].1
    }

    fn set(&mut self, name: &str, value: OptionValue) {
        *self.get_mut(name) = value;
    }

    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.fields.iter().map(|(n, _)| *n)
    }

    /// Name and value of each field, in X_ITE's order.
    pub fn entries(&self) -> impl Iterator<Item = (&'static str, &OptionValue)> + '_ {
        self.fields.iter().map(|(n, v)| (*n, v))
    }
}

/// The fields of BrowserOptions ("options"), BrowserProperties ("properties") or RenderingProperties (any other name) with
/// X_ITE's initial values: the values of their field definitions.
pub fn default_fields(set: &str) -> Fields {
    match set {
        "options" => option_fields(),
        "properties" => property_fields(),
        _ => rendering_fields(),
    }
}

/// Shading (Browser/Core/Shading.js), by key: set_Shading__ maps the option string with
/// `Shading .get (value .replace ("POINTSET", "POINT")) ?? Shading .GOURAUD`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShadingMode {
    Point,
    Wireframe,
    Flat,
    Gouraud,
    Phong,
}

impl ShadingMode {
    fn from_option(value: &str) -> ShadingMode {
        match value.replacen("POINTSET", "POINT", 1).as_str() {
            "POINT" => ShadingMode::Point,
            "WIREFRAME" => ShadingMode::Wireframe,
            "FLAT" => ShadingMode::Flat,
            "PHONG" => ShadingMode::Phong,
            "GOURAUD" => ShadingMode::Gouraud,
            _ => ShadingMode::Gouraud,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ShadingMode::Point => "POINT",
            ShadingMode::Wireframe => "WIREFRAME",
            ShadingMode::Flat => "FLAT",
            ShadingMode::Gouraud => "GOURAUD",
            ShadingMode::Phong => "PHONG",
        }
    }
}

/// The device values RenderingProperties.initialize reads (getMaxTextureSize …). The defaults are
/// the values the Rust renderer is built to behave like: the X_ITE oracle's (headless Chrome,
/// ANGLE on OpenGL 4.5, NVIDIA): MAX_TEXTURE_SIZE 32768, MAX_COMBINED_TEXTURE_IMAGE_UNITS 64,
/// 8 lights, 8 bit RGBA, max anisotropy 16, MAX_SAMPLES 8, MAX_VIEWPORT_DIMS 32768, and a 10in
/// element of 960 px.
#[derive(Clone, Debug)]
pub struct RenderingLimits {
    pub max_texture_size: i32,
    pub texture_units: i32,
    pub max_lights: i32,
    pub color_depth: i32,
    pub max_anisotropic_degree: f64,
    pub max_samples: i32,
    pub max_viewport_dims: [f64; 2],
    pub pixels_per_point: f64,
    /// window.devicePixelRatio, for ContentScale -1.
    pub device_pixel_ratio: f64,
}

impl Default for RenderingLimits {
    fn default() -> Self {
        RenderingLimits {
            max_texture_size: 32768,
            texture_units: 64,
            max_lights: 8,
            color_depth: 32,
            max_anisotropic_degree: 16.0,
            max_samples: 8,
            max_viewport_dims: [32768.0, 32768.0],
            pixels_per_point: 960.0 / 720.0,
            device_pixel_ratio: 1.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BrowserOptions {
    pub options: Fields,
    pub properties: Fields,
    pub rendering: Fields,
    pub limits: RenderingLimits,
    /// The canvas size in CSS pixels (the parent element's offsetWidth and offsetHeight).
    pub canvas: (f64, f64),
    /// The frame buffer size reshape () computed (canvas.width, canvas.height).
    pub framebuffer: (u32, u32),
    /// BrowserOptions.shading (getShading ()).
    pub shading: ShadingMode,
    /// Options set since the last [`BrowserOptions::process_events`], in order, without repeats
    /// (a tainted field is added once).
    tainted: Vec<&'static str>,
    /// Mute or SoundIntensity was processed (the runtime runs set_SoundIntensity__).
    sound_intensity_changed: bool,
}

fn option_fields() -> Fields {
    use OptionValue::*;

    let s = |v: &str| String(v.to_string());

    Fields {
        fields: vec![
            ("SplashScreen", Bool(true)),
            ("Dashboard", Bool(true)),
            ("Rubberband", Bool(true)),
            ("EnableInlineViewpoints", Bool(true)),
            ("Antialiased", Bool(true)),
            ("TextureQuality", s("MEDIUM")),
            ("PrimitiveQuality", s("MEDIUM")),
            ("QualityWhenMoving", s("SAME")),
            ("Shading", s("GOURAUD")),
            ("MotionBlur", Bool(false)),
            ("AlwaysUpdateGeometries", Bool(false)),
            ("AutoUpdate", Bool(false)),
            ("Cache", Bool(true)),
            ("ColorSpace", s("LINEAR_WHEN_PHYSICAL_MATERIAL")),
            ("ContentScale", Double(1.0)),
            ("ContextMenu", Bool(true)),
            ("Debug", Bool(false)),
            ("Exposure", Double(1.0)),
            ("DisplayColorSpace", s("SRGB")),
            ("Gravity", Double(9.80665)),
            ("LoadUrlObjects", Bool(true)),
            ("LogarithmicDepthBuffer", Bool(false)),
            ("MaximumFrameRate", Double(80.0)),
            ("MetadataReference", s("")),
            ("Multisampling", Int32(4)),
            ("Mute", Bool(false)),
            ("Notifications", Bool(true)),
            ("OrderIndependentTransparency", Bool(false)),
            ("SoundIntensity", Float(1.0)),
            ("StraightenHorizon", Bool(true)),
            ("TextCompression", s("CHAR_SPACING")),
            ("Timings", Bool(false)),
            ("ToneMapping", s("NONE")),
            ("WallFriction", Double(0.0)),
            ("XRSessionMode", s("IMMERSIVE_VR")),
        ],
    }
}

fn property_fields() -> Fields {
    use OptionValue::Bool;

    Fields {
        fields: vec![
            ("ABSTRACT_NODES", Bool(true)),
            ("CONCRETE_NODES", Bool(true)),
            ("EXTERNAL_INTERACTIONS", Bool(true)),
            ("PROTOTYPE_CREATE", Bool(true)),
            ("DOM_IMPORT", Bool(true)),
            ("XML_ENCODING", Bool(true)),
            ("CLASSIC_VRML_ENCODING", Bool(true)),
            ("BINARY_ENCODING", Bool(false)),
        ],
    }
}

fn rendering_fields() -> Fields {
    use OptionValue::*;

    Fields {
        fields: vec![
            ("Shading", String(std::string::String::new())),
            ("MaxTextureSize", Int32(0)),
            ("TextureUnits", Int32(0)),
            ("MaxLights", Int32(0)),
            ("Antialiased", Bool(true)),
            ("ColorDepth", Int32(0)),
            ("TextureMemory", Double(0.0)),
            ("ContentScale", Double(1.0)),
            ("LogarithmicDepthBuffer", Bool(false)),
            ("MaxAnisotropicDegree", Float(1.0)),
            ("MaxSamples", Int32(0)),
            ("Multisampling", Int32(4)),
            ("PixelsPerPoint", Double(1.0)),
            ("SoundDestinations", Int32(0)),
            ("XRSession", Bool(false)),
        ],
    }
}

/// How an X3DCoreContext.attributeChangedCallback attribute sets its option.
enum Attribute {
    /// `parseBooleanAttribute (newValue) ?? default`: exactly "true" or "false".
    Bool(bool),
    /// `newValue || default`.
    String(&'static str),
    /// The attribute string, converted by the field (`+value`, `value | 0`); ContentScale maps
    /// "auto" to -1.
    Number,
}

/// The attributes that set an option (lower case, as the callback compares them).
const ATTRIBUTES: &[(&str, &str, Attribute)] = &[
    ("antialiased", "Antialiased", Attribute::Bool(true)),
    ("cache", "Cache", Attribute::Bool(true)),
    ("colorspace", "ColorSpace", Attribute::String("LINEAR_WHEN_PHYSICAL_MATERIAL")),
    ("contentscale", "ContentScale", Attribute::Number),
    ("contextmenu", "ContextMenu", Attribute::Bool(true)),
    ("debug", "Debug", Attribute::Bool(false)),
    ("displaycolorspace", "DisplayColorSpace", Attribute::String("SRGB")),
    ("exposure", "Exposure", Attribute::Number),
    ("logarithmicdepthbuffer", "LogarithmicDepthBuffer", Attribute::Bool(false)),
    ("maximumframerate", "MaximumFrameRate", Attribute::Number),
    ("multisampling", "Multisampling", Attribute::Number),
    ("notifications", "Notifications", Attribute::Bool(true)),
    ("orderindependenttransparency", "OrderIndependentTransparency", Attribute::Bool(false)),
    ("splashscreen", "SplashScreen", Attribute::Bool(true)),
    ("textcompression", "TextCompression", Attribute::String("CHAR_SPACING")),
    ("timings", "Timings", Attribute::Bool(false)),
    ("tonemapping", "ToneMapping", Attribute::String("NONE")),
    ("xrsessionmode", "XRSessionMode", Attribute::String("IMMERSIVE_VR")),
];

/// ECMAScript `Number (string)` for decimal strings: white space is trimmed, "" is 0.
fn js_number(s: &str) -> f64 {
    let t = s.trim();

    match t {
        "" => 0.0,
        "Infinity" | "+Infinity" => f64::INFINITY,
        "-Infinity" => f64::NEG_INFINITY,
        _ if t.chars().all(|c| c.is_ascii_digit() || "+-.eE".contains(c)) => t.parse().unwrap_or(f64::NAN),
        _ => f64::NAN,
    }
}

impl BrowserOptions {
    /// The options after BrowserOptions.initialize and RenderingProperties.initialize on a
    /// canvas of `width` × `height` CSS pixels with no attributes.
    pub fn new(width: u32, height: u32) -> BrowserOptions {
        let mut b = BrowserOptions {
            options: option_fields(),
            properties: property_fields(),
            rendering: rendering_fields(),
            limits: RenderingLimits::default(),
            canvas: (width as f64, height as f64),
            framebuffer: (width.max(1), height.max(1)),
            shading: ShadingMode::Gouraud,
            tainted: vec![],
            sound_intensity_changed: false,
        };

        b.set_limits(RenderingLimits::default());

        // BrowserOptions.initialize: the interests run once with the default values.
        for name in ["Shading", "ContentScale", "LogarithmicDepthBuffer", "Multisampling"] {
            b.interest(name);
        }

        b
    }

    /// RenderingProperties.initialize: the device values.
    pub fn set_limits(&mut self, limits: RenderingLimits) {
        use OptionValue::*;

        let r = &mut self.rendering;
        r.set("MaxTextureSize", Int32(limits.max_texture_size));
        r.set("TextureUnits", Int32(limits.texture_units));
        r.set("MaxLights", Int32(limits.max_lights));
        r.set("ColorDepth", Int32(limits.color_depth));
        // X3DTexturingContext.getTextureMemory () is NaN.
        r.set("TextureMemory", Double(f64::NAN));
        r.set("MaxAnisotropicDegree", Float(limits.max_anisotropic_degree));
        r.set("MaxSamples", Int32(limits.max_samples));
        r.set("PixelsPerPoint", Double(limits.pixels_per_point));
        self.limits = limits;
    }

    /// The page attributes of the embedding element (X3DCoreContext attributeChangedCallback):
    /// `(attribute, value)` pairs, like `("antialiased", "false")`. Unknown attributes and the
    /// ones that do not set an option are ignored. They are applied at once with their
    /// interests, like BrowserOptions.reset before the world loads.
    pub fn apply_attributes(&mut self, attributes: &[(&str, &str)]) {
        for (attribute, value) in attributes {
            let lower = attribute.to_lowercase();
            let Some((_, name, kind)) = ATTRIBUTES.iter().find(|(a, _, _)| *a == lower) else { continue };

            let v = match kind {
                Attribute::Bool(default) => OptionValue::Bool(match *value {
                    "true" => true,
                    "false" => false,
                    _ => *default,
                }),
                Attribute::String(default) => OptionValue::String(if value.is_empty() { default.to_string() } else { value.to_string() }),
                Attribute::Number => {
                    let n = if *name == "ContentScale" && *value == "auto" { -1.0 } else { js_number(value) };

                    match self.option(name) {
                        OptionValue::Int32(_) => OptionValue::Int32(js_int32(n)),
                        OptionValue::Float(_) => OptionValue::Float(n),
                        _ => OptionValue::Double(n),
                    }
                }
            };

            self.options.set(name, v);
            self.interest(name);
        }
    }

    /// The canvas was resized: X3DRenderingContext.reshape.
    pub fn set_canvas_size(&mut self, width: u32, height: u32) {
        self.canvas = (width as f64, height as f64);
        self.reshape();
    }

    pub fn get_option(&self, name: &str) -> Option<&OptionValue> {
        self.options.get(name)
    }

    /// `setBrowserOption (name, value)` with the value already converted to the field type
    /// ([`OptionValue::type_name`] of [`BrowserOptions::get_option`]). The interest is deferred to
    /// [`BrowserOptions::process_events`].
    pub fn set_option(&mut self, name: &str, value: OptionValue) -> bool {
        let Some(i) = self.options.index(name) else { return false };
        let (field, _) = self.options.fields[i];

        self.options.fields[i].1 = value;

        if !self.tainted.contains(&field) {
            self.tainted.push(field);
        }

        true
    }

    /// A RenderingProperties value the browser sets (`getRenderingProperties () ._Name = value`).
    pub fn set_rendering_value(&mut self, name: &str, value: OptionValue) {
        if self.rendering.index(name).is_some() {
            self.rendering.set(name, value);
        }
    }

    /// Mute or SoundIntensity changed since the last call (set_SoundIntensity__ is due).
    pub fn take_sound_intensity_changed(&mut self) -> bool {
        std::mem::take(&mut self.sound_intensity_changed)
    }

    pub fn has_pending_events(&self) -> bool {
        !self.tainted.is_empty()
    }

    /// The browser processes the tainted option fields: their interests, in order.
    pub fn process_events(&mut self) {
        for name in std::mem::take(&mut self.tainted) {
            self.interest(name);
        }
    }

    fn option(&self, name: &str) -> &OptionValue {
        self.options.get(name).expect("option")
    }

    fn interest(&mut self, name: &str) {
        match name {
            "Antialiased" | "Multisampling" => self.set_multisampling(),
            "Shading" => {
                // set_Shading__
                self.shading = ShadingMode::from_option(self.option("Shading").as_str());
                self.rendering.set("Shading", OptionValue::String(self.shading.name().to_string()));
            }
            "ContentScale" => {
                // set_ContentScale__: -1 follows devicePixelRatio (updateContentScale).
                let content_scale = self.option("ContentScale").as_f64();

                if content_scale != -1.0 {
                    let v = content_scale.max(0.0);
                    self.rendering.set("ContentScale", OptionValue::Double(if v == 0.0 || v.is_nan() { 1.0 } else { v }));
                }

                self.reshape();
            }
            "LogarithmicDepthBuffer" => {
                let v = self.option("LogarithmicDepthBuffer").as_bool();
                self.rendering.set("LogarithmicDepthBuffer", OptionValue::Bool(v));
            }
            "OrderIndependentTransparency" => self.reshape(),
            "Mute" | "SoundIntensity" => self.sound_intensity_changed = true,
            _ => {}
        }
    }

    /// set_Multisampling__.
    fn set_multisampling(&mut self) {
        let max_samples = self.limits.max_samples;
        let multisampling = self.option("Multisampling").as_f64();
        let samples = (if multisampling.is_nan() { 4 } else { multisampling as i32 }).clamp(0, max_samples);
        let antialiased = self.option("Antialiased").as_bool();

        self.rendering.set("Multisampling", OptionValue::Int32(if antialiased { samples } else { 0 }));
        self.rendering.set("Antialiased", OptionValue::Bool(samples > 0));
        self.reshape();
    }

    /// X3DRenderingContext.reshape and limitFramebufferSize.
    fn reshape(&mut self) {
        let (css_width, css_height) = self.canvas;
        let mut content_scale = self.option("ContentScale").as_f64();

        if content_scale == -1.0 {
            content_scale = self.limits.device_pixel_ratio;
        }

        let (mut width, mut height) = (css_width * content_scale, css_height * content_scale);
        let [max_width, max_height] = self.limits.max_viewport_dims;

        if width > max_width || height > max_height {
            if width > height {
                height = max_height * height / width;
                width = max_width;
            } else {
                width = max_width * width / height;
                height = max_height;
            }
        }

        let width = js_int32(width).max(1);
        let height = js_int32(height).max(1);

        let scale = if css_width != 0.0 { width as f64 / css_width } else { content_scale };

        self.rendering.set("ContentScale", OptionValue::Double(scale));
        self.framebuffer = (width as u32, height as u32);
    }

    /// RenderingProperties Multisampling: the sample count the frame buffer uses.
    pub fn multisampling(&self) -> u32 {
        self.rendering.get("Multisampling").map_or(0, |v| v.as_f64() as u32)
    }

    pub fn logarithmic_depth_buffer(&self) -> bool {
        self.rendering.get("LogarithmicDepthBuffer").is_some_and(OptionValue::as_bool)
    }

    /// `getBrowserOptions () ._Gravity .getValue ()` (X3DRenderObject gravitate).
    pub fn gravity(&self) -> f64 {
        self.option("Gravity").as_f64()
    }

    /// X3DUrlObject.requestImmediateLoad: without LoadUrlObjects no url object of a world loads.
    pub fn load_url_objects(&self) -> bool {
        self.option("LoadUrlObjects").as_bool()
    }

    /// X3DRenderObject.constrainTranslation: `clamp (WallFriction, 0, 1)` (clamped by the caller).
    pub fn wall_friction(&self) -> f64 {
        self.option("WallFriction").as_f64()
    }

    /// `getStraightenHorizon ()`.
    pub fn straighten_horizon(&self) -> bool {
        self.option("StraightenHorizon").as_bool()
    }
}

/// ECMAScript ToInt32 (`value | 0`).
pub fn js_int32(v: f64) -> i32 {
    if !v.is_finite() {
        return 0;
    }

    (v.trunc().rem_euclid(4294967296.0) as u64 as u32) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_page_attributes() {
        let mut b = BrowserOptions::new(512, 512);
        assert_eq!(b.rendering.get("Shading"), Some(&OptionValue::String("GOURAUD".into())));
        assert_eq!(b.multisampling(), 4);

        b.apply_attributes(&[("antialiased", "false"), ("splashScreen", "false")]);
        assert_eq!(b.multisampling(), 0);
        // samples > 0: Antialiased stays true.
        assert_eq!(b.rendering.get("AntiAliased"), Some(&OptionValue::Bool(true)));
        assert_eq!(b.get_option("SplashScreen"), Some(&OptionValue::Bool(false)));
        assert!(b.get_option("gravity").is_none());
    }

    #[test]
    fn deferred_interests_and_reshape() {
        let mut b = BrowserOptions::new(512, 512);
        b.set_option("Shading", OptionValue::String("POINTSET".into()));
        b.set_option("ContentScale", OptionValue::Double(0.0));
        assert_eq!(b.rendering.get("Shading"), Some(&OptionValue::String("GOURAUD".into())));
        b.process_events();
        assert_eq!(b.rendering.get("Shading"), Some(&OptionValue::String("POINT".into())));
        // reshape (): max (0 | 0, 1) / 512.
        assert_eq!(b.rendering.get("ContentScale"), Some(&OptionValue::Double(1.0 / 512.0)));
        assert_eq!(b.framebuffer, (1, 1));
        assert_eq!(js_int32(-3.7), -3);
        assert_eq!(js_int32(4294967297.0), 1);
    }
}
