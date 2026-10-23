//! ParticleSystem (X_ITE `Components/ParticleSystems/ParticleSystem.js`) hosting its emitters and
//! physics models: PointEmitter, ConeEmitter, ExplosionEmitter, SurfaceEmitter, VolumeEmitter,
//! PolylineEmitter, BoundedPhysicsModel, ForcePhysicsModel and WindPhysicsModel.
//!
//! The ParticleSystem owns the simulation state X_ITE keeps in its transform feedback buffer
//! ([`crate::particles::Sim`], 28 floats per particle) in its [`State::ParticleSystem`], so it is
//! released with the node. The emitters and physics models have no state of their own: every frame
//! their fields are read, like X_ITE reads its uniforms, which are updated by field interests.
//!
//! `animateParticles` is an interest of the browser's `sensorEvents` field, registered while the
//! node is live, active (`isActive`) and `maxParticles` is not 0 (`set_live__`, `set_enabled__`).
//!
//! PolylineEmitter and WindPhysicsModel (REA batch 07): the emitter's `coord` / `coordIndex` become the
//! `polylines` texture of X_ITE ([`particles::polyline_texture`]), cached per emitter like the
//! surface and volume textures and dropped with the node state; WindPhysicsModel draws its speed and
//! (without a `direction`) its direction with `Math.random` on the CPU, in `addForce` order, before the
//! emitter pass draws `randomSeed` ([`particles::next_force_random`]).
//!
//! The color, texCoord and scale ramps (`set_color__`, `set_texCoord__`, `set_scale__`) and the
//! geometry type's scale / rotation matrix are part of the emitter pass ([`particles::Ramps`]); x3d-render
//! draws the buffer (`crates/x3d-render/src/particles.rs`) and computes the bounding boxes. Not ported:
//! `GeoCoordinate` as the coord of a PolylineEmitter.
//!
//! # The geometry provider (a requirement of this crate)
//!
//! SurfaceEmitter, VolumeEmitter and BoundedPhysicsModel read the triangulated geometry of other nodes
//! (`X3DGeometryNode .getVertices ()`), and so does the concave CollidableShape of RigidBodyPhysics
//! (`collidable.rs`, `shape_create_concave`). The tessellation lives in `x3d-render`, which this crate
//! does not depend on, so the embedder installs the builder once per process:
//!
//! * `x3d_render::traverse::install_particle_geometry ()` (`traverse.rs:3229`) calls
//!   [`set_geometry_provider`]; `Viewer3d::load_attributes` does it (`viewer.rs:217`), so every Viewer,
//!   EmbeddedViewer and the oracle drivers have it.
//! * The provider is a process-global [`OnceLock`]: **the first install wins** and stays for the life of the
//!   process. Every [`Runtime`] in the process, including a bare one created before or after the Viewer,
//!   uses it. A bare Runtime in a process where no Viewer was ever created has none.
//! * **Without a provider** a Surface or Volume emitter emits from an empty surface (particles at the
//!   origin or hidden with a NaN position), a BoundedPhysicsModel has no walls, and a concave CollidableShape
//!   has no collision shape. The runtime does not fail; it logs one console warning per node
//!   (`warn: …no geometry provider is installed…`, [`Runtime::warn_no_geometry_provider`]) the first time
//!   such a node needs geometry. When a provider is installed nothing is logged, so the console of every
//!   Viewer based run (the oracle's included) is unchanged.
//! * Observed, not assumed (tests `geometry_provider_*` in `x3d-runtime/tests/geometry_provider_*.rs` and
//!   `x3d-viewer/tests/geometry_provider_viewer.rs`): a bare Runtime warns and has empty geometry; a Runtime
//!   after [`set_geometry_provider`] does not warn and has geometry; two Runtimes in one process behave
//!   alike; a Runtime in a process in which a Viewer was created is masked by the Viewer's provider (no
//!   warning, geometry present) even if it never saw a Viewer itself. The reverse does not hold: a Runtime that
//!   ran *before* the install keeps the empty geometry it cached (the cache key is the geometry, not the
//!   provider) and does not log again.

use crate::nodes::State;
use crate::particles::{
    self, EmitterKind, EmitterParams, GeometryTexture, GeometryType, Pass, Ramp, Ramps, Sim,
};
use crate::*;
use std::collections::HashSet;
use std::sync::OnceLock;
use x3d_fields::jsmath;
use x3d_fields::math::Vector3;

/// Interest callbacks of a ParticleSystem.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ParticleCb {
    /// `set_enabled__` (`enabled` and `maxParticles`).
    SetEnabled,
    /// `animateParticles` on the browser's `sensorEvents`.
    Animate,
}

// --- geometry of the surface, volume and bounded models ---------------------------------------

/// What a [`GeometryProvider`] is asked for.
#[derive(Clone, Debug)]
pub enum GeometryRequest {
    /// `X3DGeometryNode .getVertices ()` and `.getNormals ()` of this geometry node.
    Node(NodeId),
    /// The private IndexedFaceSet of a VolumeEmitter: `coord` and `coordIndex`, `creaseAngle` PI,
    /// `convex` FALSE, everything else default.
    Volume {
        coord: Option<NodeId>,
        coord_index: Vec<i32>,
    },
}

