//! btCollisionDispatcher and the collision algorithms of btDefaultCollisionConfiguration
//! (Bullet 2.82, zlib license): sphere-sphere, box-box, convex-convex (GJK/EPA, no perturbation),
//! convex-concave (BVH triangle mesh, height field, with sphere-triangle), compound and
//! compound-compound. Algorithms are cached in the broadphase pairs, so manifolds persist.

use super::body::{BodyId, RigidBody};
use super::boxbox;
use super::contact::{G_CONTACT_BREAKING_THRESHOLD, ManifoldId, ManifoldPool, ManifoldResult};
use super::dbvt::Aabb;
use super::math::*;
use super::shape::{Shape, ShapeId, ShapeType, Shapes, to_bt};
use crate::bullet::{self, Convex, Placed};

/// A btCollisionObjectWrapper: shape, world transform and the owning object.
#[derive(Clone, Copy, Debug)]
pub struct Wrap {
    pub shape: ShapeId,
    pub tf: Tf,
    pub body: BodyId,
}

/// A joint as seen by `btCollisionObject::checkCollideWith`.
#[derive(Clone, Copy, Debug)]
pub struct ConstraintLink {
    pub a: BodyId,
    pub b: BodyId,
    pub enabled: bool,
}

/// Everything an algorithm needs from the world.
pub struct Cx<'a> {
    pub shapes: &'a Shapes,
    pub bodies: &'a [RigidBody],
    pub pool: &'a mut ManifoldPool,
    pub links: &'a [ConstraintLink],
}

impl Cx<'_> {
    /// btCollisionDispatcher::needsCollision.
    pub fn needs_collision(&self, b0: BodyId, b1: BodyId) -> bool {
        let (o0, o1) = (&self.bodies[b0], &self.bodies[b1]);

        if !o0.is_active() && !o1.is_active() {
            return false;
        }

        if o0.check_collide_with {
            for &c in &o0.constraint_refs {
                let l = &self.links[c];
                if l.a == b1 || l.b == b1 {
                    return !l.enabled;
                }
            }
        }

        true
    }

    /// btCollisionDispatcher::needsResponse.
    pub fn needs_response(&self, b0: BodyId, b1: BodyId) -> bool {
        let (o0, o1) = (&self.bodies[b0], &self.bodies[b1]);
        o0.has_contact_response()
            && o1.has_contact_response()
            && (!o0.is_static_or_kinematic() || !o1.is_static_or_kinematic())
    }

    /// btCollisionDispatcher::getNewManifold for two collision objects.
    pub fn new_manifold(&mut self, b0: BodyId, b1: BodyId) -> ManifoldId {
        let threshold = |b: BodyId| {
            self.shapes
                .contact_breaking_threshold(self.bodies[b].shape, G_CONTACT_BREAKING_THRESHOLD)
        };
        let breaking = threshold(b0).min(threshold(b1));
        let processing = self.bodies[b0]
            .contact_processing_threshold
            .min(self.bodies[b1].contact_processing_threshold);
        self.pool.new_manifold(b0, b1, breaking, processing)
    }
}

/// A cached collision algorithm.
#[derive(Debug)]
pub enum Algo {
    /// Nothing registered for the pair of shape types.
    Empty,
    SphereSphere {
        manifold: ManifoldId,
        own: bool,
    },
    BoxBox {
        manifold: Option<ManifoldId>,
        own: bool,
    },
    ConvexConvex {
        manifold: Option<ManifoldId>,
        own: bool,
    },
    ConvexConcave {
        swapped: bool,
        manifold: ManifoldId,
    },
    Compound {
        swapped: bool,
        revision: i32,
        children: Vec<Option<Box<Algo>>>,
    },
    CompoundCompound {
        rev0: i32,
        rev1: i32,
        cache: Vec<(usize, usize, Box<Algo>)>,
    },
}

