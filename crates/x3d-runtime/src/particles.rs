//! The CPU simulation of X_ITE 16.5.0's ParticleSystems (REA batch 06R): a port of the GLSL vertex
//! shader that X_ITE runs as a transform feedback pass (`X3DParticleEmitterNode.animate`, the
//! `main ()` of `createProgram`), of the emitters' `getRandomVelocity` / `getRandomPosition`
//! (PointEmitter, ConeEmitter, ExplosionEmitter, SurfaceEmitter, VolumeEmitter), of the BVH
//! texture (`standard/Math/Utility/BVH.js`, `Browser/ParticleSystems/*.glsl.js`), of the
//! BoundedPhysicsModel `bounce` and of the CPU half of `ParticleSystem.animateParticles`.
//!
//! The state is what X_ITE keeps in its interleaved transform feedback buffer: 28 float32 per
//! particle ([`Particle`]). All shader math is `f32` and is evaluated operation by operation (no
//! fused multiply-add), in the draw order of the GLSL, because the random numbers come from the
//! shader's LCG and a single misplaced draw changes every later value of that particle.
//!
//! Nothing here depends on the scene: [`crate::nodes::particle_systems`] reads the node fields,
//! builds the geometry textures and drives [`Sim`].

use std::cell::RefCell;
use std::collections::VecDeque;
use x3d_fields::jsmath;

/// Floats per particle record (7 x vec4).
pub const STRIDE: usize = 28;

/// `[0] life [1] lifetime [2] elapsedTime [3] texCoordIndex0 [4..8] color [8..11] velocity
/// [11] 0 [12..24] scale/rotation matrix rows [24..28] position`.
pub type Particle = [f32; STRIDE];

pub type V3 = [f32; 3];

/// GLSL `ARRAY_SIZE` of the intersection arrays.
const ARRAY_SIZE: usize = 32;
/// GLSL `BVH_STACK_SIZE`.
const BVH_STACK_SIZE: usize = 32;

const M_PI: f32 = std::f32::consts::PI;

// ---------------------------------------------------------------------------------------------
// float32 vector math (GLSL semantics)

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - b[1] * a[2],
        a[2] * b[0] - b[2] * a[0],
        a[0] * b[1] - b[0] * a[1],
    ]
}

fn normalize(v: V3) -> V3 {
    let l = dot(v, v).sqrt();

    [v[0] / l, v[1] / l, v[2] / l]
}

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: V3, s: f32) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn sign(x: f32) -> f32 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// GLSL `Quaternion (fromVector, toVector)`.
fn quaternion(from: V3, to: V3) -> [f32; 4] {
    let from = normalize(from);
    let to = normalize(to);
    let cos_angle = dot(from, to);
    let cross_vec = cross(from, to);
    let cross_len = dot(cross_vec, cross_vec).sqrt();

    if cross_len == 0.0 {
        if cos_angle > 0.0 {
            return [0.0, 0.0, 0.0, 1.0];
        }

        let mut t = cross(from, [1.0, 0.0, 0.0]);

        if dot(t, t) == 0.0 {
            t = cross(from, [0.0, 1.0, 0.0]);
        }

        let t = normalize(t);

        [t[0], t[1], t[2], 0.0]
    } else {
        let s = ((1.0 - cos_angle).abs() * 0.5).sqrt();
        let c = normalize(cross_vec);

        [
            c[0] * s,
            c[1] * s,
            c[2] * s,
            ((1.0 + cos_angle).abs() * 0.5).sqrt(),
        ]
    }
}

/// GLSL `multVecQuat`.
fn mult_vec_quat(v: V3, q: [f32; 4]) -> V3 {
    let qv = [q[0], q[1], q[2]];
    let inner = add(cross(qv, v), scale(v, q[3]));

    add(v, scale(cross(qv, inner), 2.0))
}

// ---------------------------------------------------------------------------------------------
// The shader's LCG

/// `uint seed; srand; random` of the shader.
#[derive(Clone, Copy, Debug)]
pub struct Rng {
    seed: u32,
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        Rng { seed }
    }

    /// `random ()`: `seed = seed * 1103515245u + 12345u; float (seed) / 4294967295.0`. The divisor
    /// is a float32 literal and rounds to 2^32; `float (uint)` rounds to nearest even.
    pub fn random(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1103515245).wrapping_add(12345);
        self.seed as f32 / 4294967296.0f32
    }

    /// `getRandomValue (min, max)`.
    pub fn value(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.random() * (hi - lo)
    }
}

// ---------------------------------------------------------------------------------------------
// Math.random for the per-frame `randomSeed` uniform

/// Where the `randomSeed` of each emitter pass comes from. X_ITE draws `Math.random () * 0xffffffff`
/// once per `animate`; tests inject the recorded values of an X_ITE run (see [`set_seed_list`]).
#[derive(Debug)]
pub enum SeedSource {
    /// xorshift128+ (the generator of V8's `Math.random`), seeded from the clock at first use.
    Random { s0: u64, s1: u64 },
    /// The values to hand out next, in order: the doubles passed to `uniform1i (randomSeed)`.
    List(VecDeque<f64>),
}