/// The triangulated geometry as X_ITE builds it.
#[derive(Clone, Debug, Default)]
pub struct GeometrySoup {
    /// x y z 1 per vertex, 3 vertices per triangle.
    pub vertices: Vec<f32>,
    /// x y z per vertex.
    pub normals: Vec<f32>,
    /// The geometry node's `solid`.
    pub solid: bool,
}

/// Builds the triangle arrays of a geometry node. The runtime cannot build geometry (the
/// tessellation lives in x3d-render), so the embedder installs the builder, see
/// `x3d_render::traverse::install_particle_geometry`. Without one, surfaces, volumes and bounded
/// models are empty and the runtime logs a warning once per node (see the module documentation).
pub type GeometryProvider = fn(&Runtime, &GeometryRequest) -> Option<GeometrySoup>;

static PROVIDER: OnceLock<GeometryProvider> = OnceLock::new();

/// Install the process-global builder. The first call wins; later calls (another Viewer) are ignored.
pub fn set_geometry_provider(provider: GeometryProvider) {
    let _ = PROVIDER.set(provider);
}

/// The installed builder (CollidableShape builds its collision shapes from the same vertices).
pub fn geometry_provider() -> Option<GeometryProvider> {
    PROVIDER.get().copied()
}

// --- node state --------------------------------------------------------------------------------

struct EmitterCache {
    node: NodeId,
    is_polyline: bool,
    signature: u64,
    texture: Option<GeometryTexture>,
    solid: bool,
}

struct BoundedCache {
    signature: u64,
    texture: Option<GeometryTexture>,
}

/// The state of one ParticleSystem.
#[derive(Default)]
pub struct PsState {
    pub sim: Sim,
    /// The ramps of the last `animateParticles` (the renderer reads the texCoord ramp and whether the color
    /// ramp is used).
    pub ramps: Ramps,
    emitter_cache: Option<EmitterCache>,
    bounded_cache: Option<BoundedCache>,
}

impl std::fmt::Debug for PsState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PsState")
            .field("max", &self.sim.max_particles)
            .field("num", &self.sim.num_particles)
            .finish()
    }
}

fn emitter_kind(name: &str) -> Option<EmitterKind> {
    match name {
        "PointEmitter" => Some(EmitterKind::Point),
        "ConeEmitter" => Some(EmitterKind::Cone),
        "ExplosionEmitter" => Some(EmitterKind::Explosion),
        "SurfaceEmitter" => Some(EmitterKind::Surface),
        "VolumeEmitter" => Some(EmitterKind::Volume),
        "PolylineEmitter" => Some(EmitterKind::Polyline),
        _ => None,
    }
}

impl Runtime {
    /// Log, once per node, that `what` of `node` needs the geometry provider and none is installed
    /// (see the module documentation). Does nothing when a provider is installed.
    pub(crate) fn warn_no_geometry_provider(&mut self, node: NodeId, what: &str) {
        if PROVIDER.get().is_some() || !self.provider_warned.insert(node) {
            return;
        }

        let name = self.builtin_name(node).unwrap_or("node");

        self.log_warn(format!("{name}: no geometry provider is installed (x3d_render::traverse::install_particle_geometry), the geometry of {what} is empty."));
    }

    /// What of this ParticleSystem needs the geometry provider, if anything is set.
    fn ps_needs_provider(
        &self,
        ps: NodeId,
        emitter: Option<NodeId>,
        kind: EmitterKind,
    ) -> Option<&'static str> {
        let set = |n: NodeId, field: &str| {
            matches!(self.pre_value(n, field), Some(FieldValue::SFNode(Some(_))))
        };

        match (emitter, kind) {
            (Some(e), EmitterKind::Surface) if set(e, "surface") => {
                return Some("its SurfaceEmitter surface");
            }
            (Some(e), EmitterKind::Volume) if set(e, "coord") => return Some("its VolumeEmitter"),
            _ => {}
        }