/// btCollisionDispatcher::findAlgorithm for wrappers (w0, w1).
pub fn find_algorithm(cx: &mut Cx, w0: &Wrap, w1: &Wrap) -> Algo {
    let t0 = cx.shapes.shape_type(w0.shape);
    let t1 = cx.shapes.shape_type(w1.shape);

    if t0 == ShapeType::Sphere && t1 == ShapeType::Sphere {
        let manifold = cx.new_manifold(w0.body, w1.body);
        return Algo::SphereSphere {
            manifold,
            own: true,
        };
    }

    if t0 == ShapeType::Box && t1 == ShapeType::Box {
        let manifold = if cx.needs_collision(w0.body, w1.body) {
            Some(cx.new_manifold(w0.body, w1.body))
        } else {
            None
        };
        return Algo::BoxBox {
            manifold,
            own: manifold.is_some(),
        };
    }

    if t0.is_convex() && t1.is_convex() {
        return Algo::ConvexConvex {
            manifold: None,
            own: false,
        };
    }

    if t0.is_convex() && t1.is_concave() {
        let manifold = cx.new_manifold(w0.body, w1.body);
        return Algo::ConvexConcave {
            swapped: false,
            manifold,
        };
    }

    if t1.is_convex() && t0.is_concave() {
        let manifold = cx.new_manifold(w1.body, w0.body);
        return Algo::ConvexConcave {
            swapped: true,
            manifold,
        };
    }

    if t0.is_compound() && t1.is_compound() {
        let rev0 = cx.shapes.update_revision(w0.shape);
        let rev1 = cx.shapes.update_revision(w1.shape);
        return Algo::CompoundCompound {
            rev0,
            rev1,
            cache: vec![],
        };
    }

    if t0.is_compound() || t1.is_compound() {
        let swapped = !t0.is_compound();
        let comp = if swapped { w1.shape } else { w0.shape };
        let revision = cx.shapes.update_revision(comp);
        let n = cx.shapes.num_child_shapes(comp);
        // the compound has a dynamic AABB tree, so no child algorithms are preallocated
        return Algo::Compound {
            swapped,
            revision,
            children: (0..n).map(|_| None).collect(),
        };
    }

    Algo::Empty
}

impl Algo {
    /// The destructor: release every owned manifold.
    pub fn release(self, cx: &mut Cx) {
        match self {
            Algo::SphereSphere { manifold, own } => {
                if own {
                    cx.pool.release_manifold(manifold);
                }
            }
            Algo::BoxBox { manifold, own } | Algo::ConvexConvex { manifold, own } => {
                if own {
                    if let Some(m) = manifold {
                        cx.pool.release_manifold(m);
                    }
                }
            }
            Algo::ConvexConcave { manifold, .. } => {
                // ~btConvexTriangleCallback: clearCache, releaseManifold
                cx.pool.get_mut(manifold).points.clear();
                cx.pool.release_manifold(manifold);
            }
            Algo::Compound { children, .. } => {
                for c in children.into_iter().flatten() {
                    c.release(cx);
                }
            }
            Algo::CompoundCompound { cache, .. } => {
                for (_, _, a) in cache {
                    a.release(cx);
                }
            }
            Algo::Empty => {}
        }
    }

    /// getAllContactManifolds.
    pub fn all_manifolds(&self, out: &mut Vec<ManifoldId>) {
        match self {
            Algo::SphereSphere { manifold, own } => {
                if *own {
                    out.push(*manifold);
                }
            }
            Algo::BoxBox { manifold, own } | Algo::ConvexConvex { manifold, own } => {
                if let (Some(m), true) = (manifold, own) {
                    out.push(*m);
                }
            }
            Algo::ConvexConcave { manifold, .. } => out.push(*manifold),
            Algo::Compound { children, .. } => {
                for c in children.iter().flatten() {
                    c.all_manifolds(out);
                }
            }
            Algo::CompoundCompound { cache, .. } => {
                for (_, _, a) in cache {
                    a.all_manifolds(out);
                }
            }
            Algo::Empty => {}
        }
    }

