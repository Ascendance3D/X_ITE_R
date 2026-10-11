//! Fog: Components/EnvironmentalEffects/X3DFogObject.js (`set_fogType__`, `set_visibilityRange__`,
//! FogContainer) and the fragment code of assets/shaders/webgl2/common/Fog2.glsl.js.

/// What FogContainer.setShaderUniforms uploads for the bound fog (fogType 0 draws no fog).
#[derive(Clone, Debug, PartialEq)]
pub struct FogParams {
    /// 1 LINEAR, 2 EXPONENTIAL.
    pub fog_type: u32,
    pub color: [f32; 3],
    pub visibility_start: f32,
    pub visibility_range: f32,
    /// Inverse of the model-view matrix' 3×3 (fog space), column-major like the GLSL mat3.
    pub matrix: [f32; 9],
}

/// X3DFogObject.set_visibilityRange__ + set_fogType__: (fogType, visibilityStart, visibilityRange).
/// `hidden` is the default fog of a layer; a bound Fog node is never hidden.
pub fn fog_type(fog_type_field: &str, visibility_start: f64, visibility_range: f64, hidden: bool) -> (u32, f64, f64) {
    let start = visibility_start.max(0.0);
    let range = visibility_range.max(0.0);
    let start = start.min(range);

    let t = if hidden || (range == 0.0 && start == 0.0) {
        0
    } else {
        match fog_type_field {
            "LINEAR" => 1,
            "EXPONENTIAL" => 2,
            _ => 1,
        }
    };

    (t, start, range)
}

/// Fog2.glsl.js `getFogInterpolant` for a vertex at `distance` from the fog's origin (the length
/// of `x3d_Fog .matrix * vertex`): 0 is the fog color, 1 the material color. Single precision
/// like the GLSL.
pub fn fog_interpolant(fog_type: u32, visibility_start: f32, visibility_range: f32, distance: f32) -> f32 {
    let visibility_start_v = visibility_start;
    let visibility_range_v = visibility_range - visibility_start_v;
    let d_v = (distance - visibility_start_v).max(0.0);

    match fog_type {
        1 => (visibility_range_v - d_v).max(0.0) / visibility_range_v,
        2 => (-d_v / (visibility_range_v - d_v).max(0.001)).exp(),
        _ => 1.0,
    }
}