        self.ps_physics_nodes(ps)
            .into_iter()
            .any(|m| {
                self.builtin_name(m) == Some("BoundedPhysicsModel")
                    && matches!(self.pre_value(m, "enabled"), Some(FieldValue::SFBool(true)))
                    && set(m, "geometry")
            })
            .then_some("its BoundedPhysicsModel")
    }

    fn ps_take(&mut self, node: NodeId) -> Option<Box<PsState>> {
        match std::mem::take(&mut self.nrt_mut(node).state) {
            State::ParticleSystem(s) => Some(s),
            other => {
                self.nrt_mut(node).state = other;
                None
            }
        }
    }

    fn ps_put(&mut self, node: NodeId, st: Box<PsState>) {
        self.nrt_mut(node).state = State::ParticleSystem(st);
    }

    /// The simulation state of a ParticleSystem (the contents of its transform feedback buffer).
    pub fn particle_sim(&self, node: NodeId) -> Option<&Sim> {
        match &self.nrt(node)?.state {
            State::ParticleSystem(s) => Some(&s.sim),
            _ => None,
        }
    }

    /// The particle records of a ParticleSystem, for tools that put a reference buffer in place (x3d-particles-drive
    /// `pickProbes`: X_ITE's records under the pointing pass, then the run's own back).
    pub fn particle_records_mut(
        &mut self,
        node: NodeId,
    ) -> Option<&mut [crate::particles::Particle]> {
        match &mut self.nrt_mut(node).state {
            State::ParticleSystem(s) => Some(&mut s.sim.particles[..]),
            _ => None,
        }
    }

    /// The ramps of a ParticleSystem as its last `animateParticles` used them.
    pub fn particle_ramps(&self, node: NodeId) -> Option<&Ramps> {
        match &self.nrt(node)?.state {
            State::ParticleSystem(s) => Some(&s.ramps),
            _ => None,
        }
    }

    /// `set_color__`, `set_texCoord__` and `set_scale__` of a ParticleSystem: its ramp textures. The texCoord
    /// ramp holds `texCoordCount` (4 for the quad types, 0 otherwise) values per key.
    pub fn particle_system_ramps(&self, ps: NodeId) -> Ramps {
        let keys = |name: &str| match self.pre_value(ps, name) {
            Some(FieldValue::MFFloat(v)) => v.clone(),
            _ => vec![],
        };
        let node = |name: &str| match self.pre_value(ps, name) {
            Some(FieldValue::SFNode(Some(n))) => self.inner_node(*n),
            _ => None,
        };
        let floats = |n: NodeId, name: &str| -> Vec<f32> {
            match self.pre_value(n, name) {
                Some(
                    FieldValue::MFColor(v)
                    | FieldValue::MFColorRGBA(v)
                    | FieldValue::MFVec2f(v)
                    | FieldValue::MFVec3f(v)
                    | FieldValue::MFVec4f(v),
                ) => v.clone(),
                _ => vec![],
            }
        };
        // The key texels, then the values.
        let ramp = |keys: &[f32], values: Vec<[f32; 4]>| Ramp {
            count: keys.len(),
            texels: keys
                .iter()
                .map(|k| [*k, 0.0, 0.0, 0.0])
                .chain(values)
                .collect(),
        };

        // set_color__: X3DCast (X3DColorNode, color) .addColors ([ ], numKeys) (the colors repeat), or white.
        let color_keys = keys("colorKey");
        let colors = (!color_keys.is_empty()).then(|| {
            let n = color_keys.len();
            let values = match node("color").map(|c| (self.builtin_name(c), c)) {
                Some((Some(ty @ ("Color" | "ColorRGBA")), c)) => {
                    let width = if ty == "Color" { 3 } else { 4 };
                    let v = floats(c, "color");
                    let length = v.len() / width;

                    (0..n)
                        .map(|i| match length {
                            0 => [1.0; 4],
                            _ => {
                                let k = (i % length) * width;

                                [
                                    v[k],
                                    v[k + 1],
                                    v[k + 2],
                                    if width == 3 { 1.0 } else { v[k + 3] },
                                ]
                            }
                        })
                        .collect()
                }
                _ => vec![[1.0; 4]; n],
            };

            ramp(&color_keys, values)
        });

        // set_texCoord__: X3DCast (X3DTextureCoordinateNode, texCoord) .addPoints ([ ]), numKeys * texCoordCount of them;
        // without the node X3D_NUM_TEX_COORDS is 0.
        let geometry_type = self.particle_geometry_type(ps);
        let tex_coords_count = geometry_type.tex_coord_count();
        let tex_coord_keys = keys("texCoordKey");
        let tex_coords = node("texCoord").and_then(|t| {
            let points = self.tex_coord_points(t)?;
            let values: Vec<[f32; 4]> = points
                .into_iter()
                .take(tex_coord_keys.len() * tex_coords_count)
                .collect();

            Some(ramp(&tex_coord_keys, values))
        });
        let tex_coords = tex_coords.filter(|r| r.count > 0);

        // set_scale__: X3DCast (Coordinate, scale) .addPoints ([ ], numKeys) (the last point repeats, none is 0 0 0), or 1.
        let scale_keys = keys("scaleKey");
        let scales = (!scale_keys.is_empty()).then(|| {
            let n = scale_keys.len();
            let values = match node("scale").filter(|c| self.builtin_name(*c) == Some("Coordinate"))
            {
                Some(c) => {
                    let v = floats(c, "point");
                    let mut points: Vec<[f32; 4]> =
                        v.chunks_exact(3).map(|p| [p[0], p[1], p[2], 1.0]).collect();
                    let last = points.last().copied().unwrap_or([0.0, 0.0, 0.0, 1.0]);

                    while points.len() < n {
                        points.push(last);
                    }

                    points.truncate(n);
                    points
                }
                None => vec![[1.0; 4]; n],
            };

            ramp(&scale_keys, values)
        });

        Ramps {
            colors,
            tex_coords,
            tex_coords_count,
            scales,
        }
    }

    /// `addPoints ([ ])` of an X3DTextureCoordinateNode (a MultiTextureCoordinate's first one); `None` when
    /// it is not one.
    fn tex_coord_points(&self, t: NodeId) -> Option<Vec<[f32; 4]>> {
        let floats = |name: &str| match self.pre_value(t, name) {
            Some(FieldValue::MFVec2f(v) | FieldValue::MFVec3f(v) | FieldValue::MFVec4f(v)) => {
                v.clone()
            }
            _ => vec![],
        };

        match self.builtin_name(t)? {
            "TextureCoordinate" => Some(
                floats("point")
                    .chunks_exact(2)
                    .map(|p| [p[0], p[1], 0.0, 1.0])
                    .collect(),
            ),
            "TextureCoordinate3D" => Some(
                floats("point")
                    .chunks_exact(3)
                    .map(|p| [p[0], p[1], p[2], 1.0])
                    .collect(),
            ),
            "TextureCoordinate4D" => Some(
                floats("point")
                    .chunks_exact(4)
                    .map(|p| [p[0], p[1], p[2], p[3]])
                    .collect(),
            ),
            "TextureCoordinateGenerator" => Some(vec![]),
            "MultiTextureCoordinate" => {
                let first = match self.pre_value(t, "texCoord") {
                    Some(FieldValue::MFNode(v)) => v
                        .iter()
                        .flatten()
                        .filter_map(|n| self.inner_node(*n))
                        .find(|n| {
                            matches!(
                                self.builtin_name(*n),
                                Some(
                                    "TextureCoordinate"
                                        | "TextureCoordinate3D"
                                        | "TextureCoordinate4D"
                                        | "TextureCoordinateGenerator"
                                )
                            )
                        }),
                    _ => None,
                };

                Some(
                    first
                        .and_then(|n| self.tex_coord_points(n))
                        .unwrap_or_default(),
                )
            }
            _ => None,
        }
    }

    /// `set_geometryType__`: the ParticleSystem's geometry type.
    pub fn particle_geometry_type(&self, ps: NodeId) -> GeometryType {
        match self.pre_value(ps, "geometryType") {
            Some(FieldValue::SFString(s)) => GeometryType::from_name(s),
            _ => GeometryType::Quad,
        }
    }

    /// The geometry texture of the ParticleSystem's PolylineEmitter (`polylines`, REA batch 07), once built by
    /// an `animateParticles`; `None` for other emitters.
    pub fn particle_polyline_texture(&self, node: NodeId) -> Option<&GeometryTexture> {
        match &self.nrt(node)?.state {
            State::ParticleSystem(s) => s
                .emitter_cache
                .as_ref()
                .filter(|c| c.is_polyline)
                .and_then(|c| c.texture.as_ref()),
            _ => None,
        }
    }

    /// The number of cached geometry texels of this ParticleSystem (emitter textures plus bounded model), a measure
    /// of what a lifecycle test expects to be released with the node.
    pub fn particle_cache_texels(&self, node: NodeId) -> usize {
        match self.nrt(node).map(|n| &n.state) {
            Some(State::ParticleSystem(s)) => {
                s.emitter_cache
                    .as_ref()
                    .and_then(|c| c.texture.as_ref())
                    .map_or(0, |t| t.array.len())
                    + s.bounded_cache
                        .as_ref()
                        .and_then(|c| c.texture.as_ref())
                        .map_or(0, |t| t.array.len())
            }
            _ => 0,
        }
    }

    /// Back to the state before the first `animateParticles`: nothing alive, `deltaTime` 0, zeroed
    /// buffers, `creationTime` `t0` (0 for an ExplosionEmitter, whose first burst is the first
    /// animate). The oracle harness does the same after loading the world.
    pub fn particle_system_reset(&mut self, node: NodeId, t0: f64) {
        let explosive = self
            .ps_emitter_params(self.ps_emitter_node(node))
            .is_explosive();

        if let Some(mut st) = self.ps_take(node) {
            st.sim.reset(if explosive { 0.0 } else { t0 });
            self.ps_put(node, st);
        }
    }

    /// The emitter node of a ParticleSystem (`X3DCast (X3DParticleEmitterNode, this ._emitter)`).
    fn ps_emitter_node(&self, ps: NodeId) -> Option<NodeId> {
        let Some(FieldValue::SFNode(Some(n))) = self.pre_value(ps, "emitter") else {
            return None;
        };
        let inner = self.inner_node(*n)?;

        emitter_kind(self.builtin_name(inner)?).map(|_| inner)
    }

    fn ps_vec3(&self, node: NodeId, name: &str, default: [f64; 3]) -> [f64; 3] {
        match self.pre_value(node, name) {
            Some(FieldValue::SFVec3f(v)) => [v.x, v.y, v.z],
            _ => default,
        }
    }

    /// The uniforms of the emitter (`set_position__`, `set_direction__`, `set_speed__`, …).
    fn ps_emitter_params(&self, emitter: Option<NodeId>) -> EmitterParams {
        let mut p = EmitterParams::default_point();
        let Some(e) = emitter else {
            return p;
        };
        let Some(kind) = self.builtin_name(e).and_then(emitter_kind) else {
            return p;
        };

        p.kind = kind;
        p.on = !matches!(self.pre_value(e, "on"), Some(FieldValue::SFBool(false)));

        let position = self.ps_vec3(e, "position", [0.0; 3]);

        p.position = [position[0] as f32, position[1] as f32, position[2] as f32];

        match kind {
            // PointEmitter.set_direction__ and VolumeEmitter.set_direction__ normalize in JavaScript.
            EmitterKind::Point | EmitterKind::Volume | EmitterKind::Polyline => {
                let d = self.ps_vec3(e, "direction", [0.0, 1.0, 0.0]);
                let mut v = Vector3::new(d[0], d[1], d[2]);

                v.normalize();
                p.direction = [v.x as f32, v.y as f32, v.z as f32];
            }
            // ConeEmitter uploads the direction as is.
            EmitterKind::Cone => {
                let d = self.ps_vec3(e, "direction", [0.0, 1.0, 0.0]);

                p.direction = [d[0] as f32, d[1] as f32, d[2] as f32];
            }
            EmitterKind::Explosion | EmitterKind::Surface => p.direction = [0.0; 3],
        }

        if kind == EmitterKind::Cone {
            p.angle = match self.pre_value(e, "angle") {
                Some(FieldValue::SFFloat(a)) => *a as f32,
                _ => 0.785398,
            };
        }

        let field = |name: &str, default: f64| match self.pre_value(e, name) {
            Some(FieldValue::SFFloat(v)) => *v,
            _ => default,
        };

        p.speed = jsmath::js_max(field("speed", 0.0), 0.0) as f32;
        p.variation = jsmath::js_max(field("variation", 0.25), 0.0) as f32;
        p.mass = jsmath::js_max(field("mass", 0.0), 0.0);
        p.surface_area = jsmath::js_max(field("surfaceArea", 0.0), 0.0);
        p
    }

    /// A hash of the stamps of `roots` and of every node below them: it changes when a field of
    /// any of them is set (what `_rebuild` and the node interests tell X_ITE).
    fn closure_signature(&self, roots: &[NodeId]) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        let mut seen = HashSet::new();
        let mut stack: Vec<NodeId> = roots.iter().rev().copied().collect();

        while let Some(n) = stack.pop() {
            if !seen.insert(n) || seen.len() > 100_000 {
                continue;
            }

            h = (h ^ u64::from(n)).wrapping_mul(0x0100_0000_01b3);
            h = (h ^ self.stamp(n)).wrapping_mul(0x0100_0000_01b3);

            if let NodeKind::Instance(_) = self.scene.node(n).kind {
                if let Some(body) = self.instance_body(n) {
                    stack.extend(self.scene.ctx(body).root_nodes.iter().flatten().rev());
                }
            }

            let mut children = vec![];

            for f in &self.scene.node(n).fields {
                children.extend(Self::node_values_pub(&f.value));
            }

            stack.extend(children.into_iter().rev());
        }

        h
    }

    /// The surface / volume texture of the emitter (`SurfaceEmitter.set_geometry__`,
    /// `VolumeEmitter.set_geometry__`), rebuilt when the emitter or what it samples changed.
    fn ps_emitter_geometry(
        &self,
        cache: &mut Option<EmitterCache>,
        emitter: Option<NodeId>,
        kind: EmitterKind,
    ) {
        let Some(e) = emitter.filter(|_| {
            matches!(
                kind,
                EmitterKind::Surface | EmitterKind::Volume | EmitterKind::Polyline
            )
        }) else {
            *cache = None;
            return;
        };
        let signature = self.closure_signature(&[e]);

        if cache
            .as_ref()
            .is_some_and(|c| c.node == e && c.signature == signature)
        {
            return;
        }

        if kind == EmitterKind::Polyline {
            let texture = particles::polyline_texture(&self.polyline_vertices(e));

            *cache = Some(EmitterCache {
                node: e,
                is_polyline: true,
                signature,
                texture: Some(texture),
                solid: true,
            });
            return;
        }

        let provider = PROVIDER.get().copied();
        let (texture, solid) = if kind == EmitterKind::Surface {
            let surface = match self.pre_value(e, "surface") {
                Some(FieldValue::SFNode(Some(n))) => self.inner_node(*n),
                _ => None,
            };
            let soup =
                surface.and_then(|g| provider.and_then(|p| p(self, &GeometryRequest::Node(g))));

            (
                soup.as_ref()
                    .map(|s| particles::surface_texture(&s.vertices, &s.normals)),
                soup.map_or(true, |s| s.solid),
            )
        } else {
            let coord = match self.pre_value(e, "coord") {
                Some(FieldValue::SFNode(Some(n))) => self.inner_node(*n),
                _ => None,
            };
            let coord_index = match self.pre_value(e, "coordIndex") {
                Some(FieldValue::MFInt32(v)) => v.clone(),
                _ => vec![],
            };
            let soup = provider
                .and_then(|p| p(self, &GeometryRequest::Volume { coord, coord_index }))
                .unwrap_or_default();

            (
                Some(particles::volume_texture(&soup.vertices, &soup.normals)),
                true,
            )
        };

        *cache = Some(EmitterCache {
            node: e,
            is_polyline: false,
            signature,
            texture,
            solid,
        });
    }

    /// `IndexedLineSet .getVertices ()` of the private IndexedLineSet of a PolylineEmitter: x y z 1 per
    /// vertex, two per segment, for every polyline of `coordIndex` (a run of indices >= 0, ended by any
    /// negative index or the end) with at least two points. An index past the end of `coord` is the origin.
    /// Nothing without a Coordinate / CoordinateDouble that has points.
    fn polyline_vertices(&self, emitter: NodeId) -> Vec<f64> {
        let coord = match self.pre_value(emitter, "coord") {
            Some(FieldValue::SFNode(Some(n))) => self.inner_node(*n),
            _ => None,
        };
        let points: Vec<f64> =
            match coord.and_then(
                |c| match (self.builtin_name(c), self.pre_value(c, "point")) {
                    (Some("Coordinate"), Some(FieldValue::MFVec3f(v))) => {
                        Some(v.iter().map(|&x| f64::from(x)).collect())
                    }
                    (Some("CoordinateDouble"), Some(FieldValue::MFVec3d(v))) => Some(v.clone()),
                    _ => None,
                },
            ) {
                Some(p) if !p.is_empty() => p,
                _ => return vec![],
            };
        let size = points.len() / 3;
        let coord_index = match self.pre_value(emitter, "coordIndex") {
            Some(FieldValue::MFInt32(v)) => v.as_slice(),
            _ => &[],
        };
        let mut polylines: Vec<Vec<i32>> = vec![];
        let mut polyline: Vec<i32> = vec![];

        for &index in coord_index {
            if index >= 0 {
                polyline.push(index);
            } else {
                polylines.push(std::mem::take(&mut polyline));
            }
        }

        if coord_index.last().is_some_and(|&i| i >= 0) {
            polylines.push(polyline);
        }

        let mut vertices = vec![];
        let mut add = |index: i32| match usize::try_from(index).ok().filter(|&i| i < size) {
            Some(i) => vertices.extend_from_slice(&[
                points[i * 3],
                points[i * 3 + 1],
                points[i * 3 + 2],
                1.0,
            ]),
            None => vertices.extend_from_slice(&[0.0, 0.0, 0.0, 1.0]),
        };

        for polyline in polylines.iter().filter(|p| p.len() > 1) {
            for pair in polyline.windows(2) {
                add(pair[0]);
                add(pair[1]);
            }
        }

        vertices
    }

    /// The inner nodes of the `physics` field, in order.
    fn ps_physics_nodes(&self, ps: NodeId) -> Vec<NodeId> {
        match self.pre_value(ps, "physics") {
            Some(FieldValue::MFNode(v)) => v
                .iter()
                .flatten()
                .filter_map(|n| self.inner_node(*n))
                .collect(),
            _ => vec![],
        }
    }

    /// `set_boundedPhysics__`: the soup of the enabled BoundedPhysicsModels, rebuilt on change.
    fn ps_bounded_geometry(&self, cache: &mut Option<BoundedCache>, physics: &[NodeId]) {
        let signature = self.closure_signature(physics);

        if cache.as_ref().is_some_and(|c| c.signature == signature) {
            return;
        }

        let provider = PROVIDER.get().copied();
        let (mut vertices, mut normals) = (vec![], vec![]);

        for &model in physics {
            if self.builtin_name(model) != Some("BoundedPhysicsModel")
                || !matches!(
                    self.pre_value(model, "enabled"),
                    Some(FieldValue::SFBool(true))
                )
            {
                continue;
            }

            let geometry = match self.pre_value(model, "geometry") {
                Some(FieldValue::SFNode(Some(n))) => self.inner_node(*n),
                _ => None,
            };
            let Some(soup) =
                geometry.and_then(|g| provider.and_then(|p| p(self, &GeometryRequest::Node(g))))
            else {
                continue;
            };
            let damping = self.get_f64(model, "damping");

            normals.extend(
                soup.normals
                    .iter()
                    .map(|&v| (f64::from(v) * damping) as f32),
            );
            vertices.extend_from_slice(&soup.vertices);
        }

        let texture =
            (!vertices.is_empty()).then(|| particles::bounded_texture(&vertices, &normals));

        *cache = Some(BoundedCache { signature, texture });
    }

    // --- initialize and the interests ---------------------------------------------------------

    /// ParticleSystem.initialize.
    pub(crate) fn particle_system_initialize(&mut self, node: NodeId) {
        self.watch_live(node);
        self.interest(node, "enabled", Cb::Particle(ParticleCb::SetEnabled));
        self.interest(node, "maxParticles", Cb::Particle(ParticleCb::SetEnabled));
        self.ps_set_enabled(node);
    }

    /// VolumeEmitter.initialize: `this ._set_coordIndex .addFieldInterest (this ._coordIndex)`.
    pub(crate) fn volume_emitter_initialize(&mut self, node: NodeId) {
        if let (Some(from), Some(to)) = (
            self.pre(node, "set_coordIndex"),
            self.pre(node, "coordIndex"),
        ) {
            self.add_field_interest(from, to);
        }
    }

    /// PolylineEmitter.initialize: `this ._set_coordIndex .addFieldInterest (this ._coordIndex)`.
    pub(crate) fn polyline_emitter_initialize(&mut self, node: NodeId) {
        self.volume_emitter_initialize(node);
    }

    pub(crate) fn particle_interest(&mut self, node: NodeId, cb: ParticleCb) {
        match cb {
            ParticleCb::SetEnabled => self.ps_set_enabled(node),
            ParticleCb::Animate => self.ps_animate(node),
        }
    }

    fn ps_sensor_events(&self) -> FKey {
        self.private_field(self.browser_node, "sensorEvents")
            .expect("browser sensorEvents")
    }

    fn ps_max_field(&self, node: NodeId) -> i32 {
        match self.pre_value(node, "maxParticles") {
            Some(FieldValue::SFInt32(v)) => *v,
            _ => 200,
        }
    }

    fn ps_set_active(&mut self, node: NodeId, active: bool) {
        if let Some(k) = self.pre(node, "isActive") {
            self.set_value(k, FieldValue::SFBool(active));
        }
    }

    /// ParticleSystem.set_live__ (the node's live state changed).
    pub(crate) fn particle_system_set_live(&mut self, node: NodeId) {
        let Some(mut st) = self.ps_take(node) else {
            return;
        };
        let live = self.ctx_live(self.scene.node(node).ctx);
        let running = self.get_bool(node, "isActive") && self.ps_max_field(node) != 0;
        let sensor = self.ps_sensor_events();
        let now = self.now * 1000.0 / 1000.0;

        if live {
            if running {
                self.add_interest(sensor, node, Cb::Particle(ParticleCb::Animate));

                if st.sim.pause_time != 0.0 {
                    st.sim.creation_time += now - st.sim.pause_time;
                    st.sim.pause_time = 0.0;
                }
            }
        } else if running {
            self.remove_interest(sensor, node, Cb::Particle(ParticleCb::Animate));

            if st.sim.pause_time == 0.0 {
                st.sim.pause_time = now;
            }
        }

        self.ps_put(node, st);
    }

    /// ParticleSystem.set_enabled__, then set_maxParticles__.
    fn ps_set_enabled(&mut self, node: NodeId) {
        let Some(mut st) = self.ps_take(node) else {
            return;
        };
        let live = self.ctx_live(self.scene.node(node).ctx);
        let on = self.get_bool(node, "enabled") && self.ps_max_field(node) != 0;
        let active = self.get_bool(node, "isActive");
        let sensor = self.ps_sensor_events();
        let now = self.now * 1000.0 / 1000.0;

        if on {
            if !active {
                if live {
                    self.add_interest(sensor, node, Cb::Particle(ParticleCb::Animate));
                    st.sim.pause_time = 0.0;
                } else {
                    st.sim.pause_time = now;
                }

                self.ps_set_active(node, true);
            }
        } else if active {
            if live {
                self.remove_interest(sensor, node, Cb::Particle(ParticleCb::Animate));
            }

            self.ps_set_active(node, false);
            st.sim.num_particles = 0;
        }

        // set_maxParticles__
        let last = st.sim.num_particles;
        let max = self.ps_max_field(node).max(0) as usize;

        st.sim.num_particles = last.min(max);

        if !self
            .ps_emitter_params(self.ps_emitter_node(node))
            .is_explosive()
        {
            st.sim.creation_time = now;
        }

        st.sim.resize(max, last);
        self.ps_put(node, st);
    }

    /// WindPhysicsModel.addForce: the `Math.random` draws happen here, on the CPU, in this order:
    /// `getRandomSpeed` (1), then `getRandomNormal` (2: theta, cphi) when `direction` is the zero vector.
    fn wind_force(&self, model: NodeId, emitter: &EmitterParams, time_by_mass: f64) -> [f32; 4] {
        let field = |name: &str, default: f64| match self.pre_value(model, name) {
            Some(FieldValue::SFFloat(v)) => *v,
            _ => default,
        };
        let random_value = |min: f64, max: f64| particles::next_force_random() * (max - min) + min;

        let speed = jsmath::js_max(field("speed", 0.1), 0.0);
        let variation = speed * jsmath::js_max(field("gustiness", 0.1), 0.0);
        let speed = random_value(jsmath::js_max(0.0, speed - variation), speed + variation);
        let pressure = jsmath::pow(10.0, 2.0 * jsmath::log(speed)) * 0.64615;
        let direction = self.ps_vec3(model, "direction", [1.0, 0.0, 0.0]);
        let mut force;

        if direction == [0.0; 3] {
            // Emitter.getRandomNormal
            let theta = random_value(-1.0, 1.0) * std::f64::consts::PI;
            let cphi = random_value(-1.0, 1.0);
            let phi = jsmath::acos(cphi);
            let r = jsmath::sin(phi);

            force = [jsmath::sin(theta) * r, jsmath::cos(theta) * r, cphi];
        } else {
            let mut v = Vector3::new(direction[0], direction[1], direction[2]);

            v.normalize();
            force = [v.x, v.y, v.z];
        }

        let s = emitter.surface_area * pressure * time_by_mass;

        for c in &mut force {
            *c *= s;
        }

        [
            force[0] as f32,
            force[1] as f32,
            force[2] as f32,
            (std::f64::consts::PI * jsmath::clamp(field("turbulence", 0.0), 0.0, 1.0)) as f32,
        ]
    }

    // --- animateParticles ---------------------------------------------------------------------

    fn ps_animate(&mut self, node: NodeId) {
        let Some(mut st) = self.ps_take(node) else {
            return;
        };
        let emitter_node = self.ps_emitter_node(node);

        let mut e = self.ps_emitter_params(emitter_node);

        if PROVIDER.get().is_none() && !self.provider_warned.contains(&node) {
            if let Some(what) = self.ps_needs_provider(node, emitter_node, e.kind) {
                self.warn_no_geometry_provider(node, what);
            }
        }

        let PsState {
            sim,
            emitter_cache,
            bounded_cache,
            ..
        } = &mut *st;

        self.ps_emitter_geometry(emitter_cache, emitter_node, e.kind);

        if e.kind == EmitterKind::Surface {
            e.solid = emitter_cache.as_ref().map_or(true, |c| c.solid);
        }

        let pl = jsmath::js_max(self.get_f64(node, "particleLifetime"), 0.0);
        let lv = jsmath::js_max(self.get_f64(node, "lifetimeVariation"), 0.0);
        let create_field = self.get_bool(node, "createParticles");
        let delta_time = sim.schedule(
            self.frame_rate,
            self.now * 1000.0,
            e.is_explosive(),
            pl,
            lv,
            create_field,
        );
        let physics = self.ps_physics_nodes(node);

        // Forces (ParticleSystem.animateParticles): only with a mass, `addForce` of every force model in
        // `physics` order; a disabled model adds no row (`i - disabledForces`).
        let mut forces: Vec<[f32; 4]> = vec![];

        if e.mass != 0.0 {
            let time_by_mass = delta_time / e.mass;

            for &model in &physics {
                if !matches!(
                    self.pre_value(model, "enabled"),
                    Some(FieldValue::SFBool(true))
                ) {
                    continue;
                }

                match self.builtin_name(model) {
                    Some("ForcePhysicsModel") => {
                        // ForcePhysicsModel.addForce: `force * timeByMass`, turbulence 0.
                        let f = self.ps_vec3(model, "force", [0.0, -9.8, 0.0]);

                        forces.push([
                            (f[0] * time_by_mass) as f32,
                            (f[1] * time_by_mass) as f32,
                            (f[2] * time_by_mass) as f32,
                            0.0,
                        ]);
                    }
                    Some("WindPhysicsModel") => {
                        forces.push(self.wind_force(model, &e, time_by_mass))
                    }
                    _ => {}
                }
            }
        }

        sim.num_forces = forces.len();
        sim.forces.clone_from(&forces);

        self.ps_bounded_geometry(bounded_cache, &physics);

        st.ramps = self.particle_system_ramps(node);

        let particle_size = match self.pre_value(node, "particleSize") {
            Some(FieldValue::SFVec2f(v)) => [v.x as f32, v.y as f32],
            _ => [0.02, 0.02],
        };
        let seed = particles::next_seed();
        let PsState {
            sim,
            emitter_cache,
            bounded_cache,
            ramps,
        } = &mut *st;
        let pass = Pass {
            emitter: &e,
            create: sim.create_particles && e.on,
            random_seed: particles::seed_uniform(seed),
            particle_lifetime: pl as f32,
            lifetime_variation: lv as f32,
            delta_time: delta_time as f32,
            forces: &forces,
            texture: emitter_cache.as_ref().and_then(|c| c.texture.as_ref()),
            bounded: bounded_cache
                .as_ref()
                .and_then(|c| c.texture.as_ref())
                .filter(|t| t.hierarchy_root > -1),
            geometry_type: self.particle_geometry_type(node),
            particle_size,
            ramps,
        };

        particles::run_pass(&mut sim.particles, sim.num_particles, &pass);
        self.ps_put(node, st);
    }
}