    /// processCollision.
    pub fn process(&mut self, cx: &mut Cx, w0: &Wrap, w1: &Wrap, res: &mut ManifoldResult) {
        match self {
            Algo::Empty => {}
            Algo::SphereSphere { manifold, .. } => sphere_sphere(cx, *manifold, w0, w1, res),
            Algo::BoxBox { manifold, own } => {
                let Some(m) = *manifold else { return };
                res.manifold = Some(m);
                let (Shape::Conv(c0), Shape::Conv(c1)) =
                    (&cx.shapes.v[w0.shape], &cx.shapes.v[w1.shape])
                else {
                    return;
                };
                let side1 = scale(add(c0.implicit, [c0.margin; 3]), 2.0);
                let side2 = scale(add(c1.implicit, [c1.margin; 3]), 2.0);
                let contacts = boxbox::box_box(w0.tf.o, &w0.tf.b, side1, w1.tf.o, &w1.tf.b, side2);

                for (n, p, d) in contacts {
                    res.add_contact_point(cx.pool, cx.bodies, n, p, d);
                }

                if *own {
                    res.refresh_contact_points(cx.pool, cx.bodies);
                }
            }
            Algo::ConvexConvex { manifold, own } => {
                if manifold.is_none() {
                    *manifold = Some(cx.new_manifold(w0.body, w1.body));
                    *own = true;
                }
                let m = manifold.unwrap();
                res.manifold = Some(m);
                convex_convex(cx, m, w0, w1, res);

                if *own {
                    res.refresh_contact_points(cx.pool, cx.bodies);
                }
            }
            Algo::ConvexConcave { swapped, manifold } => {
                convex_concave(cx, *swapped, *manifold, w0, w1, res)
            }
            Algo::Compound {
                swapped,
                revision,
                children,
            } => compound(cx, *swapped, *revision, children, w0, w1, res),
            Algo::CompoundCompound { rev0, rev1, cache } => {
                compound_compound(cx, *rev0, *rev1, cache, w0, w1, res)
            }
        }
    }
}

/// btSphereSphereCollisionAlgorithm::processCollision.
fn sphere_sphere(
    cx: &mut Cx,
    manifold: ManifoldId,
    w0: &Wrap,
    w1: &Wrap,
    res: &mut ManifoldResult,
) {
    res.manifold = Some(manifold);
    let r0 = cx.shapes.get_margin(w0.shape);
    let r1 = cx.shapes.get_margin(w1.shape);
    let diff = sub(w0.tf.o, w1.tf.o);
    let l = len(diff);

    // iff distance positive, don't generate a new contact
    if l > r0 + r1 {
        res.refresh_contact_points(cx.pool, cx.bodies);
        return;
    }

    let dist = l - (r0 + r1);
    let mut normal_on_surface_b = [1.0, 0.0, 0.0];

    // the asm keeps the normal unrounded for pos1 and stores it as a float for the callee
    if l > SIMD_EPSILON {
        let inv = 1.0 / l;
        normal_on_surface_b = [diff[0] * inv, diff[1] * inv, diff[2] * inv];
    }

    let pos1 = add(w1.tf.o, scale(normal_on_surface_b, r1));
    res.add_contact_point(cx.pool, cx.bodies, r3(normal_on_surface_b), r3(pos1), dist);
    res.refresh_contact_points(cx.pool, cx.bodies);
}

/// The Convex of bullet.rs for a convex shape.
fn core_of(shapes: &Shapes, id: ShapeId) -> Option<Convex> {
    match &shapes.v[id] {
        Shape::Conv(c) => Some(c.core()),
        _ => None,
    }
}

/// btConvexConvexAlgorithm::processCollision for the GJK path (no polyhedron data, no
/// perturbation iterations, no capsule-capsule special case).
fn convex_convex(
    cx: &mut Cx,
    manifold: ManifoldId,
    w0: &Wrap,
    w1: &Wrap,
    res: &mut ManifoldResult,
) {
    let (Some(a), Some(b)) = (core_of(cx.shapes, w0.shape), core_of(cx.shapes, w1.shape)) else {
        return;
    };
    let threshold = cx.pool.get(manifold).contact_breaking_threshold;
    let m = a.margin() + b.margin() + threshold;
    let max_d2 = m * m;

    if let Some(r) = super::gjk::gjk_pair(
        Placed {
            shape: &a,
            t: to_bt(&w0.tf),
        },
        Placed {
            shape: &b,
            t: to_bt(&w1.tf),
        },
        max_d2,
    ) {
        res.add_contact_point(
            cx.pool,
            cx.bodies,
            r3(r.normal),
            r3(r.point_on_b),
            r.distance,
        );
    }
}