thread_local! {
    static SEEDS: RefCell<Option<SeedSource>> = const { RefCell::new(None) };
    static SEED_LIST_EXHAUSTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;

    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn fresh_random() -> SeedSource {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    let local = 0u8;
    let mut state = nanos ^ ((&local as *const u8 as u64) << 17) ^ u64::from(std::process::id());
    let (s0, s1) = (splitmix64(&mut state), splitmix64(&mut state));

    SeedSource::Random { s0, s1 }
}

/// Use `seeds` (the raw `randomSeed` uniform values, in order) for the next emitter passes of this
/// thread, instead of a random source.
pub fn set_seed_list(seeds: Vec<f64>) {
    SEED_LIST_EXHAUSTED.with(|e| e.set(false));
    SEEDS.with(|s| *s.borrow_mut() = Some(SeedSource::List(seeds.into())));
}

/// Back to a random source (the default).
pub fn use_random_seeds() {
    SEEDS.with(|s| *s.borrow_mut() = None);
}

/// A seed list ran dry and a random value was used instead (since [`set_seed_list`]).
pub fn seed_list_exhausted() -> bool {
    SEED_LIST_EXHAUSTED.with(|e| e.get())
}

fn random_double(s0: &mut u64, s1: &mut u64) -> f64 {
    let mut x = *s0;
    let y = *s1;

    *s0 = y;
    x ^= x << 23;
    x ^= x >> 17;
    x ^= y ^ (y >> 26);
    *s1 = x;

    ((x.wrapping_add(y)) >> 11) as f64 / (1u64 << 53) as f64
}

/// `Math .random () * 0xffffffff`: the value X_ITE hands to `gl .uniform1i (randomSeed, …)`.
pub fn next_seed() -> f64 {
    SEEDS.with(|cell| {
        let mut cell = cell.borrow_mut();

        if let Some(SeedSource::List(list)) = cell.as_mut() {
            if let Some(v) = list.pop_front() {
                return v;
            }

            SEED_LIST_EXHAUSTED.with(|e| e.set(true));
            *cell = Some(fresh_random());
        }

        let source = cell.get_or_insert_with(fresh_random);

        match source {
            SeedSource::Random { s0, s1 } => random_double(s0, s1) * 4294967295.0,
            SeedSource::List(_) => 0.0,
        }
    })
}

thread_local! {
    /// The `Math.random ()` values the physics models draw on the CPU, in order (see [`set_force_draws`]).
    static FORCE_DRAWS: RefCell<Option<VecDeque<f64>>> = const { RefCell::new(None) };
    static FORCE_DRAWS_EXHAUSTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FORCE_RANDOM: RefCell<Option<SeedSource>> = const { RefCell::new(None) };
}

/// Use `draws` for the `Math.random ()` calls of `WindPhysicsModel.addForce` (`getRandomValue`) of this thread,
/// in the order X_ITE made them: per frame, the draws of every enabled wind model; the one `randomSeed` draw
/// of the emitter pass that follows is not part of this list, it comes from [`set_seed_list`].
pub fn set_force_draws(draws: Vec<f64>) {
    FORCE_DRAWS_EXHAUSTED.with(|e| e.set(false));
    FORCE_DRAWS.with(|s| *s.borrow_mut() = Some(draws.into()));
}

/// Back to a random source for the physics models (the default).
pub fn use_random_force_draws() {
    FORCE_DRAWS.with(|s| *s.borrow_mut() = None);
}

/// A force draw list ran dry and a random value was used instead (since [`set_force_draws`]).
pub fn force_draws_exhausted() -> bool {
    FORCE_DRAWS_EXHAUSTED.with(|e| e.get())
}

/// Recorded force draws that nothing consumed (yet).
pub fn force_draws_left() -> usize {
    FORCE_DRAWS.with(|s| s.borrow().as_ref().map_or(0, VecDeque::len))
}

/// `Math .random ()` for the CPU side of the physics models (`ParticleSystem.animateParticles` calling
/// `addForce`): the next recorded value, else the generator of V8.
pub fn next_force_random() -> f64 {
    let recorded = FORCE_DRAWS.with(|s| s.borrow_mut().as_mut().map(VecDeque::pop_front));

    match recorded {
        Some(Some(v)) => return v,
        Some(None) => {
            FORCE_DRAWS_EXHAUSTED.with(|e| e.set(true));
            FORCE_DRAWS.with(|s| *s.borrow_mut() = None);
        }
        None => {}
    }

    FORCE_RANDOM.with(
        |cell| match cell.borrow_mut().get_or_insert_with(fresh_random) {
            SeedSource::Random { s0, s1 } => random_double(s0, s1),
            SeedSource::List(_) => 0.0,
        },
    )
}

/// WebGL `uniform1i` of a double: ToInt32, as the shader's `int randomSeed`.
pub fn seed_uniform(value: f64) -> u32 {
    jsmath::to_int32(value) as u32
}

// ---------------------------------------------------------------------------------------------
// Emitter parameters

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmitterKind {
    Point,
    Cone,
    Explosion,
    Surface,
    Volume,
    Polyline,
}

/// The uniforms of an emitter program (after the JavaScript clamping and normalization).
#[derive(Clone, Debug)]
pub struct EmitterParams {
    pub kind: EmitterKind,
    /// `emitter .on`: with `createParticles` it selects the program with `X3D_CREATE_PARTICLES`.
    pub on: bool,
    pub position: V3,
    /// The `direction` uniform: PointEmitter and VolumeEmitter normalize it in JavaScript (double),
    /// ConeEmitter uploads it unchanged.
    pub direction: V3,
    pub angle: f32,
    /// `max (speed, 0)` and `max (variation, 0)` as float uniforms.
    pub speed: f32,
    pub variation: f32,
    /// `max (mass, 0)` (JavaScript double).
    pub mass: f64,
    /// `max (surfaceArea, 0)` (JavaScript double): the area WindPhysicsModel multiplies its pressure with.
    pub surface_area: f64,
    /// The `solid` uniform of a SurfaceEmitter (`surfaceNode ._solid`, true without geometry).
    pub solid: bool,
}

impl EmitterParams {
    /// The defaults of `PointEmitter`, which is X_ITE's default emitter (`getDefaultEmitter`).
    pub fn default_point() -> Self {
        EmitterParams {
            kind: EmitterKind::Point,
            on: true,
            position: [0.0; 3],
            direction: [0.0, 1.0, 0.0],
            angle: 0.785398,
            speed: 0.0,
            variation: 0.25,
            mass: 0.0,
            surface_area: 0.0,
            solid: true,
        }
    }

    pub fn is_explosive(&self) -> bool {
        self.kind == EmitterKind::Explosion
    }
}

// ---------------------------------------------------------------------------------------------
// Geometry textures (SurfaceEmitter, VolumeEmitter, ParticleSystem bounded physics)

/// The RGBA32F texture X_ITE samples with `texelFetch`: areas (surface, volume), vertices,
/// normals and the BVH (volume, bounded).
#[derive(Clone, Debug, Default)]
pub struct GeometryTexture {
    pub array: Vec<[f32; 4]>,
    pub vertices_index: i32,
    pub normals_index: i32,
    pub hierarchy_index: i32,
    /// Texel of the root pointer, -1 if there is no hierarchy.
    pub hierarchy_root: i32,
}

impl GeometryTexture {
    /// `texelFetch (sampler, index, 0)`; texels outside the data read as zero.
    fn fetch(&self, index: i64) -> [f32; 4] {
        if index < 0 {
            return [0.0; 4];
        }

        self.array.get(index as usize).copied().unwrap_or([0.0; 4])
    }

    fn fetch3(&self, index: i64) -> V3 {
        let t = self.fetch(index);

        [t[0], t[1], t[2]]
    }
}

/// `Triangle3 .area`: `|(b - a) x (c - a)| / 2` in doubles (`Math .hypot` for the norm).
fn triangle_area(vertices: &[f32], i: usize) -> f64 {
    let v = |k: usize| {
        [
            vertices[i + k] as f64,
            vertices[i + k + 1] as f64,
            vertices[i + k + 2] as f64,
        ]
    };
    let (a, b, c) = (v(0), v(4), v(8));
    let (b, c) = (
        [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
        [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
    );
    let x = [
        b[1] * c[2] - b[2] * c[1],
        b[2] * c[0] - b[0] * c[2],
        b[0] * c[1] - b[1] * c[0],
    ];

    jsmath::hypot(&x) / 2.0
}

fn area_texels(vertices: &[f32], num_area_so_far: usize) -> Vec<[f32; 4]> {
    let mut array = vec![[0.0f32; 4]; num_area_so_far];
    let mut area_so_far = 0.0f64;

    for i in (0..vertices.len()).step_by(12) {
        if i + 12 > vertices.len() {
            break;
        }

        area_so_far += triangle_area(vertices, i);
        array[i / 12 + 1][0] = area_so_far as f32;
    }

    array
}

fn push_vertices_and_normals(array: &mut Vec<[f32; 4]>, vertices: &[f32], normals: &[f32]) {
    for v in vertices.chunks_exact(4) {
        array.push([v[0], v[1], v[2], v[3]]);
    }

    for n in normals.chunks_exact(3) {
        array.push([n[0], n[1], n[2], 0.0]);
    }
}

/// `SurfaceEmitter.set_geometry__` for a geometry with these `getVertices ()` (vec4 per vertex, 3
/// per triangle) and `getNormals ()` (vec3 per vertex).
pub fn surface_texture(vertices: &[f32], normals: &[f32]) -> GeometryTexture {
    let num_vertices = vertices.len() / 4;
    let num_area_so_far = num_vertices / 3 + 1;
    let vertices_index = num_area_so_far;
    let normals_index = vertices_index + num_vertices;
    let mut array = area_texels(vertices, num_area_so_far);

    push_vertices_and_normals(&mut array, vertices, normals);

    GeometryTexture {
        array,
        vertices_index: if num_vertices > 0 {
            vertices_index as i32
        } else {
            -1
        },
        normals_index: if num_vertices > 0 {
            normals_index as i32
        } else {
            -1
        },
        hierarchy_index: -1,
        hierarchy_root: -1,
    }
}

/// `VolumeEmitter.set_geometry__`: areas, vertices, normals and the BVH of the private
/// IndexedFaceSet.
pub fn volume_texture(vertices: &[f32], normals: &[f32]) -> GeometryTexture {
    let num_vertices = vertices.len() / 4;
    let num_normals = normals.len() / 3;
    let num_area_so_far = num_vertices / 3 + 1;
    let hierarchy = build_hierarchy(vertices);
    let vertices_index = num_area_so_far;
    let normals_index = vertices_index + num_vertices;
    let hierarchy_index = normals_index + num_normals;
    let mut array = area_texels(vertices, num_area_so_far);

    push_vertices_and_normals(&mut array, vertices, normals);

    let hierarchy_root = hierarchy_index as i32 + hierarchy.len() as i32 - 1;

    array.extend(hierarchy);

    GeometryTexture {
        array,
        vertices_index: vertices_index as i32,
        normals_index: normals_index as i32,
        hierarchy_index: hierarchy_index as i32,
        hierarchy_root,
    }
}

/// `PolylineEmitter.set_polylines__`: the `polylines` texture. Texel 0 is 0, texels 1 .. n the running length
/// (`lengthSoFar`, x) at the end of each segment, then the segment end points (two RGBA texels per segment).
/// `vertices` are the x y z w of `IndexedLineSet .getVertices ()` (JavaScript doubles), two vertices per
/// segment. `vertices_index` is the number of length texels, or -1 without vertices.
pub fn polyline_texture(vertices: &[f64]) -> GeometryTexture {
    let num_vertices = vertices.len() / 4;
    let num_length_so_far = num_vertices / 2 + 1;
    let size = ((num_length_so_far + num_vertices) as f64).sqrt().ceil() as usize;
    let mut array = vec![[0.0f32; 4]; (size * size).max(num_length_so_far + num_vertices)];
    let mut length_so_far = 0.0f64;

    for (segment, pair) in vertices.chunks_exact(8).enumerate() {
        // `vertex2 .subtract (vertex1) .norm ()`, a Math.hypot.
        length_so_far += jsmath::hypot(&[pair[4] - pair[0], pair[5] - pair[1], pair[6] - pair[2]]);
        array[segment + 1][0] = length_so_far as f32;
    }

    for (k, v) in vertices.chunks_exact(4).enumerate() {
        array[num_length_so_far + k] = [v[0] as f32, v[1] as f32, v[2] as f32, v[3] as f32];
    }

    GeometryTexture {
        array,
        vertices_index: if num_vertices > 0 {
            num_length_so_far as i32
        } else {
            -1
        },
        normals_index: -1,
        hierarchy_index: -1,
        hierarchy_root: -1,
    }
}

/// `ParticleSystem.set_boundedPhysics__`: the triangle soup of the enabled bounded models (normals
/// already multiplied by `damping`), vertices first, then normals, then the BVH.
pub fn bounded_texture(vertices: &[f32], normals: &[f32]) -> GeometryTexture {
    let num_vertices = vertices.len() / 4;
    let num_normals = normals.len() / 3;
    let hierarchy = build_hierarchy(vertices);
    let normals_index = num_vertices;
    let hierarchy_index = normals_index + num_normals;
    let mut array = Vec::with_capacity(num_vertices + num_normals + hierarchy.len());

    push_vertices_and_normals(&mut array, vertices, normals);

    let hierarchy_root = hierarchy_index as i32 + hierarchy.len() as i32 - 1;

    array.extend(hierarchy);

    GeometryTexture {
        array,
        vertices_index: 0,
        normals_index: normals_index as i32,
        hierarchy_index: hierarchy_index as i32,
        hierarchy_root,
    }
}

// --- BVH.js ---------------------------------------------------------------------------------

enum Bvh {
    Triangle(usize),
    Node {
        min: [f64; 3],
        max: [f64; 3],
        left: Box<Bvh>,
        right: Box<Bvh>,
    },
}

/// `SortComparator (vertices, axis)`. The comparator indexes the vertex array with the TRIANGLE
/// NUMBER as if it were an offset (`vertices [a + axis]`, a quirk of the original: it should be
/// `a * 12`), and the `axis` it reads is the constructor argument 0 (the `compare .axis = …`
/// assignments of `Node` set a property that the closure never reads). Reading past the array
/// gives NaN, which makes every comparison false.
fn sort_key(vertices: &[f32], a: usize) -> f64 {
    let at = |i: usize| vertices.get(i).map_or(f64::NAN, |&v| v as f64);
    let (x, y, z) = (at(a), at(a + 4), at(a + 8));

    if x.is_nan() || y.is_nan() || z.is_nan() {
        return f64::NAN;
    }

    x.min(y).min(z)
}

/// QuickSort.js over the shared triangle array (the comparison sees elements outside [lo, hi]
/// too, `undefined` elements compare false).
fn quicksort(array: &mut [usize], vertices: &[f32], lo: i64, hi: i64) {
    let (mut i, mut j) = (lo, hi);
    let at = |array: &[usize], k: i64| {
        if k >= 0 {
            array.get(k as usize).copied()
        } else {
            None
        }
    };
    let less = |a: Option<usize>, b: Option<usize>| match (a, b) {
        (Some(a), Some(b)) => sort_key(vertices, a) < sort_key(vertices, b),
        _ => false,
    };
    let x = at(array, (lo + hi) >> 1);

    loop {
        while less(at(array, i), x) {
            i += 1;
        }

        while less(x, at(array, j)) {
            j -= 1;
        }

        if i < j {
            array.swap(i as usize, j as usize);
            i += 1;
            j -= 1;
        } else {
            if i == j {
                i += 1;
                j -= 1;
            }

            break;
        }
    }

    if lo < j {
        quicksort(array, vertices, lo, j);
    }

    if i < hi {
        quicksort(array, vertices, i, hi);
    }
}

fn bvh_node(vertices: &[f32], triangles: &mut [usize], first: usize, size: usize) -> Bvh {
    let vertex = |t: usize, k: usize| {
        [
            vertices[t * 12 + k * 4] as f64,
            vertices[t * 12 + k * 4 + 1] as f64,
            vertices[t * 12 + k * 4 + 2] as f64,
        ]
    };
    let first_vertex = vertex(triangles[first], 0);
    let (mut min, mut max) = (first_vertex, first_vertex);

    for &t in &triangles[first..first + size] {
        for k in 0..3 {
            let v = vertex(t, k);

            for c in 0..3 {
                min[c] = min[c].min(v[c]);
                max[c] = max[c].max(v[c]);
            }
        }
    }

    // Node.getLongestAxis would choose the axis here, but the comparator ignores it (see sort_key).
    let left_size = if size > 2 {
        quicksort(triangles, vertices, first as i64, (first + size) as i64 - 1);
        size >> 1
    } else {
        1
    };

    let right_size = size - left_size;
    let left = if left_size > 1 {
        bvh_node(vertices, triangles, first, left_size)
    } else {
        Bvh::Triangle(triangles[first])
    };
    let right = if right_size > 1 {
        bvh_node(vertices, triangles, first + left_size, right_size)
    } else {
        Bvh::Triangle(triangles[first + left_size])
    };

    Bvh::Node {
        min,
        max,
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn bvh_to_array(node: &Bvh, out: &mut Vec<[f32; 4]>) -> usize {
    match node {
        Bvh::Triangle(t) => {
            let index = out.len();

            out.push([1.0, (t * 3) as f32, 0.0, 0.0]);
            index
        }
        Bvh::Node {
            min,
            max,
            left,
            right,
        } => {
            let l = bvh_to_array(left, out);
            let r = bvh_to_array(right, out);
            let index = out.len();

            out.push([0.0, l as f32, r as f32, 0.0]);
            out.push([min[0] as f32, min[1] as f32, min[2] as f32, 0.0]);
            out.push([max[0] as f32, max[1] as f32, max[2] as f32, 0.0]);
            index
        }
    }
}

/// `new BVH (vertices, normals) .toArray ([ ])`: the hierarchy texels, ending with the root
/// pointer. Empty for an empty soup.
pub fn build_hierarchy(vertices: &[f32]) -> Vec<[f32; 4]> {
    let n = vertices.len() / 12;
    let mut out = vec![];

    match n {
        0 => {}
        1 => {
            let root = bvh_to_array(&Bvh::Triangle(0), &mut out);

            out.push([root as f32, 0.0, 0.0, 0.0]);
        }
        _ => {
            let mut triangles: Vec<usize> = (0..n).collect();
            let root = bvh_node(vertices, &mut triangles, 0, n);
            let root = bvh_to_array(&root, &mut out);

            out.push([root as f32, 0.0, 0.0, 0.0]);
        }
    }

    out
}

// --- Line3 / Plane3 / Box3 / BVH traversal (GLSL) ---------------------------------------------

#[derive(Clone, Copy)]
struct Line {
    point: V3,
    direction: V3,
}

/// `intersects (line, a, b, c, r)` (Moller-Trumbore, non-culling): `r = (u, v, 1 - u - v)`.
fn intersects_triangle(line: Line, a: V3, b: V3, c: V3) -> Option<V3> {
    let edge1 = sub(b, a);
    let edge2 = sub(c, a);
    let pvec = cross(line.direction, edge2);
    let det = dot(edge1, pvec);

    if det == 0.0 {
        return None;
    }

    let inv_det = 1.0 / det;
    let tvec = sub(line.point, a);
    let u = dot(tvec, pvec) * inv_det;

    if u < 0.0 || u > 1.0 {
        return None;
    }

    let qvec = cross(tvec, edge1);
    let v = dot(line.direction, qvec) * inv_det;

    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    Some([u, v, 1.0 - u - v])
}

/// `intersects (plane3 (point, normal), line, intersection)`.
fn intersects_plane(point: V3, normal: V3, line: Line) -> Option<V3> {
    let theta = dot(line.direction, normal);

    if theta == 0.0 {
        return None;
    }

    let distance_from_origin = dot(normal, point);
    let t = (distance_from_origin - dot(normal, line.point)) / theta;

    Some(add(line.point, scale(line.direction, t)))
}

/// `intersects (min, max, line)` of Box3.glsl.js: five of the six faces (there is no "left").
fn intersects_box(min: V3, max: V3, line: Line) -> bool {
    let faces: [(V3, V3, usize, usize); 5] = [
        (max, [0.0, 0.0, 1.0], 0, 1),
        (min, [0.0, 0.0, -1.0], 0, 1),
        (max, [0.0, 1.0, 0.0], 0, 2),
        (min, [0.0, -1.0, 0.0], 0, 2),
        (max, [1.0, 0.0, 0.0], 1, 2),
    ];

    for (point, normal, a, b) in faces {
        if let Some(x) = intersects_plane(point, normal, line) {
            if x[a] >= min[a] && x[b] >= min[b] && max[a] >= x[a] && max[b] >= x[b] {
                return true;
            }
        }
    }

    false
}

/// `getIntersections`: the infinite `line` against every triangle, in BVH traversal order, and the number of
/// hits. With `want_normals` the interpolated normal of each hit is returned too.
///
/// Both GLSL versions write `points [count]` (and the bounded one `normals [count]`) without checking
/// ARRAY_SIZE; WebGL clamps the index, so every hit after the 31st lands in the last entry, the last one in
/// traversal order stays there, and the count keeps going. Measured against X_ITE on the GPU for the bounded
/// version (BoundedPhysicsModel `bounce`: oracle/synthetic-rea-b16-bounded bp-hits-64-below, bp-hits-64-floor-last;
/// `min_index` takes the first of equal distances, so the entries past ARRAY_SIZE it reads add nothing) and for
/// the volume version (VolumeEmitter `getRandomPosition`: oracle/synthetic-rea-b17-emitter-hits, which sorts and
/// picks among the whole count).
fn get_intersections(
    tex: &GeometryTexture,
    line: Line,
    want_normals: bool,
) -> (Vec<[f32; 4]>, Vec<V3>, usize) {
    let hierarchy_index = tex.hierarchy_index as i64;
    let mut points = vec![];
    let mut normals = vec![];
    let mut count = 0;
    let mut stack: Vec<i64> = Vec::with_capacity(BVH_STACK_SIZE);
    let mut current = tex.fetch(tex.hierarchy_root as i64)[0] as i32 as i64 + hierarchy_index;

    while !stack.is_empty() || current >= 0 {
        if current >= 0 {
            let node = tex.fetch(current);

            if node[0] as i32 == 0 {
                if intersects_box(tex.fetch3(current + 1), tex.fetch3(current + 2), line) {
                    if stack.len() < BVH_STACK_SIZE {
                        stack.push(current);
                    }

                    current = node[1] as i32 as i64 + hierarchy_index;
                } else {
                    current = -1;
                }
            } else {
                let t = node[1] as i32 as i64;
                let v = tex.vertices_index as i64 + t;
                let (a, b, c) = (tex.fetch3(v), tex.fetch3(v + 1), tex.fetch3(v + 2));

                if let Some(r) = intersects_triangle(line, a, b, c) {
                    let p = add(add(scale(a, r[2]), scale(b, r[0])), scale(c, r[1]));
                    let point = [p[0], p[1], p[2], 1.0];
                    let normal = want_normals.then(|| {
                        let n = tex.normals_index as i64 + t;
                        let (n0, n1, n2) = (tex.fetch3(n), tex.fetch3(n + 1), tex.fetch3(n + 2));

                        add(add(scale(n0, r[2]), scale(n1, r[0])), scale(n2, r[1]))
                    });

                    count += 1;

                    if points.len() < ARRAY_SIZE {
                        points.push(point);
                        normals.extend(normal);
                    } else {
                        points[ARRAY_SIZE - 1] = point;

                        if let Some(normal) = normal {
                            normals[ARRAY_SIZE - 1] = normal;
                        }
                    }
                }

                current = -1;
            }
        } else {
            let top = stack.pop().unwrap_or(0);

            current = tex.fetch(top)[2] as i32 as i64 + hierarchy_index;
        }
    }

    (points, normals, count)
}

/// The index WebGL accesses for `points [i]` of an `ARRAY_SIZE` array: out-of-range indices are clamped.
fn clamped(i: usize) -> usize {
    i.min(ARRAY_SIZE - 1)
}

/// `sort (points, count, plane3 (line .point, line .direction))`: the comb sort by plane distance. A `count` past
/// ARRAY_SIZE reads and swaps the clamped entries.
fn comb_sort(points: &mut [[f32; 4]], count: usize, line: Line) {
    let distance =
        |p: &[f32; 4]| dot([p[0], p[1], p[2]], line.direction) - dot(line.direction, line.point);
    let count = count as i32;
    let shrink = 1.0f32 / 1.3f32;
    let mut gap = count;
    let mut exchanged = true;

    while exchanged {
        gap = (gap as f32 * shrink) as i32;

        if gap <= 1 {
            exchanged = false;
            gap = 1;
        }

        for i in 0..count - gap {
            let (i, j) = (clamped(i as usize), clamped((gap + i) as usize));

            if distance(&points[i]) > distance(&points[j]) {
                points.swap(i, j);
                exchanged = true;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Emitter sampling

/// GLSL `upperBound (sampler, count, value)` on the x of the texels from 0.
fn upper_bound(tex: &GeometryTexture, mut count: i64, value: f32) -> i64 {
    let mut first = 0i64;

    while count > 0 {
        let step = count >> 1;
        let index = first + step;

        if value < tex.fetch(index)[0] {
            count = step;
        } else {
            first = index + 1;
            count -= step + 1;
        }
    }

    first
}

/// GLSL `interpolate (sampler, count, fraction, index0, index1, weight)`.
fn interpolate(tex: &GeometryTexture, count: i64, fraction: f32) -> (i64, i64, f32) {
    if count == 1 || fraction <= tex.fetch(0)[0] {
        (0, 0, 0.0)
    } else if fraction >= tex.fetch(count - 1)[0] {
        (count - 2, count - 1, 1.0)
    } else {
        let index = upper_bound(tex, count, fraction);

        if index < count {
            let (i1, i0) = (index, index - 1);
            let (key0, key1) = (tex.fetch(i0)[0], tex.fetch(i1)[0]);
            let weight = ((fraction - key0) / (key1 - key0)).clamp(0.0, 1.0);

            (i0, i1, weight)
        } else {
            (0, 0, 0.0)
        }
    }
}

struct Shader<'a> {
    rng: Rng,
    e: &'a EmitterParams,
    particle_lifetime: f32,
    lifetime_variation: f32,
}

impl Shader<'_> {
    fn random_lifetime(&mut self) -> f32 {
        let v = self.particle_lifetime * self.lifetime_variation;
        let lo = 0.0f32.max(self.particle_lifetime - v);

        self.rng.value(lo, self.particle_lifetime + v)
    }

    fn random_speed(&mut self) -> f32 {
        let v = self.e.speed * self.e.variation;
        let lo = 0.0f32.max(self.e.speed - v);

        self.rng.value(lo, self.e.speed + v)
    }

    fn random_normal(&mut self) -> V3 {
        let theta = self.rng.value(-M_PI, M_PI);
        let cphi = self.rng.value(-1.0, 1.0);
        let r = (1.0 - cphi * cphi).sqrt();

        [theta.sin() * r, theta.cos() * r, cphi]
    }

    fn random_normal_with_angle(&mut self, angle: f32) -> V3 {
        let theta = self.rng.value(-M_PI, M_PI);
        let cphi = self.rng.value(angle.cos(), 1.0);
        let r = (1.0 - cphi * cphi).sqrt();

        [theta.sin() * r, theta.cos() * r, cphi]
    }

    fn random_normal_with_direction_and_angle(&mut self, direction: V3, angle: f32) -> V3 {
        let rotation = quaternion([0.0, 0.0, 1.0], direction);
        let normal = self.random_normal_with_angle(angle);

        mult_vec_quat(normal, rotation)
    }

    fn random_surface_normal(&mut self, direction: V3) -> V3 {
        let theta = self.rng.value(-M_PI, M_PI);
        let cphi = self.rng.random().powf(1.0f32 / 3.0f32);
        let r = (1.0 - cphi * cphi).sqrt();
        let normal = [theta.sin() * r, theta.cos() * r, cphi];
        let rotation = quaternion([0.0, 0.0, 1.0], direction);

        mult_vec_quat(normal, rotation)
    }

    fn random_spherical_velocity(&mut self) -> V3 {
        let normal = self.random_normal();
        let speed = self.random_speed();

        scale(normal, speed)
    }

    /// `getRandomPointOnSurface`: draw 1 chooses the triangle by area, draws 2 and 3 are the
    /// barycentric coordinates.
    fn random_point_on_surface(&mut self, tex: &GeometryTexture) -> ([f32; 4], V3) {
        let vi = tex.vertices_index as i64;
        let last_area_so_far = tex.fetch(vi - 1)[0];
        let fraction = self.rng.random() * last_area_so_far;
        let count = vi;
        let index0: i64;

        if count == 1 || fraction <= tex.fetch(0)[0] {
            index0 = 0;
        } else if fraction >= tex.fetch(count - 1)[0] {
            index0 = count - 2;
        } else {
            // upperBound
            let (mut first, mut c) = (0i64, count);

            while c > 0 {
                let step = c >> 1;
                let index = first + step;

                if fraction < tex.fetch(index)[0] {
                    c = step;
                } else {
                    first = index + 1;
                    c -= step + 1;
                }
            }

            index0 = if first < count { first - 1 } else { 0 };
        }

        let index0 = index0 * 3;
        let ni = tex.normals_index as i64;
        let (v0, v1, v2) = (
            tex.fetch(vi + index0),
            tex.fetch(vi + index0 + 1),
            tex.fetch(vi + index0 + 2),
        );
        let (n0, n1, n2) = (
            tex.fetch3(ni + index0),
            tex.fetch3(ni + index0 + 1),
            tex.fetch3(ni + index0 + 2),
        );

        // getRandomBarycentricCoord
        let mut u = self.rng.random();
        let mut v = self.rng.random();

        if u + v > 1.0 {
            u = 1.0 - u;
            v = 1.0 - v;
        }

        let t = 1.0 - u - v;
        let r = [t, u, v];
        let mut position = [0.0f32; 4];

        for c in 0..4 {
            position[c] = r[2] * v0[c] + r[0] * v1[c] + r[1] * v2[c];
        }

        let normal = normalize(add(add(scale(n0, r[2]), scale(n1, r[0])), scale(n2, r[1])));

        (position, normal)
    }

    /// `getRandomVelocity ()` and `getRandomPosition ()` of the emitter, in the order of
    /// `output2 = …; output6 = …`.
    fn velocity_and_position(&mut self, texture: Option<&GeometryTexture>) -> (V3, [f32; 4]) {
        let e = self.e;
        let pos4 = [e.position[0], e.position[1], e.position[2], 1.0];

        match e.kind {
            EmitterKind::Point => {
                let v = if e.direction == [0.0; 3] {
                    self.random_spherical_velocity()
                } else {
                    scale(e.direction, self.random_speed())
                };

                (v, pos4)
            }
            EmitterKind::Cone => {
                let v = if e.direction == [0.0; 3] {
                    self.random_spherical_velocity()
                } else {
                    let normal = self.random_normal_with_direction_and_angle(e.direction, e.angle);
                    let speed = self.random_speed();

                    scale(normal, speed)
                };

                (v, pos4)
            }
            EmitterKind::Explosion => (self.random_spherical_velocity(), pos4),
            EmitterKind::Polyline => {
                let v = if e.direction == [0.0; 3] {
                    self.random_spherical_velocity()
                } else {
                    scale(e.direction, self.random_speed())
                };
                let position = match texture.filter(|t| t.vertices_index >= 0) {
                    Some(tex) => self.random_polyline_position(tex),
                    None => [f32::NAN; 4],
                };

                (v, position)
            }
            EmitterKind::Surface => {
                let Some(tex) = texture.filter(|t| t.vertices_index >= 0) else {
                    return ([0.0; 3], [f32::NAN; 4]);
                };
                let (position, mut normal) = self.random_point_on_surface(tex);

                if !e.solid && self.rng.random() > 0.5 {
                    normal = [-normal[0], -normal[1], -normal[2]];
                }

                (scale(normal, self.random_speed()), position)
            }
            EmitterKind::Volume => {
                let Some(tex) = texture.filter(|t| t.hierarchy_root >= 0) else {
                    return ([0.0; 3], [f32::NAN; 4]);
                };
                let velocity = if e.direction == [0.0; 3] {
                    self.random_spherical_velocity()
                } else {
                    scale(e.direction, self.random_speed())
                };
                let position = self.random_volume_position(tex);

                (velocity, position)
            }
        }
    }

    /// PolylineEmitter `getRandomPosition ()`: one draw picks the length along all polylines, `interpolate`
    /// finds the segment, `mix` the point.
    fn random_polyline_position(&mut self, tex: &GeometryTexture) -> [f32; 4] {
        let vi = tex.vertices_index as i64;
        let last_length_so_far = tex.fetch(vi - 1)[0];
        let fraction = self.rng.random() * last_length_so_far;
        let (index0, index1, weight) = interpolate(tex, vi, fraction);
        let (v0, v1) = (tex.fetch(vi + index0 * 2), tex.fetch(vi + index0 * 2 + 1));
        let _ = index1;
        let mut out = [0.0f32; 4];

        for c in 0..4 {
            out[c] = v0[c] * (1.0 - weight) + v1[c] * weight;
        }

        out
    }

    /// VolumeEmitter `getRandomPosition ()`.
    fn random_volume_position(&mut self, tex: &GeometryTexture) -> [f32; 4] {
        let (point, normal) = self.random_point_on_surface(tex);
        let line = Line {
            point: [point[0], point[1], point[2]],
            direction: self.random_surface_normal(normal),
        };
        let (mut points, _, count) = get_intersections(tex, line, false);

        // An even count is needed: the last point in traversal order is dropped when odd.
        let n = count - count % 2;

        match n {
            0 => return [0.0; 4],
            2 => {}
            _ => comb_sort(&mut points, n, line),
        }

        let index = ((self.rng.random() % 1.0) * (n / 2) as f32) as usize * 2;
        let a = self.rng.random();
        let (p, q) = (points[clamped(index)], points[clamped(index + 1)]);
        let mut out = [0.0f32; 4];

        for c in 0..4 {
            out[c] = p[c] * (1.0 - a) + q[c] * a;
        }

        out
    }
}

/// `bounce (deltaTime, fromPosition, toPosition, velocity)` of the BoundedPhysicsModel soup.
fn bounce(tex: &GeometryTexture, from: [f32; 4], to: &mut [f32; 4], velocity: &mut V3) {
    let line = Line {
        point: [from[0], from[1], from[2]],
        direction: normalize(*velocity),
    };
    let (points, normals, _) = get_intersections(tex, line, true);

    if points.is_empty() {
        return;
    }

    // min_index (points, count, 0.0, plane3 (line .point, line .direction))
    let d0 = dot(line.direction, line.point);
    let (mut index, mut dist) = (usize::MAX, 1000000.0f32);

    for (i, p) in points.iter().enumerate() {
        let d = dot([p[0], p[1], p[2]], line.direction) - d0;

        if d >= 0.0 && d < dist {
            dist = d;
            index = i;
        }
    }

    if index == usize::MAX {
        return;
    }

    let point = [points[index][0], points[index][1], points[index][2]];
    let nrm = normals[index];
    let normal = normalize(nrm);
    let plane_d = dot(normal, point);
    let side = |p: V3| sign(dot(p, normal) - plane_d);

    if side([from[0], from[1], from[2]]) == side([to[0], to[1], to[2]]) {
        return;
    }

    let damping = dot(nrm, nrm).sqrt();
    let reflected = sub(*velocity, scale(normal, 2.0 * dot(normal, *velocity)));
    let p = add(point, scale(normalize(reflected), 0.0001));

    *velocity = scale(reflected, damping);
    *to = [p[0], p[1], p[2], 1.0];
}

// ---------------------------------------------------------------------------------------------
// The ParticleSystem's geometry type and ramps

/// `Browser/Shape/GeometryType.js`: the `X3D_GEOMETRY_TYPE` of the emitter shader and the
/// ParticleSystem's `geometryType` field (an unknown name is QUAD).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GeometryType {
    Point,
    Line,
    Triangle,
    #[default]
    Quad,
    Sprite,
    Geometry,
}

impl GeometryType {
    /// `GeometryType .get (this ._geometryType .getValue ()) ?? GeometryType .QUAD`.
    pub fn from_name(name: &str) -> GeometryType {
        match name {
            "POINT" => GeometryType::Point,
            "LINE" => GeometryType::Line,
            "TRIANGLE" => GeometryType::Triangle,
            "SPRITE" => GeometryType::Sprite,
            "GEOMETRY" => GeometryType::Geometry,
            _ => GeometryType::Quad,
        }
    }

    /// `texCoordCount` of `set_geometryType__`: 4 for the quad types, 0 otherwise.
    pub fn tex_coord_count(self) -> usize {
        match self {
            GeometryType::Triangle | GeometryType::Quad | GeometryType::Sprite => 4,
            _ => 0,
        }
    }
}

/// One RGBA32F ramp texture of a ParticleSystem (`set_color__`, `set_texCoord__`, `set_scale__`): the
/// `count` keys in the x of texels `0 .. count`, the values from texel `count` on.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ramp {
    pub count: usize,
    pub texels: Vec<[f32; 4]>,
}

impl Ramp {
    /// `texelFetch`; texels outside the data read as zero.
    pub fn fetch(&self, index: i64) -> [f32; 4] {
        if index < 0 {
            return [0.0; 4];
        }

        self.texels.get(index as usize).copied().unwrap_or([0.0; 4])
    }

    /// GLSL `upperBound (sampler, count, value)`.
    fn upper_bound(&self, mut count: i64, value: f32) -> i64 {
        let mut first = 0i64;

        while count > 0 {
            let step = count >> 1;
            let index = first + step;

            if value < self.fetch(index)[0] {
                count = step;
            } else {
                first = index + 1;
                count -= step + 1;
            }
        }

        first
    }

    /// GLSL `interpolate (sampler, count, fraction, index0)` (the texCoord ramp: no weight).
    fn index0(&self, fraction: f32) -> i64 {
        let count = self.count as i64;

        if count == 1 || fraction <= self.fetch(0)[0] {
            0
        } else if fraction >= self.fetch(count - 1)[0] {
            count - 2
        } else {
            let index = self.upper_bound(count, fraction);

            if index < count { index - 1 } else { 0 }
        }
    }

    /// GLSL `interpolate (sampler, count, fraction, index0, index1, weight)`.
    fn weighted(&self, fraction: f32) -> (i64, i64, f32) {
        let count = self.count as i64;

        if count == 1 || fraction <= self.fetch(0)[0] {
            (0, 0, 0.0)
        } else if fraction >= self.fetch(count - 1)[0] {
            (count - 2, count - 1, 1.0)
        } else {
            let index = self.upper_bound(count, fraction);

            if index < count {
                let (key0, key1) = (self.fetch(index - 1)[0], self.fetch(index)[0]);

                (
                    index - 1,
                    index,
                    ((fraction - key0) / (key1 - key0)).clamp(0.0, 1.0),
                )
            } else {
                (0, 0, 0.0)
            }
        }
    }

    /// GLSL `mix (texelFetch (count + index0), texelFetch (count + index1), weight)`.
    fn mix(&self, fraction: f32) -> [f32; 4] {
        let (i0, i1, w) = self.weighted(fraction);
        let (a, b) = (
            self.fetch(self.count as i64 + i0),
            self.fetch(self.count as i64 + i1),
        );

        std::array::from_fn(|c| a[c] * (1.0 - w) + b[c] * w)
    }
}

/// The ramps of a ParticleSystem as its emitter shader reads them: `X3D_NUM_COLORS`,
/// `X3D_NUM_TEX_COORDS` (with `X3D_TEX_COORDS_COUNT`) and `X3D_NUM_SCALES` are the `count`s; a
/// missing ramp is a count of 0.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ramps {
    pub colors: Option<Ramp>,
    pub tex_coords: Option<Ramp>,
    pub tex_coords_count: usize,
    pub scales: Option<Ramp>,
}

impl Ramps {
    /// `getTexCoordIndex0 (fraction)`: `X3D_NUM_TEX_COORDS + index0 * X3D_TEX_COORDS_COUNT`, -1 without a ramp.
    fn tex_coord_index0(&self, fraction: f32) -> f32 {
        match &self.tex_coords {
            Some(r) => (r.count as i64 + r.index0(fraction) * self.tex_coords_count as i64) as f32,
            None => -1.0,
        }
    }

    /// `getColor (fraction)`: `vec4 (1.0)` without a ramp.
    fn color(&self, fraction: f32) -> [f32; 4] {
        self.colors.as_ref().map_or([1.0; 4], |r| r.mix(fraction))
    }

    /// `getScale (fraction)`: `vec3 (1.0)` without a ramp.
    fn scale(&self, fraction: f32) -> V3 {
        self.scales.as_ref().map_or([1.0; 3], |r| {
            let s = r.mix(fraction);

            [s[0], s[1], s[2]]
        })
    }
}

/// GLSL `Matrix3 (quaternion)`: the three columns.
fn quaternion_matrix(q: [f32; 4]) -> [V3; 3] {
    let [x, y, z, w] = q;
    let (a, b, c, d, e, f, g, h, i) = (
        y * y,
        z * z,
        x * y,
        z * w,
        z * x,
        y * w,
        x * x,
        y * z,
        x * w,
    );

    [
        [1.0 - 2.0 * (a + b), 2.0 * (c + d), 2.0 * (e - f)],
        [2.0 * (c - d), 1.0 - 2.0 * (b + g), 2.0 * (h + i)],
        [2.0 * (e + f), 2.0 * (h - i), 1.0 - 2.0 * (a + g)],
    ]
}

/// The end of the emitter shader's `main ()`: `output3` … `output5`, the scale / rotation matrix of the
/// geometry type, into `out[12..24]`. `velocity` is `output2 .xyz`.
fn write_matrix(
    out: &mut Particle,
    geometry_type: GeometryType,
    particle_size: [f32; 2],
    ramps: &Ramps,
    fraction: f32,
    velocity: V3,
) {
    let s = ramps.scale(fraction);
    let columns = match geometry_type {
        GeometryType::Point | GeometryType::Sprite | GeometryType::Geometry => {
            [[s[0], 0.0, 0.0], [0.0, s[1], 0.0], [0.0, 0.0, s[2]]]
        }
        GeometryType::Line => {
            let mut m = quaternion_matrix(quaternion([0.0, 0.0, 1.0], velocity));

            if ramps.scales.is_some() {
                // m *= mat3 (scale .x, …): column j times scale [j].
                for (column, s) in m.iter_mut().zip(s) {
                    *column = scale(*column, s);
                }
            }

            m
        }
        GeometryType::Triangle | GeometryType::Quad => {
            let size = [particle_size[0] * s[0], particle_size[1] * s[1]];

            [[size[0], 0.0, 0.0], [0.0, size[1], 0.0], [0.0, 0.0, s[2]]]
        }
    };

    for (k, column) in columns.iter().enumerate() {
        out[12 + k * 4..15 + k * 4].copy_from_slice(column);
        out[15 + k * 4] = 0.0;
    }
}

// ---------------------------------------------------------------------------------------------
// One emitter pass (the vertex shader over all particles)

/// The uniforms and samplers of one `emitterNode .animate (particleSystem, deltaTime)`.
pub struct Pass<'a> {
    pub emitter: &'a EmitterParams,
    /// `ps .createParticles && emitter .on` (the `X3D_CREATE_PARTICLES` define).
    pub create: bool,
    pub random_seed: u32,
    pub particle_lifetime: f32,
    pub lifetime_variation: f32,
    pub delta_time: f32,
    /// `forces` texture rows (x y z, w = turbulence).
    pub forces: &'a [[f32; 4]],
    /// SurfaceEmitter `surface` or VolumeEmitter `volume`.
    pub texture: Option<&'a GeometryTexture>,
    /// ParticleSystem bounded physics (only when its hierarchy root is > -1).
    pub bounded: Option<&'a GeometryTexture>,
    /// `X3D_GEOMETRY_TYPE`.
    pub geometry_type: GeometryType,
    /// The `particleSize` uniform.
    pub particle_size: [f32; 2],
    /// The color, texCoord and scale ramps.
    pub ramps: &'a Ramps,
}

/// The vertex shader `main ()` for particles `0 .. num`.
pub fn run_pass(particles: &mut [Particle], num: usize, pass: &Pass) {
    let mut shader = Shader {
        rng: Rng::new(0),
        e: pass.emitter,
        particle_lifetime: pass.particle_lifetime,
        lifetime_variation: pass.lifetime_variation,
    };

    for (id, p) in particles.iter_mut().enumerate().take(num) {
        // srand ((gl_VertexID + randomSeed) * randomSeed), int arithmetic wraps.
        shader.rng = Rng::new(
            (id as u32)
                .wrapping_add(pass.random_seed)
                .wrapping_mul(pass.random_seed),
        );

        let life = p[0] as i32;
        let lifetime = p[1];
        let elapsed = p[2] + pass.delta_time;
        let mut fraction = elapsed / lifetime;
        let mut out: Particle = [0.0; STRIDE];

        if elapsed > lifetime {
            // Create a new particle or hide the particle.
            let lifetime = shader.random_lifetime();

            fraction = 0.0;

            out[0] = life.wrapping_add(1).max(1) as f32;
            out[1] = lifetime;
            out[2] = 0.0;
            out[3] = pass.ramps.tex_coord_index0(0.0);

            if pass.create {
                out[4..8].copy_from_slice(&pass.ramps.color(0.0));

                let (velocity, position) = shader.velocity_and_position(pass.texture);

                out[8..11].copy_from_slice(&velocity);
                out[24..28].copy_from_slice(&position);
            } else {
                out[24..28].copy_from_slice(&[f32::NAN; 4]);
            }
        } else {
            let mut velocity = [p[8], p[9], p[10]];
            let mut position = [p[24], p[25], p[26], p[27]];
            let from = position;

            for force in pass.forces {
                let direction = [force[0], force[1], force[2]];
                let normal = shader.random_normal_with_direction_and_angle(direction, force[3]);
                let speed = dot(direction, direction).sqrt();

                velocity = add(velocity, scale(normal, speed));
            }

            for c in 0..3 {
                position[c] += velocity[c] * pass.delta_time;
            }

            if let Some(bounded) = pass.bounded {
                bounce(bounded, from, &mut position, &mut velocity);
            }

            out[0] = life as f32;
            out[1] = lifetime;
            out[2] = elapsed;
            out[3] = pass.ramps.tex_coord_index0(fraction);
            out[4..8].copy_from_slice(&pass.ramps.color(fraction));
            out[8..11].copy_from_slice(&velocity);
            out[24..28].copy_from_slice(&position);
        }

        // vec3 scale = getScale (fraction); output3 … output5 by X3D_GEOMETRY_TYPE.
        let velocity = [out[8], out[9], out[10]];

        write_matrix(
            &mut out,
            pass.geometry_type,
            pass.particle_size,
            pass.ramps,
            fraction,
            velocity,
        );

        *p = out;
    }
}

// ---------------------------------------------------------------------------------------------
// ParticleSystem.animateParticles (CPU half) and the buffer bookkeeping

/// The state `ParticleSystem` keeps besides its fields: the particle buffer and the counters that
/// decide how many particles exist and when an ExplosionEmitter bursts.
#[derive(Clone, Debug, Default)]
pub struct Sim {
    /// `maxParticles` (clamped >= 0): the number of particle slots.
    pub max_particles: usize,
    /// `numParticles`: slots 0 .. num_particles are animated and drawn.
    pub num_particles: usize,
    /// Seconds (`Date .now () / 1000`) of the last particle creation / burst.
    pub creation_time: f64,
    pub pause_time: f64,
    /// The moving average of the frame time (starts at 0, converges to `1 / frameRate`).
    pub delta_time: f64,
    /// `createParticles` of the last animate (the field, false between ExplosionEmitter bursts).
    pub create_particles: bool,
    pub num_forces: usize,
    /// The force texture rows of the last animate (`forces`: x y z, w = turbulence), `num_forces` of them.
    pub forces: Vec<[f32; 4]>,
    /// The transform feedback buffer: `max_particles` records.
    pub particles: Vec<Particle>,
    /// The LINE triangles the last DISPLAY pass made (X3DLineGeometryNode.displayInstancedThick), if it made any.
    pub thick_lines: Option<std::rc::Rc<ThickLines>>,
    /// `thick_lines` as it was when each layer of the last DISPLAY pass began its draw, by layer (the key of
    /// x3d_render's LayerKey: `None` for layer 0): what the shadow maps of that layer drew (X3DRenderObject.draw renders
    /// them before the shapes, and the layers draw in order, so a layer sees the triangles of the layers before it).
    pub thick_lines_before_layer:
        Vec<(Option<x3d_fields::NodeId>, Option<std::rc::Rc<ThickLines>>)>,
}

/// LineProperties.getTransformLines (a width above ALIASED_LINE_WIDTH_RANGE [1]) of a LINE ParticleSystem: the
/// triangles X3DLineGeometryNode.displayInstancedThick writes into `shapeNode [_lineTrianglesBuffer0]` (positions) and
/// `[_lineTrianglesBuffer1]` (instance matrices), and `shapeNode [_numLines0]`. The node keeps them until the next
/// DISPLAY pass draws the system; displaySimpleInstancedThick (pointing, collision and shadow passes) draws them while
/// `num_lines` is the current line count, and nothing otherwise. Reset does not touch them (they are GL buffers).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThickLines {
    /// `numLines`: line segments × particles when the triangles were made.
    pub num_lines: usize,
    /// Six positions (x y z 1, the particle's own space) per segment and particle, in particle order.
    pub positions: Vec<[f32; 4]>,
    /// The instance matrix of each particle (column-major, as transform feedback wrote it).
    pub instances: Vec<[f32; 16]>,
}

impl Sim {
    /// `resizeBuffers (lastNumParticles)` for the new `max_particles`: the first
    /// `min (max, lastNumParticles)` records survive, the rest are zero.
    pub fn resize(&mut self, max_particles: usize, last_num_particles: usize) {
        self.max_particles = max_particles;

        let keep = max_particles
            .min(last_num_particles)
            .min(self.particles.len());

        self.particles.truncate(keep);
        self.particles.resize(max_particles, [0.0; STRIDE]);
    }

    /// Zero the buffers, `deltaTime`, `numParticles` and set `creationTime`: the state before the
    /// first `animateParticles`.
    pub fn reset(&mut self, creation_time: f64) {
        self.delta_time = 0.0;
        self.num_particles = 0;
        self.creation_time = creation_time;
        self.particles.iter_mut().for_each(|p| *p = [0.0; STRIDE]);
    }

    /// The CPU part of `animateParticles`. Returns the `deltaTime` handed to the emitter (the moving
    /// average, `Infinity` on an ExplosionEmitter burst). `now_ms` is `Date .now ()`.
    pub fn schedule(
        &mut self,
        frame_rate: f64,
        now_ms: f64,
        explosive: bool,
        particle_lifetime: f64,
        lifetime_variation: f64,
        create_field: bool,
    ) -> f64 {
        const DELAY: f64 = 15.0;

        let dt = 1.0 / jsmath::js_max(frame_rate, 10.0);

        self.delta_time = ((DELAY - 1.0) * self.delta_time + dt) / DELAY;

        let mut delta_time = self.delta_time;
        let now = now_ms / 1000.0;

        if explosive {
            let lifetime = particle_lifetime + particle_lifetime * lifetime_variation;

            if now - self.creation_time > lifetime {
                self.creation_time = now;
                self.num_particles = self.max_particles;
                self.create_particles = create_field;
                delta_time = f64::INFINITY;
            } else {
                self.create_particles = false;
            }
        } else {
            self.create_particles = create_field;

            if self.num_particles < self.max_particles {
                let new_particles = jsmath::js_max(
                    0.0,
                    ((now - self.creation_time) * self.max_particles as f64 / particle_lifetime)
                        .floor(),
                );

                if new_particles != 0.0 && !new_particles.is_nan() {
                    self.creation_time = now;
                }

                // Math.min (maxParticles, numParticles + newParticles), with JS NaN and Infinity.
                let n = jsmath::js_min(
                    self.max_particles as f64,
                    self.num_particles as f64 + new_particles,
                );

                self.num_particles = if n.is_nan() { 0 } else { n as usize };
            }
        }

        delta_time
    }
}

#[cfg(test)]
mod ramp_tests {
    use super::*;

    fn ramp(keys: &[f32], values: &[[f32; 4]]) -> Ramp {
        Ramp {
            count: keys.len(),
            texels: keys
                .iter()
                .map(|k| [*k, 0.0, 0.0, 0.0])
                .chain(values.iter().copied())
                .collect(),
        }
    }

    #[test]
    fn weighted_interpolate_follows_the_glsl() {
        let r = ramp(
            &[0.0, 0.5, 1.0],
            &[
                [1.0, 0.0, 0.0, 1.0],
                [0.0, 1.0, 0.0, 1.0],
                [0.0, 0.0, 1.0, 0.0],
            ],
        );

        assert_eq!(r.weighted(-1.0), (0, 0, 0.0));
        assert_eq!(r.weighted(0.0), (0, 0, 0.0));
        assert_eq!(r.weighted(0.25), (0, 1, 0.5));
        assert_eq!(r.weighted(0.75), (1, 2, 0.5));
        assert_eq!(r.weighted(1.0), (1, 2, 1.0));
        assert_eq!(r.weighted(7.0), (1, 2, 1.0));
        assert_eq!(r.mix(0.25), [0.5, 0.5, 0.0, 1.0]);
        assert_eq!(r.mix(0.75), [0.0, 0.5, 0.5, 0.5]);
    }

    #[test]
    fn tex_coord_index_is_a_step_that_never_reaches_the_last_key() {
        // interpolate (sampler, count, fraction, index0): fraction >= the last key gives count - 2.
        let two = Ramps {
            tex_coords: Some(ramp(&[0.0, 0.5], &[])),
            tex_coords_count: 4,
            ..Default::default()
        };
        let three = Ramps {
            tex_coords: Some(ramp(&[0.0, 0.3, 0.6], &[])),
            tex_coords_count: 4,
            ..Default::default()
        };

        for f in [0.0, 0.25, 0.5, 0.9] {
            assert_eq!(two.tex_coord_index0(f), 2.0);
        }

        assert_eq!(three.tex_coord_index0(0.1), 3.0);
        assert_eq!(three.tex_coord_index0(0.4), 7.0);
        assert_eq!(three.tex_coord_index0(0.9), 7.0);
        assert_eq!(Ramps::default().tex_coord_index0(0.5), -1.0);
    }

    #[test]
    fn matrix_of_each_geometry_type() {
        let ramps = Ramps {
            scales: Some(ramp(&[0.0], &[[2.0, 3.0, 4.0, 1.0]])),
            ..Default::default()
        };
        let mut out: Particle = [0.0; STRIDE];

        write_matrix(
            &mut out,
            GeometryType::Quad,
            [0.1, 0.2],
            &ramps,
            0.5,
            [0.0, 1.0, 0.0],
        );
        assert_eq!(
            out[12..24],
            [0.2, 0.0, 0.0, 0.0, 0.0, 0.6, 0.0, 0.0, 0.0, 0.0, 4.0, 0.0]
        );

        write_matrix(
            &mut out,
            GeometryType::Sprite,
            [0.1, 0.2],
            &ramps,
            0.5,
            [0.0, 1.0, 0.0],
        );
        assert_eq!(
            out[12..24],
            [2.0, 0.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0, 0.0, 4.0, 0.0]
        );

        // LINE: the rotation from z to the velocity, its columns times the scale.
        write_matrix(
            &mut out,
            GeometryType::Line,
            [0.1, 0.2],
            &Ramps::default(),
            0.5,
            [0.0, 5.0, 0.0],
        );
        let z = [out[20], out[21], out[22]];
        assert!(
            (z[0]).abs() < 1e-6 && (z[1] - 1.0).abs() < 1e-6 && z[2].abs() < 1e-6,
            "{z:?}"
        );

        write_matrix(
            &mut out,
            GeometryType::Line,
            [0.1, 0.2],
            &ramps,
            0.5,
            [0.0, 5.0, 0.0],
        );
        assert!((out[21] - 4.0).abs() < 1e-6, "{:?}", &out[12..24]);
    }
}