/// btConvexConcaveCollisionAlgorithm::processCollision with btConvexTriangleCallback.
fn convex_concave(
    cx: &mut Cx,
    swapped: bool,
    manifold: ManifoldId,
    w0: &Wrap,
    w1: &Wrap,
    res: &mut ManifoldResult,
) {
    let convex_w = if swapped { w1 } else { w0 };
    let tri_w = if swapped { w0 } else { w1 };

    if !cx.shapes.shape_type(tri_w.shape).is_concave()
        || !cx.shapes.shape_type(convex_w.shape).is_convex()
    {
        return;
    }

    res.manifold = Some(manifold);

    // setTimeStepAndCounters: the convex shape's AABB in triangle space
    let convex_in_tri = rt(tri_w.tf.inverse().mul(&convex_w.tf));
    let (mut lo, mut hi) = cx.shapes.aabb(convex_w.shape, &convex_in_tri);
    let extra = cx.shapes.get_margin(tri_w.shape);
    hi = r3(add(hi, [extra; 3]));
    lo = r3(sub(lo, [extra; 3]));
    cx.pool.get_mut(manifold).body0 = convex_w.body;
    cx.pool.get_mut(manifold).body1 = tri_w.body;

    let Some(convex) = core_of(cx.shapes, convex_w.shape) else {
        return;
    };
    let tt = to_bt(&tri_w.tf);
    let tc = to_bt(&convex_w.tf);
    let threshold = cx.pool.get(manifold).contact_breaking_threshold;

    // processAllTriangles; collect the triangles first, the callback then runs for each of them.
    let mut tris: Vec<[[f64; 3]; 3]> = vec![];
    match &cx.shapes.v[tri_w.shape] {
        Shape::Bvh(m) => m.process_all_triangles(lo, hi, |t| tris.push(*t)),
        Shape::Height(h) => h.process_all_triangles(lo, hi, |t| tris.push(*t)),
        _ => {}
    }

    for v in tris {
        if !bullet::triangle_overlaps_aabb(&v, lo, hi) {
            continue;
        }

        if let Convex::Sphere { radius } = &convex {
            // btSphereTriangleCollisionAlgorithm (the sphere is body0 of the child pair)
            let in_tri = tt.inverse_times(&tc);
            if let Some((point, normal, depth)) =
                bullet::sphere_triangle(in_tri.origin, *radius, &v, threshold)
            {
                res.add_contact_point(
                    cx.pool,
                    cx.bodies,
                    r3(tt.rotate(normal)),
                    r3(tt.apply(point)),
                    depth,
                );
            }
        } else {
            let tri = Convex::Triangle { v, margin: extra };
            let max_d2 = (convex.margin() + tri.margin() + threshold).powi(2);

            if let Some(r) = super::gjk::gjk_pair(
                Placed {
                    shape: &convex,
                    t: tc,
                },
                Placed { shape: &tri, t: tt },
                max_d2,
            ) {
                res.add_contact_point(
                    cx.pool,
                    cx.bodies,
                    r3(r.normal),
                    r3(r.point_on_b),
                    r.distance,
                );
            }
        }
    }

    res.refresh_contact_points(cx.pool, cx.bodies);
}

/// The child wrapper of a compound: world transform = parent * child, rounded as a stored btTransform.
fn child_wrap(cx: &Cx, parent: &Wrap, index: usize) -> Wrap {
    let ct = cx.shapes.child_transform(parent.shape, index);
    Wrap {
        shape: cx.shapes.child_shape(parent.shape, index),
        tf: rt(parent.tf.mul(&ct)),
        body: parent.body,
    }
}

/// btCompoundCollisionAlgorithm::processCollision (with a dynamic AABB tree).
fn compound(
    cx: &mut Cx,
    swapped: bool,
    revision: i32,
    children: &mut Vec<Option<Box<Algo>>>,
    w0: &Wrap,
    w1: &Wrap,
    res: &mut ManifoldResult,
) {
    let col = if swapped { w1 } else { w0 };
    let other = if swapped { w0 } else { w1 };

    // btCompoundShape might have changed: make sure the child algorithm caches are still valid
    let rev = cx.shapes.update_revision(col.shape);
    if rev != revision {
        for c in children.drain(..).flatten() {
            c.release(cx);
        }
        let n = cx.shapes.num_child_shapes(col.shape);
        children.resize_with(n, || None);
        // m_compoundShapeRevision is only set by the constructor, so the caches are rebuilt on
        // every call once the compound has changed.
    }

    // we need to refresh all contact manifolds
    for c in children.iter().flatten() {
        let mut ms = vec![];
        c.all_manifolds(&mut ms);

        for m in ms {
            if cx.pool.get(m).num_contacts() > 0 {
                let mut r = *res;
                r.manifold = Some(m);
                r.refresh_contact_points(cx.pool, cx.bodies);
            }
        }
    }

    // use the dynamic aabb tree to cull potential child-overlaps
    let other_in_compound = rt(col.tf.inverse().mul(&other.tf));
    let (lo, hi) = cx.shapes.aabb(other.shape, &other_in_compound);
    let bounds = Aabb::from_mm(lo, hi);
    let mut leaves = vec![];

    if let Shape::Compound(c) = &cx.shapes.v[col.shape] {
        c.tree.collide_tv(c.tree.root, &bounds, &mut leaves);
        let idx: Vec<usize> = leaves.iter().map(|n| c.tree.nodes[*n].data).collect();
        leaves = idx;
    }

    for index in leaves {
        process_child_shape(cx, children, col, other, index, res);
    }

    // if not longer overlapping, remove the algorithm
    for i in 0..children.len() {
        if children[i].is_some() {
            let cw = child_wrap(cx, col, i);
            let a0 = cx.shapes.aabb(cw.shape, &cw.tf);
            let a1 = cx.shapes.aabb(other.shape, &other.tf);

            if !bullet::aabb_overlap(&a0, &a1) {
                if let Some(a) = children[i].take() {
                    a.release(cx);
                }
            }
        }
    }
}

/// btCompoundLeafCallback::ProcessChildShape.
fn process_child_shape(
    cx: &mut Cx,
    children: &mut [Option<Box<Algo>>],
    col: &Wrap,
    other: &Wrap,
    index: usize,
    res: &mut ManifoldResult,
) {
    let cw = child_wrap(cx, col, index);
    let a0 = cx.shapes.aabb(cw.shape, &cw.tf);
    let a1 = cx.shapes.aabb(other.shape, &other.tf);

    if bullet::aabb_overlap(&a0, &a1) {
        if children[index].is_none() {
            children[index] = Some(Box::new(find_algorithm(cx, &cw, other)));
        }

        let mut algo = children[index].take().unwrap();
        algo.process(cx, &cw, other, res);
        children[index] = Some(algo);
    }
}

/// btCompoundCompoundCollisionAlgorithm::processCollision.
fn compound_compound(
    cx: &mut Cx,
    rev0: i32,
    rev1: i32,
    cache: &mut Vec<(usize, usize, Box<Algo>)>,
    w0: &Wrap,
    w1: &Wrap,
    res: &mut ManifoldResult,
) {
    let r0 = cx.shapes.update_revision(w0.shape);
    let r1 = cx.shapes.update_revision(w1.shape);

    if r0 != rev0 || r1 != rev1 {
        // clear all
        for (_, _, a) in cache.drain(..) {
            a.release(cx);
        }
    }

    // we need to refresh all contact manifolds
    for (_, _, a) in cache.iter() {
        let mut ms = vec![];
        a.all_manifolds(&mut ms);

        for m in ms {
            if cx.pool.get(m).num_contacts() > 0 {
                let mut r = *res;
                r.manifold = Some(m);
                r.refresh_contact_points(cx.pool, cx.bodies);
            }
        }
    }

    // MycollideTT. The asm inlines `inverse () * world` and btTransformAabb into the traversal and never
    // stores the relative transform: everything below stays an unrounded double
    let col = |t: &Tf, j: usize| [t.b[0][j], t.b[1][j], t.b[2][j]];
    let ac = [col(&w0.tf, 0), col(&w0.tf, 1), col(&w0.tf, 2)];
    let bc = [col(&w1.tf, 0), col(&w1.tf, 1), col(&w1.tf, 2)];
    let xm: [[f64; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|j| dot(ac[i], bc[j])));
    let neg_pa = neg(w0.tf.o);
    let origin: V3 = std::array::from_fn(|i| dot(ac[i], neg_pa) + dot(ac[i], w1.tf.o));
    let my_intersect = |a: &Aabb, b: &Aabb| -> bool {
        let c: V3 = std::array::from_fn(|k| (b.mx[k] + b.mi[k]) * 0.5);
        let h: V3 = std::array::from_fn(|k| (b.mx[k] - b.mi[k]) * 0.5 + 0.0);
        (0..3).all(|i| {
            let center = origin[i] + (xm[i][0] * c[0] + xm[i][1] * c[1] + xm[i][2] * c[2]);
            let ext = xm[i][0].abs() * h[0] + xm[i][1].abs() * h[1] + xm[i][2].abs() * h[2];
            a.mi[i] <= ext + center && a.mx[i] >= center - ext
        })
    };
    let mut pairs: Vec<(usize, usize)> = vec![];

    if let (Shape::Compound(c0), Shape::Compound(c1)) =
        (&cx.shapes.v[w0.shape], &cx.shapes.v[w1.shape])
    {
        if c0.tree.root != super::dbvt::NIL && c1.tree.root != super::dbvt::NIL {
            let mut stack = vec![(c0.tree.root, c1.tree.root)];

            while let Some((a, b)) = stack.pop() {
                let (na, nb) = (&c0.tree.nodes[a], &c1.tree.nodes[b]);
                if my_intersect(&na.volume, &nb.volume) {
                    if !na.is_leaf() {
                        if !nb.is_leaf() {
                            stack.push((na.childs[0], nb.childs[0]));
                            stack.push((na.childs[1], nb.childs[0]));
                            stack.push((na.childs[0], nb.childs[1]));
                            stack.push((na.childs[1], nb.childs[1]));
                        } else {
                            stack.push((na.childs[0], b));
                            stack.push((na.childs[1], b));
                        }
                    } else if !nb.is_leaf() {
                        stack.push((a, nb.childs[0]));
                        stack.push((a, nb.childs[1]));
                    } else {
                        pairs.push((na.data, nb.data));
                    }
                }
            }
        }
    }

    for (i0, i1) in pairs {
        let cw0 = child_wrap(cx, w0, i0);
        let cw1 = child_wrap(cx, w1, i1);
        let a0 = cx.shapes.aabb(cw0.shape, &cw0.tf);
        let a1 = cx.shapes.aabb(cw1.shape, &cw1.tf);

        if bullet::aabb_overlap(&a0, &a1) {
            let slot = match cache.iter().position(|(a, b, _)| *a == i0 && *b == i1) {
                Some(s) => s,
                None => {
                    let algo = find_algorithm(cx, &cw0, &cw1);
                    cache.push((i0, i1, Box::new(algo)));
                    cache.len() - 1
                }
            };

            let mut algo = std::mem::replace(&mut cache[slot].2, Box::new(Algo::Empty));
            algo.process(cx, &cw0, &cw1, res);
            cache[slot].2 = algo;
        }
    }

    // remove non-overlapping child pairs
    let mut remove: Vec<(usize, usize)> = vec![];

    for (i0, i1, _) in cache.iter() {
        let cw0 = child_wrap(cx, w0, *i0);
        let cw1 = child_wrap(cx, w1, *i1);
        let a0 = cx.shapes.aabb(cw0.shape, &cw0.tf);
        let a1 = cx.shapes.aabb(cw1.shape, &cw1.tf);

        if !bullet::aabb_overlap(&a0, &a1) {
            remove.push((*i0, *i1));
        }
    }

    for (i0, i1) in remove {
        // the algorithm is destroyed in the first loop, the hashed simple pair cache then swaps
        // the last pair into the freed slot
        if let Some(pos) = cache.iter().position(|(a, b, _)| *a == i0 && *b == i1) {
            let (_, _, algo) = cache.swap_remove(pos);
            algo.release(cx);
        }
    }
}
