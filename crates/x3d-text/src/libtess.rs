//! Port of libtess.js 1.2.2 (https://github.com/brendankenny/libtess.js, a port of the SGI GLU
//! tessellator; SGI Free Software License B / MIT-compatible), as X_ITE configures it in
//! `Browser/Text/PolygonText.js` `triangulatePolygon`: winding rule NONZERO, normal (0, 0, 1),
//! vertex and combine callbacks only (libtess.js then emits plain triangles).
//!
//! The data structures use arenas instead of object references: half-edges come in pairs so that
//! `sym (e) = e ^ 1`; every `null` is `NIL`. The functions keep libtess.js' names and order so the
//! output (vertex order and all) can be compared with the JavaScript library.

use crate::timsort;

const NIL: usize = usize::MAX;
const GLU_TESS_MAX_COORD: f64 = 1e150;
const SENTINEL_COORD: f64 = 4.0 * GLU_TESS_MAX_COORD;

#[derive(Clone, Default)]
struct HalfEdge {
    next: usize,
    o_next: usize,
    l_next: usize,
    org: usize,
    l_face: usize,
    active_region: usize,
    winding: i32,
}

#[derive(Clone)]
struct Vertex {
    next: usize,
    prev: usize,
    an_edge: usize,
    /// The vertex callback data: the (x, y) position libtess hands back for this vertex.
    data: [f64; 2],
    /// The vertex callback data of the indexed mode (Triangle3.triangulatePolygon: point.index).
    index: usize,
    coords: [f64; 3],
    s: f64,
    t: f64,
    pq_handle: isize,
}

#[derive(Clone)]
struct Face {
    next: usize,
    prev: usize,
    an_edge: usize,
    inside: bool,
}

#[derive(Clone)]
struct Region {
    e_up: usize,
    node_up: usize,
    winding_number: i32,
    inside: bool,
    sentinel: bool,
    dirty: bool,
    fix_upper_edge: bool,
}

#[derive(Clone)]
struct DictNode {
    key: usize,
    next: usize,
    prev: usize,
}

/// libtess.PriorityQHeap
struct PqHeap {
    heap: Vec<usize>,
    verts: Vec<usize>,
    handles: Vec<usize>,
    size: usize,
    max: usize,
    free_list: usize,
    initialized: bool,
}

/// libtess.PriorityQ
struct Pq {
    verts: Vec<usize>,
    order: Vec<usize>,
    size: usize,
    initialized: bool,
    heap: PqHeap,
}

pub struct Tess {
    edges: Vec<HalfEdge>,
    verts: Vec<Vertex>,
    faces: Vec<Face>,
    regions: Vec<Region>,
    dict: Vec<DictNode>,
    pq: Pq,
    event: usize,
    last_edge: usize,
    /// GLU_TESS_WINDING_ODD (Triangle3.triangulatePolygon) instead of NONZERO (PolygonText).
    winding_odd: bool,
    /// Indexed mode: vertex data are indices, the combine callback returns data [0].
    indexed: bool,
    /// NurbsTrimmedSurface mode: vertex data are points with an optional index; a merge of two
    /// coincident vertices keeps the first point (combineCallbackIndex `return a`), an
    /// intersection makes a new point without index (`new Vector3 (... coords)`).
    nurbs: bool,
    /// SVGParser mode: vertex data are indices into the parser's coordinate list; every combine
    /// callback appends a vertex (`svg_new`) and returns its index, counted from `svg_next`.
    svg: bool,
    svg_next: usize,
    svg_new: Vec<[f64; 3]>,
}

const E_HEAD: usize = 0;
const V_HEAD: usize = 0;
const F_HEAD: usize = 0;
const DICT_HEAD: usize = 0;

// --- geom ---------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct St {
    s: f64,
    t: f64,
}

fn vert_eq(u: St, v: St) -> bool {
    u.s == v.s && u.t == v.t
}

fn vert_leq(u: St, v: St) -> bool {
    u.s < v.s || (u.s == v.s && u.t <= v.t)
}

fn edge_eval(u: St, v: St, w: St) -> f64 {
    let gap_l = v.s - u.s;
    let gap_r = w.s - v.s;

    if gap_l + gap_r > 0.0 {
        if gap_l < gap_r {
            return (v.t - u.t) + (u.t - w.t) * (gap_l / (gap_l + gap_r));
        }

        return (v.t - w.t) + (w.t - u.t) * (gap_r / (gap_l + gap_r));
    }

    0.0
}

fn edge_sign(u: St, v: St, w: St) -> f64 {
    let gap_l = v.s - u.s;
    let gap_r = w.s - v.s;

    if gap_l + gap_r > 0.0 {
        return (v.t - w.t) * gap_l + (v.t - u.t) * gap_r;
    }

    0.0
}

fn trans_leq(u: St, v: St) -> bool {
    u.t < v.t || (u.t == v.t && u.s <= v.s)
}

fn trans_eval(u: St, v: St, w: St) -> f64 {
    let gap_l = v.t - u.t;
    let gap_r = w.t - v.t;

    if gap_l + gap_r > 0.0 {
        if gap_l < gap_r {
            return (v.s - u.s) + (u.s - w.s) * (gap_l / (gap_l + gap_r));
        }

        return (v.s - w.s) + (w.s - u.s) * (gap_r / (gap_l + gap_r));
    }

    0.0
}

fn trans_sign(u: St, v: St, w: St) -> f64 {
    let gap_l = v.t - u.t;
    let gap_r = w.t - v.t;

    if gap_l + gap_r > 0.0 {
        return (v.s - w.s) * gap_l + (v.s - u.s) * gap_r;
    }

    0.0
}

fn vert_l1_dist(u: St, v: St) -> f64 {
    (u.s - v.s).abs() + (u.t - v.t).abs()
}

fn interpolate(a: f64, x: f64, b: f64, y: f64) -> f64 {
    let a = if a < 0.0 { 0.0 } else { a };
    let b = if b < 0.0 { 0.0 } else { b };

    if a <= b {
        if b == 0.0 {
            return (x + y) / 2.0;
        }

        x + (y - x) * (a / (a + b))
    } else {
        y + (x - y) * (b / (a + b))
    }
}

/// libtess.geom.edgeIntersect: returns the intersection (s, t).
fn edge_intersect(mut o1: St, mut d1: St, mut o2: St, mut d2: St) -> St {
    let mut v = St { s: 0.0, t: 0.0 };

    if !vert_leq(o1, d1) {
        std::mem::swap(&mut o1, &mut d1);
    }

    if !vert_leq(o2, d2) {
        std::mem::swap(&mut o2, &mut d2);
    }

    if !vert_leq(o1, o2) {
        std::mem::swap(&mut o1, &mut o2);
        std::mem::swap(&mut d1, &mut d2);
    }

    if !vert_leq(o2, d1) {
        v.s = (o2.s + d1.s) / 2.0;
    } else if vert_leq(d1, d2) {
        let mut z1 = edge_eval(o1, o2, d1);
        let mut z2 = edge_eval(o2, d1, d2);

        if z1 + z2 < 0.0 {
            z1 = -z1;
            z2 = -z2;
        }

        v.s = interpolate(z1, o2.s, z2, d1.s);
    } else {
        let mut z1 = edge_sign(o1, o2, d1);
        let mut z2 = -edge_sign(o1, d2, d1);

        if z1 + z2 < 0.0 {
            z1 = -z1;
            z2 = -z2;
        }

        v.s = interpolate(z1, o2.s, z2, d2.s);
    }

    if !trans_leq(o1, d1) {
        std::mem::swap(&mut o1, &mut d1);
    }

    if !trans_leq(o2, d2) {
        std::mem::swap(&mut o2, &mut d2);
    }

    if !trans_leq(o1, o2) {
        std::mem::swap(&mut o1, &mut o2);
        std::mem::swap(&mut d1, &mut d2);
    }

    if !trans_leq(o2, d1) {
        v.t = (o2.t + d1.t) / 2.0;
    } else if trans_leq(d1, d2) {
        let mut z1 = trans_eval(o1, o2, d1);
        let mut z2 = trans_eval(o2, d1, d2);

        if z1 + z2 < 0.0 {
            z1 = -z1;
            z2 = -z2;
        }

        v.t = interpolate(z1, o2.t, z2, d1.t);
    } else {
        let mut z1 = trans_sign(o1, o2, d1);
        let mut z2 = -trans_sign(o1, d2, d1);

        if z1 + z2 < 0.0 {
            z1 = -z1;
            z2 = -z2;
        }

        v.t = interpolate(z1, o2.t, z2, d2.t);
    }

    v
}

// --- priority queue -----------------------------------------------------------------------------

impl PqHeap {
    fn new() -> Self {
        let mut heap = vec![0usize; 33];
        heap[1] = 1;
        PqHeap {
            heap,
            verts: vec![NIL, NIL],
            handles: vec![0, 0],
            size: 0,
            max: 32,
            free_list: 0,
            initialized: false,
        }
    }
}

impl Pq {
    fn new() -> Self {
        Pq {
            verts: vec![],
            order: vec![],
            size: 0,
            initialized: false,
            heap: PqHeap::new(),
        }
    }
}

// --- the tessellator ----------------------------------------------------------------------------

impl Tess {
    fn new() -> Self {
        let mut t = Tess {
            edges: vec![HalfEdge::default(), HalfEdge::default()],
            verts: vec![],
            faces: vec![],
            regions: vec![],
            dict: vec![],
            pq: Pq::new(),
            event: NIL,
            last_edge: NIL,
            winding_odd: false,
            nurbs: false,
            indexed: false,
            svg: false,
            svg_next: 0,
            svg_new: vec![],
        };

        for (i, e) in t.edges.iter_mut().enumerate() {
            e.next = i;
            e.o_next = NIL;
            e.l_next = NIL;
            e.org = NIL;
            e.l_face = NIL;
            e.active_region = NIL;
        }

        t.verts.push(Vertex {
            next: 0,
            prev: 0,
            an_edge: NIL,
            data: [0.0; 2],
            index: NIL,
            coords: [0.0; 3],
            s: 0.0,
            t: 0.0,
            pq_handle: 0,
        });
        t.faces.push(Face {
            next: 0,
            prev: 0,
            an_edge: NIL,
            inside: false,
        });
        t
    }

    // half-edge navigation
    fn r_face(&self, e: usize) -> usize {
        self.edges[e ^ 1].l_face
    }
    fn dst(&self, e: usize) -> usize {
        self.edges[e ^ 1].org
    }
    fn o_prev(&self, e: usize) -> usize {
        self.edges[e ^ 1].l_next
    }
    fn l_prev(&self, e: usize) -> usize {
        self.edges[e].o_next ^ 1
    }
    fn r_prev(&self, e: usize) -> usize {
        self.edges[e ^ 1].o_next
    }
    fn d_next(&self, e: usize) -> usize {
        self.r_prev(e) ^ 1
    }

    fn st(&self, v: usize) -> St {
        St {
            s: self.verts[v].s,
            t: self.verts[v].t,
        }
    }
    fn org(&self, e: usize) -> usize {
        self.edges[e].org
    }

    fn edge_goes_left(&self, e: usize) -> bool {
        vert_leq(self.st(self.dst(e)), self.st(self.org(e)))
    }
    fn edge_goes_right(&self, e: usize) -> bool {
        vert_leq(self.st(self.org(e)), self.st(self.dst(e)))
    }

    // --- mesh -----------------------------------------------------------------------------------

    fn make_edge_pair(&mut self, e_next: usize) -> usize {
        let e = self.edges.len();
        let e_sym = e + 1;

        self.edges.push(HalfEdge {
            next: e,
            o_next: NIL,
            l_next: NIL,
            org: NIL,
            l_face: NIL,
            active_region: NIL,
            winding: 0,
        });
        self.edges.push(HalfEdge {
            next: e_sym,
            o_next: NIL,
            l_next: NIL,
            org: NIL,
            l_face: NIL,
            active_region: NIL,
            winding: 0,
        });

        let e_prev = self.edges[e_next ^ 1].next;

        self.edges[e_sym].next = e_prev;
        self.edges[e_prev ^ 1].next = e;
        self.edges[e].next = e_next;
        self.edges[e_next ^ 1].next = e_sym;
        self.edges[e].o_next = e;
        self.edges[e].l_next = e_sym;
        self.edges[e_sym].o_next = e_sym;
        self.edges[e_sym].l_next = e;
        e
    }

    fn splice(&mut self, a: usize, b: usize) {
        let a_o_next = self.edges[a].o_next;
        let b_o_next = self.edges[b].o_next;

        self.edges[a_o_next ^ 1].l_next = b;
        self.edges[b_o_next ^ 1].l_next = a;
        self.edges[a].o_next = b_o_next;
        self.edges[b].o_next = a_o_next;
    }

    fn make_vertex(&mut self, e_orig: usize, v_next: usize) {
        let v_prev = self.verts[v_next].prev;
        let v_new = self.verts.len();

        self.verts.push(Vertex {
            next: v_next,
            prev: v_prev,
            an_edge: e_orig,
            data: [0.0; 2],
            index: NIL,
            coords: [0.0; 3],
            s: 0.0,
            t: 0.0,
            pq_handle: 0,
        });
        self.verts[v_prev].next = v_new;
        self.verts[v_next].prev = v_new;

        let mut e = e_orig;

        loop {
            self.edges[e].org = v_new;
            e = self.edges[e].o_next;

            if e == e_orig {
                break;
            }
        }
    }

    fn make_face(&mut self, e_orig: usize, f_next: usize) {
        let f_prev = self.faces[f_next].prev;
        let f_new = self.faces.len();
        let inside = self.faces[f_next].inside;

        self.faces.push(Face {
            next: f_next,
            prev: f_prev,
            an_edge: e_orig,
            inside,
        });
        self.faces[f_prev].next = f_new;
        self.faces[f_next].prev = f_new;

        let mut e = e_orig;

        loop {
            self.edges[e].l_face = f_new;
            e = self.edges[e].l_next;

            if e == e_orig {
                break;
            }
        }
    }

    fn kill_edge(&mut self, e_del: usize) {
        let e_next = self.edges[e_del].next;
        let e_prev = self.edges[e_del ^ 1].next;

        self.edges[e_next ^ 1].next = e_prev;
        self.edges[e_prev ^ 1].next = e_next;
    }

    fn kill_vertex(&mut self, v_del: usize, new_org: usize) {
        let e_start = self.verts[v_del].an_edge;
        let mut e = e_start;

        loop {
            self.edges[e].org = new_org;
            e = self.edges[e].o_next;

            if e == e_start {
                break;
            }
        }

        let v_prev = self.verts[v_del].prev;
        let v_next = self.verts[v_del].next;

        self.verts[v_next].prev = v_prev;
        self.verts[v_prev].next = v_next;
    }

    fn kill_face(&mut self, f_del: usize, new_l_face: usize) {
        let e_start = self.faces[f_del].an_edge;
        let mut e = e_start;

        loop {
            self.edges[e].l_face = new_l_face;
            e = self.edges[e].l_next;

            if e == e_start {
                break;
            }
        }

        let f_prev = self.faces[f_del].prev;
        let f_next = self.faces[f_del].next;

        self.faces[f_next].prev = f_prev;
        self.faces[f_prev].next = f_next;
    }

    fn make_edge(&mut self) -> usize {
        let e = self.make_edge_pair(E_HEAD);

        self.make_vertex(e, V_HEAD);
        self.make_vertex(e ^ 1, V_HEAD);
        self.make_face(e, F_HEAD);
        e
    }

    fn mesh_splice(&mut self, e_org: usize, e_dst: usize) {
        let mut joining_loops = false;
        let mut joining_vertices = false;

        if e_org == e_dst {
            return;
        }

        if self.edges[e_dst].org != self.edges[e_org].org {
            joining_vertices = true;
            self.kill_vertex(self.edges[e_dst].org, self.edges[e_org].org);
        }

        if self.edges[e_dst].l_face != self.edges[e_org].l_face {
            joining_loops = true;
            self.kill_face(self.edges[e_dst].l_face, self.edges[e_org].l_face);
        }

        self.splice(e_dst, e_org);

        if !joining_vertices {
            self.make_vertex(e_dst, self.edges[e_org].org);
            let org = self.edges[e_org].org;
            self.verts[org].an_edge = e_org;
        }

        if !joining_loops {
            self.make_face(e_dst, self.edges[e_org].l_face);
            let f = self.edges[e_org].l_face;
            self.faces[f].an_edge = e_org;
        }
    }

    fn delete_edge(&mut self, e_del: usize) {
        let e_del_sym = e_del ^ 1;
        let mut joining_loops = false;

        if self.edges[e_del].l_face != self.r_face(e_del) {
            joining_loops = true;
            self.kill_face(self.edges[e_del].l_face, self.r_face(e_del));
        }

        if self.edges[e_del].o_next == e_del {
            self.kill_vertex(self.edges[e_del].org, NIL);
        } else {
            let rf = self.r_face(e_del);
            self.faces[rf].an_edge = self.o_prev(e_del);
            let org = self.edges[e_del].org;
            self.verts[org].an_edge = self.edges[e_del].o_next;
            self.splice(e_del, self.o_prev(e_del));

            if !joining_loops {
                self.make_face(e_del, self.edges[e_del].l_face);
            }
        }

        if self.edges[e_del_sym].o_next == e_del_sym {
            self.kill_vertex(self.edges[e_del_sym].org, NIL);
            self.kill_face(self.edges[e_del_sym].l_face, NIL);
        } else {
            let lf = self.edges[e_del].l_face;
            self.faces[lf].an_edge = self.o_prev(e_del_sym);
            let org = self.edges[e_del_sym].org;
            self.verts[org].an_edge = self.edges[e_del_sym].o_next;
            self.splice(e_del_sym, self.o_prev(e_del_sym));
        }

        self.kill_edge(e_del);
    }

    fn add_edge_vertex(&mut self, e_org: usize) -> usize {
        let e_new = self.make_edge_pair(e_org);
        let e_new_sym = e_new ^ 1;

        self.splice(e_new, self.edges[e_org].l_next);
        self.edges[e_new].org = self.dst(e_org);
        self.make_vertex(e_new_sym, self.edges[e_new].org);

        let lf = self.edges[e_org].l_face;
        self.edges[e_new].l_face = lf;
        self.edges[e_new_sym].l_face = lf;
        e_new
    }

    fn split_edge(&mut self, e_org: usize) -> usize {
        let temp = self.add_edge_vertex(e_org);
        let e_new = temp ^ 1;

        self.splice(e_org ^ 1, self.o_prev(e_org ^ 1));
        self.splice(e_org ^ 1, e_new);
        self.edges[e_org ^ 1].org = self.edges[e_new].org;

        let d = self.dst(e_new);
        self.verts[d].an_edge = e_new ^ 1;
        self.edges[e_new ^ 1].l_face = self.r_face(e_org);
        self.edges[e_new].winding = self.edges[e_org].winding;
        self.edges[e_new ^ 1].winding = self.edges[e_org ^ 1].winding;
        e_new
    }

    fn connect(&mut self, e_org: usize, e_dst: usize) -> usize {
        let mut joining_loops = false;
        let e_new = self.make_edge_pair(e_org);
        let e_new_sym = e_new ^ 1;

        if self.edges[e_dst].l_face != self.edges[e_org].l_face {
            joining_loops = true;
            self.kill_face(self.edges[e_dst].l_face, self.edges[e_org].l_face);
        }

        self.splice(e_new, self.edges[e_org].l_next);
        self.splice(e_new_sym, e_dst);
        self.edges[e_new].org = self.dst(e_org);
        self.edges[e_new_sym].org = self.edges[e_dst].org;

        let lf = self.edges[e_org].l_face;
        self.edges[e_new].l_face = lf;
        self.edges[e_new_sym].l_face = lf;
        self.faces[lf].an_edge = e_new_sym;

        if !joining_loops {
            self.make_face(e_new, lf);
        }

        e_new
    }

    #[allow(dead_code)]
    fn zap_face(&mut self, f_zap: usize) {
        let e_start = self.faces[f_zap].an_edge;
        let mut e_next = self.edges[e_start].l_next;

        loop {
            let e = e_next;
            e_next = self.edges[e].l_next;
            self.edges[e].l_face = NIL;

            if self.r_face(e) == NIL {
                if self.edges[e].o_next == e {
                    self.kill_vertex(self.edges[e].org, NIL);
                } else {
                    let org = self.edges[e].org;
                    self.verts[org].an_edge = self.edges[e].o_next;
                    self.splice(e, self.o_prev(e));
                }

                let e_sym = e ^ 1;

                if self.edges[e_sym].o_next == e_sym {
                    self.kill_vertex(self.edges[e_sym].org, NIL);
                } else {
                    let org = self.edges[e_sym].org;
                    self.verts[org].an_edge = self.edges[e_sym].o_next;
                    self.splice(e_sym, self.o_prev(e_sym));
                }

                self.kill_edge(e);
            }

            if e == e_start {
                break;
            }
        }

        let f_prev = self.faces[f_zap].prev;
        let f_next = self.faces[f_zap].next;

        self.faces[f_next].prev = f_prev;
        self.faces[f_prev].next = f_next;
    }

    // --- priority queue operations --------------------------------------------------------------

    fn heap_float_down(&mut self, index: usize) {
        let mut curr_index = index;
        let curr_handle = self.pq.heap.heap[curr_index];

        loop {
            let mut child_index = curr_index << 1;

            if child_index < self.pq.heap.size {
                let a = self.pq.heap.verts[self.pq.heap.heap[child_index + 1]];
                let b = self.pq.heap.verts[self.pq.heap.heap[child_index]];

                if vert_leq(self.st(a), self.st(b)) {
                    child_index += 1;
                }
            }

            let child_handle = self.pq.heap.heap[child_index];

            if child_index > self.pq.heap.size
                || vert_leq(
                    self.st(self.pq.heap.verts[curr_handle]),
                    self.st(self.pq.heap.verts[child_handle]),
                )
            {
                self.pq.heap.heap[curr_index] = curr_handle;
                self.pq.heap.handles[curr_handle] = curr_index;
                return;
            }

            self.pq.heap.heap[curr_index] = child_handle;
            self.pq.heap.handles[child_handle] = curr_index;
            curr_index = child_index;
        }
    }

    fn heap_float_up(&mut self, index: usize) {
        let mut curr_index = index;
        let curr_handle = self.pq.heap.heap[curr_index];

        loop {
            let parent_index = curr_index >> 1;
            let parent_handle = self.pq.heap.heap[parent_index];

            if parent_index == 0
                || vert_leq(
                    self.st(self.pq.heap.verts[parent_handle]),
                    self.st(self.pq.heap.verts[curr_handle]),
                )
            {
                self.pq.heap.heap[curr_index] = curr_handle;
                self.pq.heap.handles[curr_handle] = curr_index;
                return;
            }

            self.pq.heap.heap[curr_index] = parent_handle;
            self.pq.heap.handles[parent_handle] = curr_index;
            curr_index = parent_index;
        }
    }

    fn heap_init(&mut self) {
        let mut i = self.pq.heap.size;

        while i >= 1 {
            self.heap_float_down(i);
            i -= 1;
        }

        self.pq.heap.initialized = true;
    }

    fn heap_insert(&mut self, vert: usize) -> usize {
        self.pq.heap.size += 1;
        let end_index = self.pq.heap.size;

        if end_index * 2 > self.pq.heap.max {
            self.pq.heap.max *= 2;
        }

        let new_vert_slot;

        if self.pq.heap.free_list == 0 {
            new_vert_slot = end_index;
        } else {
            new_vert_slot = self.pq.heap.free_list;
            self.pq.heap.free_list = self.pq.heap.handles[self.pq.heap.free_list];
        }

        let h = &mut self.pq.heap;

        if h.verts.len() <= new_vert_slot {
            h.verts.resize(new_vert_slot + 1, NIL);
        }

        if h.handles.len() <= new_vert_slot.max(end_index) {
            h.handles.resize(new_vert_slot.max(end_index) + 1, 0);
        }

        if h.heap.len() <= end_index {
            h.heap.resize(end_index + 1, 0);
        }

        h.verts[new_vert_slot] = vert;
        h.handles[new_vert_slot] = end_index;
        h.heap[end_index] = new_vert_slot;

        if h.initialized {
            self.heap_float_up(end_index);
        }

        new_vert_slot
    }

    fn heap_is_empty(&self) -> bool {
        self.pq.heap.size == 0
    }

    fn heap_minimum(&self) -> usize {
        let h = &self.pq.heap;
        h.verts.get(h.heap[1]).copied().unwrap_or(NIL)
    }

    fn heap_extract_min(&mut self) -> usize {
        let min_handle = self.pq.heap.heap[1];
        let min_vertex = self.pq.heap.verts.get(min_handle).copied().unwrap_or(NIL);

        if self.pq.heap.size > 0 {
            let size = self.pq.heap.size;
            let h = &mut self.pq.heap;

            h.heap[1] = h.heap[size];
            let h1 = h.heap[1];
            h.handles[h1] = 1;
            h.verts[min_handle] = NIL;
            h.handles[min_handle] = h.free_list;
            h.free_list = min_handle;
            h.size -= 1;

            if h.size > 0 {
                self.heap_float_down(1);
            }
        }

        min_vertex
    }

    fn heap_remove(&mut self, remove_handle: usize) {
        let heap_index = self.pq.heap.handles[remove_handle];
        let size = self.pq.heap.size;

        {
            let h = &mut self.pq.heap;
            h.heap[heap_index] = h.heap[size];
            let hi = h.heap[heap_index];
            h.handles[hi] = heap_index;
            h.size -= 1;
        }

        if heap_index <= self.pq.heap.size {
            if heap_index <= 1 {
                self.heap_float_down(heap_index);
            } else {
                let h = &self.pq.heap;
                let vert = h.verts[h.heap[heap_index]];
                let parent_vert = h.verts[h.heap[heap_index >> 1]];

                if vert_leq(self.st(parent_vert), self.st(vert)) {
                    self.heap_float_down(heap_index);
                } else {
                    self.heap_float_up(heap_index);
                }
            }
        }

        let h = &mut self.pq.heap;

        h.verts[remove_handle] = NIL;
        h.handles[remove_handle] = h.free_list;
        h.free_list = remove_handle;
    }

    fn pq_insert(&mut self, vert: usize) -> isize {
        if self.pq.initialized {
            return self.heap_insert(vert) as isize;
        }

        let curr = self.pq.size;

        self.pq.size += 1;

        if self.pq.verts.len() <= curr {
            self.pq.verts.resize(curr + 1, NIL);
        }

        self.pq.verts[curr] = vert;
        -(curr as isize + 1)
    }

    fn pq_init(&mut self) {
        let size = self.pq.size;
        let mut order: Vec<usize> = (0..size).collect();
        let verts = self.pq.verts.clone();
        let sts: Vec<St> = verts
            .iter()
            .map(|v| {
                if *v == NIL {
                    St { s: 0.0, t: 0.0 }
                } else {
                    self.st(*v)
                }
            })
            .collect();

        timsort::sort(
            &mut order,
            |a, b| if vert_leq(sts[*a], sts[*b]) { 1 } else { -1 },
        );

        self.pq.order = order;
        self.pq.initialized = true;
        self.heap_init();
    }

    fn pq_extract_min(&mut self) -> usize {
        if self.pq.size == 0 {
            return self.heap_extract_min();
        }

        let sort_min = self.pq.verts[self.pq.order[self.pq.size - 1]];

        if !self.heap_is_empty() {
            let heap_min = self.heap_minimum();

            if vert_leq(self.st(heap_min), self.st(sort_min)) {
                return self.heap_extract_min();
            }
        }

        loop {
            self.pq.size -= 1;

            if !(self.pq.size > 0 && self.pq.verts[self.pq.order[self.pq.size - 1]] == NIL) {
                break;
            }
        }

        sort_min
    }

    fn pq_minimum(&self) -> usize {
        if self.pq.size == 0 {
            return self.heap_minimum();
        }

        let sort_min = self.pq.verts[self.pq.order[self.pq.size - 1]];

        if !self.heap_is_empty() {
            let heap_min = self.heap_minimum();

            if vert_leq(self.st(heap_min), self.st(sort_min)) {
                return heap_min;
            }
        }

        sort_min
    }

    fn pq_remove(&mut self, remove_handle: isize) {
        if remove_handle >= 0 {
            self.heap_remove(remove_handle as usize);
            return;
        }

        let h = (-(remove_handle + 1)) as usize;

        self.pq.verts[h] = NIL;

        while self.pq.size > 0 && self.pq.verts[self.pq.order[self.pq.size - 1]] == NIL {
            self.pq.size -= 1;
        }
    }

    // --- dictionary -----------------------------------------------------------------------------

    fn region_below(&self, reg: usize) -> usize {
        let node = self.regions[reg].node_up;
        self.dict[self.dict[node].prev].key
    }

    fn region_above(&self, reg: usize) -> usize {
        let node = self.regions[reg].node_up;
        self.dict[self.dict[node].next].key
    }

    /// libtess.sweep.edgeLeq_
    fn edge_leq(&self, reg1: usize, reg2: usize) -> bool {
        let event = self.event;
        let e1 = self.regions[reg1].e_up;
        let e2 = self.regions[reg2].e_up;
        let st = |v: usize| self.st(v);

        if self.dst(e1) == event {
            if self.dst(e2) == event {
                if vert_leq(st(self.org(e1)), st(self.org(e2))) {
                    return edge_sign(st(self.dst(e2)), st(self.org(e1)), st(self.org(e2))) <= 0.0;
                }

                return edge_sign(st(self.dst(e1)), st(self.org(e2)), st(self.org(e1))) >= 0.0;
            }

            return edge_sign(st(self.dst(e2)), st(event), st(self.org(e2))) <= 0.0;
        }

        if self.dst(e2) == event {
            return edge_sign(st(self.dst(e1)), st(event), st(self.org(e1))) >= 0.0;
        }

        let t1 = edge_eval(st(self.dst(e1)), st(event), st(self.org(e1)));
        let t2 = edge_eval(st(self.dst(e2)), st(event), st(self.org(e2)));
        t1 >= t2
    }

    fn dict_insert_before(&mut self, node: usize, key: usize) -> usize {
        let mut node = node;

        loop {
            node = self.dict[node].prev;

            if !(self.dict[node].key != NIL && !self.edge_leq(self.dict[node].key, key)) {
                break;
            }
        }

        let new_node = self.dict.len();
        let next = self.dict[node].next;

        self.dict.push(DictNode {
            key,
            next,
            prev: node,
        });
        self.dict[next].prev = new_node;
        self.dict[node].next = new_node;
        new_node
    }

    fn dict_delete_node(&mut self, node: usize) {
        let (n, p) = (self.dict[node].next, self.dict[node].prev);

        self.dict[n].prev = p;
        self.dict[p].next = n;
    }

    fn dict_search(&self, key: usize) -> usize {
        let mut node = DICT_HEAD;

        loop {
            node = self.dict[node].next;

            if !(self.dict[node].key != NIL && !self.edge_leq(key, self.dict[node].key)) {
                break;
            }
        }

        node
    }

    // --- sweep ----------------------------------------------------------------------------------

    fn new_region(&mut self) -> usize {
        self.regions.push(Region {
            e_up: NIL,
            node_up: NIL,
            winding_number: 0,
            inside: false,
            sentinel: false,
            dirty: false,
            fix_upper_edge: false,
        });
        self.regions.len() - 1
    }

    fn add_winding(&mut self, e_dst: usize, e_src: usize) {
        self.edges[e_dst].winding += self.edges[e_src].winding;
        self.edges[e_dst ^ 1].winding += self.edges[e_src ^ 1].winding;
    }

    fn delete_region(&mut self, reg: usize) {
        let e = self.regions[reg].e_up;

        self.edges[e].active_region = NIL;
        self.dict_delete_node(self.regions[reg].node_up);
        self.regions[reg].node_up = NIL;
    }

    fn fix_upper_edge(&mut self, reg: usize, new_edge: usize) {
        self.delete_edge(self.regions[reg].e_up);
        self.regions[reg].fix_upper_edge = false;
        self.regions[reg].e_up = new_edge;
        self.edges[new_edge].active_region = reg;
    }

    fn top_left_region(&mut self, reg: usize) -> usize {
        let mut reg = reg;
        let org = self.org(self.regions[reg].e_up);

        loop {
            reg = self.region_above(reg);

            if self.org(self.regions[reg].e_up) != org {
                break;
            }
        }

        if self.regions[reg].fix_upper_edge {
            let below = self.region_below(reg);
            let e = self.connect(
                self.regions[below].e_up ^ 1,
                self.edges[self.regions[reg].e_up].l_next,
            );

            self.fix_upper_edge(reg, e);
            reg = self.region_above(reg);
        }

        reg
    }

    fn top_right_region(&self, reg: usize) -> usize {
        let mut reg = reg;
        let dst = self.dst(self.regions[reg].e_up);

        loop {
            reg = self.region_above(reg);

            if self.dst(self.regions[reg].e_up) != dst {
                break;
            }
        }

        reg
    }

    fn add_region_below(&mut self, reg_above: usize, e_new_up: usize) -> usize {
        let reg_new = self.new_region();

        self.regions[reg_new].e_up = e_new_up;

        let node = self.dict_insert_before(self.regions[reg_above].node_up, reg_new);

        self.regions[reg_new].node_up = node;
        self.edges[e_new_up].active_region = reg_new;
        reg_new
    }

    fn is_winding_inside(&self, n: i32) -> bool {
        if self.winding_odd {
            n & 1 != 0 // GLU_TESS_WINDING_ODD
        } else {
            n != 0 // GLU_TESS_WINDING_NONZERO
        }
    }

    fn compute_winding(&mut self, reg: usize) {
        let above = self.region_above(reg);

        self.regions[reg].winding_number =
            self.regions[above].winding_number + self.edges[self.regions[reg].e_up].winding;
        self.regions[reg].inside = self.is_winding_inside(self.regions[reg].winding_number);
    }

    fn finish_region(&mut self, reg: usize) {
        let e = self.regions[reg].e_up;
        let f = self.edges[e].l_face;

        self.faces[f].inside = self.regions[reg].inside;
        self.faces[f].an_edge = e;
        self.delete_region(reg);
    }

    fn finish_left_regions(&mut self, reg_first: usize, reg_last: usize) -> usize {
        let mut reg_prev = reg_first;
        let mut e_prev = self.regions[reg_first].e_up;

        while reg_prev != reg_last {
            self.regions[reg_prev].fix_upper_edge = false;

            let reg = self.region_below(reg_prev);
            let mut e = self.regions[reg].e_up;

            if self.org(e) != self.org(e_prev) {
                if !self.regions[reg].fix_upper_edge {
                    self.finish_region(reg_prev);
                    break;
                }

                e = self.connect(self.l_prev(e_prev), e ^ 1);
                self.fix_upper_edge(reg, e);
            }

            if self.edges[e_prev].o_next != e {
                self.mesh_splice(self.o_prev(e), e);
                self.mesh_splice(e_prev, e);
            }

            self.finish_region(reg_prev);
            e_prev = self.regions[reg].e_up;
            reg_prev = reg;
        }

        e_prev
    }

    fn add_right_edges(
        &mut self,
        reg_up: usize,
        e_first: usize,
        e_last: usize,
        e_top_left: usize,
        clean_up: bool,
    ) {
        let mut first_time = true;
        let mut e = e_first;

        loop {
            self.add_region_below(reg_up, e ^ 1);
            e = self.edges[e].o_next;

            if e == e_last {
                break;
            }
        }

        let mut e_top_left = e_top_left;

        if e_top_left == NIL {
            let below = self.region_below(reg_up);
            e_top_left = self.r_prev(self.regions[below].e_up);
        }

        let mut reg_prev = reg_up;
        let mut e_prev = e_top_left;

        loop {
            let reg = self.region_below(reg_prev);

            e = self.regions[reg].e_up ^ 1;

            if self.org(e) != self.org(e_prev) {
                break;
            }

            if self.edges[e].o_next != e_prev {
                self.mesh_splice(self.o_prev(e), e);
                self.mesh_splice(self.o_prev(e_prev), e);
            }

            self.regions[reg].winding_number =
                self.regions[reg_prev].winding_number - self.edges[e].winding;
            self.regions[reg].inside = self.is_winding_inside(self.regions[reg].winding_number);
            self.regions[reg_prev].dirty = true;

            if !first_time && self.check_for_right_splice(reg_prev) {
                self.add_winding(e, e_prev);
                self.delete_region(reg_prev);
                self.delete_edge(e_prev);
            }

            first_time = false;
            reg_prev = reg;
            e_prev = e;
        }

        self.regions[reg_prev].dirty = true;

        if clean_up {
            self.walk_dirty_regions(reg_prev);
        }
    }

    /// libtess.sweep.callCombine_ with X_ITE's combine callbacks: PolygonText returns
    /// `new Vector3 (... coords)`, Triangle3.triangulatePolygon returns `data [0]` (`first`).
    fn call_combine(&mut self, isect: usize, coords: [f64; 3], first: usize) {
        if self.svg {
            // SVGParser combineCallback: `vertices .push (new Vector3 (... coords))`, return its index.
            self.verts[isect].index = self.svg_next + self.svg_new.len();
            self.svg_new.push(coords);
        } else if self.nurbs {
            self.verts[isect].data = [coords[0], coords[1]];
            self.verts[isect].index = NIL;
        } else if self.indexed {
            self.verts[isect].index = self.verts[first].index;
        } else {
            self.verts[isect].data = [coords[0], coords[1]];
        }
    }

    fn splice_merge_vertices(&mut self, e1: usize, e2: usize) {
        let org = self.org(e1);
        let coords = self.verts[org].coords;

        // NurbsTrimmedSurface: combineCallbackIndex returns `a` (c is null and a, b coincide), the
        // plain combineCallback a new point with the same coordinates: either way the data stay.
        if !self.nurbs {
            self.call_combine(org, coords, org);
        }

        self.mesh_splice(e1, e2);
    }

    fn vertex_weights(
        &mut self,
        isect: usize,
        org: usize,
        dst: usize,
        weights: &mut [f64; 4],
        weight_index: usize,
    ) {
        let t1 = vert_l1_dist(self.st(org), self.st(isect));
        let t2 = vert_l1_dist(self.st(dst), self.st(isect));
        let (i0, i1) = (weight_index, weight_index + 1);

        weights[i0] = 0.5 * t2 / (t1 + t2);
        weights[i1] = 0.5 * t1 / (t1 + t2);

        let (oc, dc) = (self.verts[org].coords, self.verts[dst].coords);
        let ic = &mut self.verts[isect].coords;

        ic[0] += weights[i0] * oc[0] + weights[i1] * dc[0];
        ic[1] += weights[i0] * oc[1] + weights[i1] * dc[1];
        ic[2] += weights[i0] * oc[2] + weights[i1] * dc[2];
    }

    fn get_intersect_data(
        &mut self,
        isect: usize,
        org_up: usize,
        dst_up: usize,
        org_lo: usize,
        dst_lo: usize,
    ) {
        let mut weights = [0.0; 4];

        self.verts[isect].coords = [0.0; 3];
        self.vertex_weights(isect, org_up, dst_up, &mut weights, 0);
        self.vertex_weights(isect, org_lo, dst_lo, &mut weights, 2);

        let coords = self.verts[isect].coords;
        self.call_combine(isect, coords, org_up);
    }

    fn check_for_right_splice(&mut self, reg_up: usize) -> bool {
        let reg_lo = self.region_below(reg_up);
        let e_up = self.regions[reg_up].e_up;
        let e_lo = self.regions[reg_lo].e_up;
        let st = |s: &Self, v: usize| s.st(v);

        if vert_leq(st(self, self.org(e_up)), st(self, self.org(e_lo))) {
            if edge_sign(
                st(self, self.dst(e_lo)),
                st(self, self.org(e_up)),
                st(self, self.org(e_lo)),
            ) > 0.0
            {
                return false;
            }

            if !vert_eq(st(self, self.org(e_up)), st(self, self.org(e_lo))) {
                self.split_edge(e_lo ^ 1);
                self.mesh_splice(e_up, self.o_prev(e_lo));
                self.regions[reg_up].dirty = true;
                self.regions[reg_lo].dirty = true;
            } else if self.org(e_up) != self.org(e_lo) {
                let h = self.verts[self.org(e_up)].pq_handle;

                self.pq_remove(h);
                self.splice_merge_vertices(self.o_prev(e_lo), e_up);
            }
        } else {
            if edge_sign(
                st(self, self.dst(e_up)),
                st(self, self.org(e_lo)),
                st(self, self.org(e_up)),
            ) < 0.0
            {
                return false;
            }

            let above = self.region_above(reg_up);

            self.regions[above].dirty = true;
            self.regions[reg_up].dirty = true;
            self.split_edge(e_up ^ 1);
            self.mesh_splice(self.o_prev(e_lo), e_up);
        }

        true
    }

    fn check_for_left_splice(&mut self, reg_up: usize) -> bool {
        let reg_lo = self.region_below(reg_up);
        let e_up = self.regions[reg_up].e_up;
        let e_lo = self.regions[reg_lo].e_up;
        let st = |s: &Self, v: usize| s.st(v);

        if vert_leq(st(self, self.dst(e_up)), st(self, self.dst(e_lo))) {
            if edge_sign(
                st(self, self.dst(e_up)),
                st(self, self.dst(e_lo)),
                st(self, self.org(e_up)),
            ) < 0.0
            {
                return false;
            }

            let above = self.region_above(reg_up);

            self.regions[above].dirty = true;
            self.regions[reg_up].dirty = true;

            let e = self.split_edge(e_up);

            self.mesh_splice(e_lo ^ 1, e);

            let lf = self.edges[e].l_face;
            self.faces[lf].inside = self.regions[reg_up].inside;
        } else {
            if edge_sign(
                st(self, self.dst(e_lo)),
                st(self, self.dst(e_up)),
                st(self, self.org(e_lo)),
            ) > 0.0
            {
                return false;
            }

            self.regions[reg_up].dirty = true;
            self.regions[reg_lo].dirty = true;

            let e = self.split_edge(e_lo);

            self.mesh_splice(self.edges[e_up].l_next, e_lo ^ 1);

            let rf = self.r_face(e);
            self.faces[rf].inside = self.regions[reg_up].inside;
        }

        true
    }

    fn check_for_intersect(&mut self, reg_up_arg: usize) -> bool {
        let mut reg_up = reg_up_arg;
        let mut reg_lo = self.region_below(reg_up);
        let mut e_up = self.regions[reg_up].e_up;
        let mut e_lo = self.regions[reg_lo].e_up;
        let org_up = self.org(e_up);
        let org_lo = self.org(e_lo);
        let dst_up = self.dst(e_up);
        let dst_lo = self.dst(e_lo);

        if org_up == org_lo {
            return false;
        }

        let t_min_up = self.verts[org_up].t.min(self.verts[dst_up].t);
        let t_max_lo = self.verts[org_lo].t.max(self.verts[dst_lo].t);

        if t_min_up > t_max_lo {
            return false;
        }

        let st = |s: &Self, v: usize| s.st(v);

        if vert_leq(st(self, org_up), st(self, org_lo)) {
            if edge_sign(st(self, dst_lo), st(self, org_up), st(self, org_lo)) > 0.0 {
                return false;
            }
        } else if edge_sign(st(self, dst_up), st(self, org_lo), st(self, org_up)) < 0.0 {
            return false;
        }

        let mut isect = edge_intersect(
            st(self, dst_up),
            st(self, org_up),
            st(self, dst_lo),
            st(self, org_lo),
        );
        let event = self.event;

        if vert_leq(isect, st(self, event)) {
            isect.s = self.verts[event].s;
            isect.t = self.verts[event].t;
        }

        let org_min = if vert_leq(st(self, org_up), st(self, org_lo)) {
            org_up
        } else {
            org_lo
        };

        if vert_leq(st(self, org_min), isect) {
            isect.s = self.verts[org_min].s;
            isect.t = self.verts[org_min].t;
        }

        if vert_eq(isect, st(self, org_up)) || vert_eq(isect, st(self, org_lo)) {
            self.check_for_right_splice(reg_up);
            return false;
        }

        if (!vert_eq(st(self, dst_up), st(self, event))
            && edge_sign(st(self, dst_up), st(self, event), isect) >= 0.0)
            || (!vert_eq(st(self, dst_lo), st(self, event))
                && edge_sign(st(self, dst_lo), st(self, event), isect) <= 0.0)
        {
            if dst_lo == event {
                self.split_edge(e_up ^ 1);
                self.mesh_splice(e_lo ^ 1, e_up);
                reg_up = self.top_left_region(reg_up);
                e_up = self.regions[self.region_below(reg_up)].e_up;
                self.finish_left_regions(self.region_below(reg_up), reg_lo);
                self.add_right_edges(reg_up, self.o_prev(e_up), e_up, e_up, true);
                return true;
            }

            if dst_up == event {
                self.split_edge(e_lo ^ 1);
                self.mesh_splice(self.edges[e_up].l_next, self.o_prev(e_lo));
                reg_lo = reg_up;
                reg_up = self.top_right_region(reg_up);

                let e = self.r_prev(self.regions[self.region_below(reg_up)].e_up);

                self.regions[reg_lo].e_up = self.o_prev(e_lo);
                e_lo = self.finish_left_regions(reg_lo, NIL);
                self.add_right_edges(reg_up, self.edges[e_lo].o_next, self.r_prev(e_up), e, true);
                return true;
            }

            if edge_sign(st(self, dst_up), st(self, event), isect) >= 0.0 {
                let above = self.region_above(reg_up);

                self.regions[above].dirty = true;
                self.regions[reg_up].dirty = true;
                self.split_edge(e_up ^ 1);

                let o = self.org(e_up);
                self.verts[o].s = self.verts[event].s;
                self.verts[o].t = self.verts[event].t;
            }

            if edge_sign(st(self, dst_lo), st(self, event), isect) <= 0.0 {
                self.regions[reg_up].dirty = true;
                self.regions[reg_lo].dirty = true;
                self.split_edge(e_lo ^ 1);

                let o = self.org(e_lo);
                self.verts[o].s = self.verts[event].s;
                self.verts[o].t = self.verts[event].t;
            }

            return false;
        }

        self.split_edge(e_up ^ 1);
        self.split_edge(e_lo ^ 1);
        self.mesh_splice(self.o_prev(e_lo), e_up);

        let o = self.org(e_up);

        self.verts[o].s = isect.s;
        self.verts[o].t = isect.t;

        let h = self.pq_insert(o);

        self.verts[o].pq_handle = h;
        self.get_intersect_data(o, org_up, dst_up, org_lo, dst_lo);

        let above = self.region_above(reg_up);

        self.regions[above].dirty = true;
        self.regions[reg_up].dirty = true;
        self.regions[reg_lo].dirty = true;
        false
    }

    fn walk_dirty_regions(&mut self, reg_up_arg: usize) {
        let mut reg_up = reg_up_arg;
        let mut reg_lo = self.region_below(reg_up);

        loop {
            while self.regions[reg_lo].dirty {
                reg_up = reg_lo;
                reg_lo = self.region_below(reg_lo);
            }

            if !self.regions[reg_up].dirty {
                reg_lo = reg_up;
                reg_up = self.region_above(reg_up);

                if reg_up == NIL || !self.regions[reg_up].dirty {
                    return;
                }
            }

            self.regions[reg_up].dirty = false;

            let mut e_up = self.regions[reg_up].e_up;
            let mut e_lo = self.regions[reg_lo].e_up;

            if self.dst(e_up) != self.dst(e_lo) && self.check_for_left_splice(reg_up) {
                if self.regions[reg_lo].fix_upper_edge {
                    self.delete_region(reg_lo);
                    self.delete_edge(e_lo);
                    reg_lo = self.region_below(reg_up);
                    e_lo = self.regions[reg_lo].e_up;
                } else if self.regions[reg_up].fix_upper_edge {
                    self.delete_region(reg_up);
                    self.delete_edge(e_up);
                    reg_up = self.region_above(reg_lo);
                    e_up = self.regions[reg_up].e_up;
                }
            }

            if self.org(e_up) != self.org(e_lo) {
                if self.dst(e_up) != self.dst(e_lo)
                    && !self.regions[reg_up].fix_upper_edge
                    && !self.regions[reg_lo].fix_upper_edge
                    && (self.dst(e_up) == self.event || self.dst(e_lo) == self.event)
                {
                    if self.check_for_intersect(reg_up) {
                        return;
                    }
                } else {
                    self.check_for_right_splice(reg_up);
                }
            }

            if self.org(e_up) == self.org(e_lo) && self.dst(e_up) == self.dst(e_lo) {
                self.add_winding(e_lo, e_up);
                self.delete_region(reg_up);
                self.delete_edge(e_up);
                reg_up = self.region_above(reg_lo);
            }
        }
    }

    fn connect_right_vertex(&mut self, reg_up_arg: usize, e_bottom_left_arg: usize) {
        let mut reg_up = reg_up_arg;
        let mut e_bottom_left = e_bottom_left_arg;
        let mut e_top_left = self.edges[e_bottom_left].o_next;
        let reg_lo = self.region_below(reg_up);
        let e_up = self.regions[reg_up].e_up;
        let e_lo = self.regions[reg_lo].e_up;
        let mut degenerate = false;

        if self.dst(e_up) != self.dst(e_lo) {
            self.check_for_intersect(reg_up);
        }

        let event = self.event;

        if vert_eq(self.st(self.org(e_up)), self.st(event)) {
            self.mesh_splice(self.o_prev(e_top_left), e_up);
            reg_up = self.top_left_region(reg_up);
            e_top_left = self.regions[self.region_below(reg_up)].e_up;
            self.finish_left_regions(self.region_below(reg_up), reg_lo);
            degenerate = true;
        }

        if vert_eq(self.st(self.org(e_lo)), self.st(event)) {
            self.mesh_splice(e_bottom_left, self.o_prev(e_lo));
            e_bottom_left = self.finish_left_regions(reg_lo, NIL);
            degenerate = true;
        }

        if degenerate {
            self.add_right_edges(
                reg_up,
                self.edges[e_bottom_left].o_next,
                e_top_left,
                e_top_left,
                true,
            );
            return;
        }

        let e_new_arg = if vert_leq(self.st(self.org(e_lo)), self.st(self.org(e_up))) {
            self.o_prev(e_lo)
        } else {
            e_up
        };
        let e_new = self.connect(self.l_prev(e_bottom_left), e_new_arg);

        self.add_right_edges(
            reg_up,
            e_new,
            self.edges[e_new].o_next,
            self.edges[e_new].o_next,
            false,
        );

        let ar = self.edges[e_new ^ 1].active_region;

        self.regions[ar].fix_upper_edge = true;
        self.walk_dirty_regions(reg_up);
    }

    fn connect_left_degenerate(&mut self, reg_up: usize, v_event: usize) {
        let e = self.regions[reg_up].e_up;

        if vert_eq(self.st(self.org(e)), self.st(v_event)) {
            // TOLERANCE_NONZERO is false.
            return;
        }

        if !vert_eq(self.st(self.dst(e)), self.st(v_event)) {
            self.split_edge(e ^ 1);

            if self.regions[reg_up].fix_upper_edge {
                self.delete_edge(self.edges[e].o_next);
                self.regions[reg_up].fix_upper_edge = false;
            }

            self.mesh_splice(self.verts[v_event].an_edge, e);
            self.sweep_event(v_event);
            return;
        }

        // TOLERANCE_NONZERO is false: nothing else to do.
    }

    fn connect_left_vertex(&mut self, v_event: usize) {
        let tmp = self.new_region();

        self.regions[tmp].e_up = self.verts[v_event].an_edge ^ 1;

        let node = self.dict_search(tmp);
        let reg_up = self.dict[node].key;
        let reg_lo = self.region_below(reg_up);
        let e_up = self.regions[reg_up].e_up;
        let e_lo = self.regions[reg_lo].e_up;

        if edge_sign(
            self.st(self.dst(e_up)),
            self.st(v_event),
            self.st(self.org(e_up)),
        ) == 0.0
        {
            self.connect_left_degenerate(reg_up, v_event);
            return;
        }

        let reg = if vert_leq(self.st(self.dst(e_lo)), self.st(self.dst(e_up))) {
            reg_up
        } else {
            reg_lo
        };

        if self.regions[reg_up].inside || self.regions[reg].fix_upper_edge {
            let e_new;

            if reg == reg_up {
                e_new = self.connect(self.verts[v_event].an_edge ^ 1, self.edges[e_up].l_next);
            } else {
                let temp = self.connect(self.d_next(e_lo), self.verts[v_event].an_edge);

                e_new = temp ^ 1;
            }

            if self.regions[reg].fix_upper_edge {
                self.fix_upper_edge(reg, e_new);
            } else {
                let r = self.add_region_below(reg_up, e_new);

                self.compute_winding(r);
            }

            self.sweep_event(v_event);
        } else {
            let a = self.verts[v_event].an_edge;

            self.add_right_edges(reg_up, a, a, NIL, true);
        }
    }

    fn sweep_event(&mut self, v_event: usize) {
        self.event = v_event;

        let mut e = self.verts[v_event].an_edge;

        while self.edges[e].active_region == NIL {
            e = self.edges[e].o_next;

            if e == self.verts[v_event].an_edge {
                self.connect_left_vertex(v_event);
                return;
            }
        }

        let ar = self.edges[e].active_region;
        let reg_up = self.top_left_region(ar);
        let reg = self.region_below(reg_up);
        let e_top_left = self.regions[reg].e_up;
        let e_bottom_left = self.finish_left_regions(reg, NIL);

        if self.edges[e_bottom_left].o_next == e_top_left {
            self.connect_right_vertex(reg_up, e_bottom_left);
        } else {
            self.add_right_edges(
                reg_up,
                self.edges[e_bottom_left].o_next,
                e_top_left,
                e_top_left,
                true,
            );
        }
    }

    fn add_sentinel(&mut self, t: f64) {
        let reg = self.new_region();
        let e = self.make_edge();
        let (o, d) = (self.org(e), self.dst(e));

        self.verts[o].s = SENTINEL_COORD;
        self.verts[o].t = t;
        self.verts[d].s = -SENTINEL_COORD;
        self.verts[d].t = t;
        self.event = d;

        let r = &mut self.regions[reg];

        r.e_up = e;
        r.winding_number = 0;
        r.inside = false;
        r.fix_upper_edge = false;
        r.sentinel = true;
        r.dirty = false;

        let node = self.dict_insert_before(DICT_HEAD, reg);

        self.regions[reg].node_up = node;
    }

    fn init_edge_dict(&mut self) {
        self.dict = vec![DictNode {
            key: NIL,
            next: 0,
            prev: 0,
        }];
        self.add_sentinel(-SENTINEL_COORD);
        self.add_sentinel(SENTINEL_COORD);
    }

    fn done_edge_dict(&mut self) {
        loop {
            let reg = self.dict[self.dict[DICT_HEAD].next].key;

            if reg == NIL {
                break;
            }

            self.delete_region(reg);
        }
    }

    fn remove_degenerate_edges(&mut self) {
        let mut e = self.edges[E_HEAD].next;

        while e != E_HEAD {
            let mut e_next = self.edges[e].next;
            let mut e_l_next = self.edges[e].l_next;

            if vert_eq(self.st(self.org(e)), self.st(self.dst(e)))
                && self.edges[self.edges[e].l_next].l_next != e
            {
                self.splice_merge_vertices(e_l_next, e);
                self.delete_edge(e);
                e = e_l_next;
                e_l_next = self.edges[e].l_next;
            }

            if self.edges[e_l_next].l_next == e {
                if e_l_next != e {
                    if e_l_next == e_next || e_l_next == (e_next ^ 1) {
                        e_next = self.edges[e_next].next;
                    }

                    self.delete_edge(e_l_next);
                }

                if e == e_next || e == (e_next ^ 1) {
                    e_next = self.edges[e_next].next;
                }

                self.delete_edge(e);
            }

            e = e_next;
        }
    }

    fn init_priority_q(&mut self) {
        self.pq = Pq::new();

        let mut v = self.verts[V_HEAD].next;

        while v != V_HEAD {
            let h = self.pq_insert(v);

            self.verts[v].pq_handle = h;
            v = self.verts[v].next;
        }

        self.pq_init();
    }

    fn remove_degenerate_faces(&mut self) {
        let mut f = self.faces[F_HEAD].next;

        while f != F_HEAD {
            let f_next = self.faces[f].next;
            let e = self.faces[f].an_edge;

            if self.edges[self.edges[e].l_next].l_next == e {
                self.add_winding(self.edges[e].o_next, e);
                self.delete_edge(e);
            }

            f = f_next;
        }
    }

    fn compute_interior(&mut self) {
        self.remove_degenerate_edges();
        self.init_priority_q();
        self.init_edge_dict();

        loop {
            let v = self.pq_extract_min();

            if v == NIL {
                break;
            }

            loop {
                let v_next = self.pq_minimum();

                if v_next == NIL || !vert_eq(self.st(v_next), self.st(v)) {
                    break;
                }

                let v_next = self.pq_extract_min();

                self.splice_merge_vertices(self.verts[v].an_edge, self.verts[v_next].an_edge);
            }

            self.sweep_event(v);
        }

        let min_region = self.dict[self.dict[DICT_HEAD].next].key;

        self.event = self.org(self.regions[min_region].e_up);
        self.done_edge_dict();
        self.remove_degenerate_faces();
    }

    // --- tessmono -------------------------------------------------------------------------------

    fn tessellate_mono_region(&mut self, face: usize) {
        let mut up = self.faces[face].an_edge;

        while vert_leq(self.st(self.dst(up)), self.st(self.org(up))) {
            up = self.l_prev(up);
        }

        while vert_leq(self.st(self.org(up)), self.st(self.dst(up))) {
            up = self.edges[up].l_next;
        }

        let mut lo = self.l_prev(up);

        while self.edges[up].l_next != lo {
            if vert_leq(self.st(self.dst(up)), self.st(self.org(lo))) {
                while self.edges[lo].l_next != up
                    && (self.edge_goes_left(self.edges[lo].l_next)
                        || edge_sign(
                            self.st(self.org(lo)),
                            self.st(self.dst(lo)),
                            self.st(self.dst(self.edges[lo].l_next)),
                        ) <= 0.0)
                {
                    let temp = self.connect(self.edges[lo].l_next, lo);

                    lo = temp ^ 1;
                }

                lo = self.l_prev(lo);
            } else {
                while self.edges[lo].l_next != up
                    && (self.edge_goes_right(self.l_prev(up))
                        || edge_sign(
                            self.st(self.dst(up)),
                            self.st(self.org(up)),
                            self.st(self.org(self.l_prev(up))),
                        ) >= 0.0)
                {
                    let temp = self.connect(up, self.l_prev(up));

                    up = temp ^ 1;
                }

                up = self.edges[up].l_next;
            }
        }

        while self.edges[self.edges[lo].l_next].l_next != up {
            let temp = self.connect(self.edges[lo].l_next, lo);

            lo = temp ^ 1;
        }
    }

    fn tessellate_interior(&mut self) {
        let mut f = self.faces[F_HEAD].next;

        while f != F_HEAD {
            let next = self.faces[f].next;

            if self.faces[f].inside {
                self.tessellate_mono_region(f);
            }

            f = next;
        }
    }

    // --- polygon input --------------------------------------------------------------------------

    fn add_vertex(&mut self, x: f64, y: f64) {
        let mut e = self.last_edge;

        if e == NIL {
            e = self.make_edge();
            self.mesh_splice(e, e ^ 1);
        } else {
            self.split_edge(e);
            e = self.edges[e].l_next;
        }

        let o = self.org(e);

        self.verts[o].data = [x, y];
        self.verts[o].coords = [x, y, 0.0];
        self.edges[e].winding = 1;
        self.edges[e ^ 1].winding = -1;
        self.last_edge = e;
    }

    /// libtess.normal.projectPolygon with normal (0, 0, 0): computeNormal_, the long-axis
    /// projection (TRUE_PROJECT is false) and checkOrientation_.
    fn project_polygon_computed(&mut self) {
        let norm = self.compute_normal();
        let i = long_axis(&norm);
        let (s_axis, t_axis) = ((i + 1) % 3, (i + 2) % 3);
        let t_negate = if norm[i] > 0.0 { 1.0 } else { -1.0 };
        let mut v = self.verts[V_HEAD].next;

        while v != V_HEAD {
            self.verts[v].s = self.verts[v].coords[s_axis];
            self.verts[v].t = t_negate * self.verts[v].coords[t_axis];
            v = self.verts[v].next;
        }

        self.check_orientation();
    }

    fn compute_normal(&self) -> [f64; 3] {
        let mut max_val = [-2.0 * GLU_TESS_MAX_COORD; 3];
        let mut min_val = [2.0 * GLU_TESS_MAX_COORD; 3];
        let mut max_vert = [NIL; 3];
        let mut min_vert = [NIL; 3];
        let mut v = self.verts[V_HEAD].next;

        while v != V_HEAD {
            for i in 0..3 {
                let c = self.verts[v].coords[i];
                if c < min_val[i] {
                    min_val[i] = c;
                    min_vert[i] = v;
                }
                if c > max_val[i] {
                    max_val[i] = c;
                    max_vert[i] = v;
                }
            }
            v = self.verts[v].next;
        }

        let mut index = 0;
        if max_val[1] - min_val[1] > max_val[0] - min_val[0] {
            index = 1;
        }
        if max_val[2] - min_val[2] > max_val[index] - min_val[index] {
            index = 2;
        }
        if min_val[index] >= max_val[index] {
            return [0.0, 0.0, 1.0];
        }

        let mut norm = [0.0; 3];
        let mut max_len2 = 0.0;
        let (c1, c2) = (
            self.verts[min_vert[index]].coords,
            self.verts[max_vert[index]].coords,
        );
        let d1 = [c1[0] - c2[0], c1[1] - c2[1], c1[2] - c2[2]];
        let mut v = self.verts[V_HEAD].next;

        while v != V_HEAD {
            let c = self.verts[v].coords;
            let d2 = [c[0] - c2[0], c[1] - c2[1], c[2] - c2[2]];
            let t = [
                d1[1] * d2[2] - d1[2] * d2[1],
                d1[2] * d2[0] - d1[0] * d2[2],
                d1[0] * d2[1] - d1[1] * d2[0],
            ];
            let t_len2 = t[0] * t[0] + t[1] * t[1] + t[2] * t[2];
            if t_len2 > max_len2 {
                max_len2 = t_len2;
                norm = t;
            }
            v = self.verts[v].next;
        }

        if max_len2 <= 0.0 {
            norm = [0.0; 3];
            norm[long_axis(&d1)] = 1.0;
        }

        norm
    }

    fn check_orientation(&mut self) {
        let mut area = 0.0;
        let mut f = self.faces[F_HEAD].next;

        while f != F_HEAD {
            let start = self.faces[f].an_edge;
            let mut e = start;

            if self.edges[e].winding > 0 {
                loop {
                    let (o, d) = (self.org(e), self.dst(e));
                    area +=
                        (self.verts[o].s - self.verts[d].s) * (self.verts[o].t + self.verts[d].t);
                    e = self.edges[e].l_next;
                    if e == start {
                        break;
                    }
                }
            }

            f = self.faces[f].next;
        }

        if area < 0.0 {
            let mut v = self.verts[V_HEAD].next;
            while v != V_HEAD {
                self.verts[v].t = -self.verts[v].t;
                v = self.verts[v].next;
            }
        }
    }

    /// projectPolygon with normal (0, 0, 1): s = x, t = y.
    fn project_polygon(&mut self) {
        let mut v = self.verts[V_HEAD].next;

        while v != V_HEAD {
            self.verts[v].s = self.verts[v].coords[0];
            self.verts[v].t = self.verts[v].coords[1];
            v = self.verts[v].next;
        }
    }

    fn render_mesh_indices(&self) -> Vec<usize> {
        let mut out = vec![];
        let mut f = self.faces[F_HEAD].prev;

        while f != F_HEAD {
            if self.faces[f].inside {
                let start = self.faces[f].an_edge;
                let mut e = start;

                loop {
                    out.push(self.verts[self.org(e)].index);
                    e = self.edges[e].l_next;

                    if e == start {
                        break;
                    }
                }
            }

            f = self.faces[f].prev;
        }

        out
    }

    fn render_mesh_points(&self) -> Vec<([f64; 2], Option<usize>)> {
        let mut out = vec![];
        let mut f = self.faces[F_HEAD].prev;

        while f != F_HEAD {
            if self.faces[f].inside {
                let start = self.faces[f].an_edge;
                let mut e = start;

                loop {
                    let v = &self.verts[self.org(e)];
                    out.push((v.data, (v.index != NIL).then_some(v.index)));
                    e = self.edges[e].l_next;

                    if e == start {
                        break;
                    }
                }
            }

            f = self.faces[f].prev;
        }

        out
    }

    fn render_mesh(&self) -> Vec<[f64; 2]> {
        let mut out = vec![];
        let mut f = self.faces[F_HEAD].prev;

        while f != F_HEAD {
            if self.faces[f].inside {
                let start = self.faces[f].an_edge;
                let mut e = start;

                loop {
                    out.push(self.verts[self.org(e)].data);
                    e = self.edges[e].l_next;

                    if e == start {
                        break;
                    }
                }
            }

            f = self.faces[f].prev;
        }

        out
    }
}

fn long_axis(v: &[f64; 3]) -> usize {
    let mut i = 0;
    if v[1].abs() > v[0].abs() {
        i = 1;
    }
    if v[2].abs() > v[i].abs() {
        i = 2;
    }
    i
}

/// X_ITE `Triangle3.triangulatePolygon`: one contour of 3D points with their indices, winding
/// rule ODD, no normal (libtess computes one), combine callback `data [0]`. Returns the indices
/// of the output triangles in the order libtess reports them (three per triangle).
pub fn triangulate_indexed(points: &[([f64; 3], usize)]) -> Vec<usize> {
    let mut tess = Tess::new();
    tess.winding_odd = true;
    tess.indexed = true;
    tess.last_edge = NIL;

    for (p, index) in points {
        let c = p.map(|x| x.clamp(-GLU_TESS_MAX_COORD, GLU_TESS_MAX_COORD));
        tess.add_vertex(c[0], c[1]);
        let o = tess.org(tess.last_edge);
        tess.verts[o].coords = c;
        tess.verts[o].index = *index;
    }

    tess.project_polygon_computed();
    tess.compute_interior();
    tess.tessellate_interior();
    tess.render_mesh_indices()
}

/// X_ITE `PolygonText.triangulatePolygon`: gluTessBeginPolygon, one contour per entry of
/// `contours`, gluTessEndPolygon; returns the vertices of the output triangles in the order libtess
/// reports them (three per triangle).
pub fn triangulate(contours: &[Vec<[f64; 2]>]) -> Vec<[f64; 2]> {
    let mut tess = Tess::new();

    for contour in contours {
        tess.last_edge = NIL;

        for p in contour {
            // gluTessVertex clamps to ±GLU_TESS_MAX_COORD.
            let x = p[0].clamp(-GLU_TESS_MAX_COORD, GLU_TESS_MAX_COORD);
            let y = p[1].clamp(-GLU_TESS_MAX_COORD, GLU_TESS_MAX_COORD);

            tess.add_vertex(x, y);
        }
    }

    tess.project_polygon();
    tess.compute_interior();
    tess.tessellate_interior();
    tess.render_mesh()
}

/// X_ITE `NurbsTrimmedSurface.triangulatePolygon`: winding rule ODD, normal (0, 0, 1), one
/// contour per entry. The vertex data are the points themselves (`gluTessVertex (point, point)`)
/// with an optional `index` property; see [`Tess::nurbs`] for the combine callbacks. Returns the
/// output triangle vertices in libtess order.
pub fn triangulate_points(
    contours: &[Vec<([f64; 2], Option<usize>)>],
) -> Vec<([f64; 2], Option<usize>)> {
    let mut tess = Tess::new();
    tess.winding_odd = true;
    tess.nurbs = true;

    for contour in contours {
        tess.last_edge = NIL;

        for (p, index) in contour {
            let x = p[0].clamp(-GLU_TESS_MAX_COORD, GLU_TESS_MAX_COORD);
            let y = p[1].clamp(-GLU_TESS_MAX_COORD, GLU_TESS_MAX_COORD);

            tess.add_vertex(x, y);
            let o = tess.org(tess.last_edge);
            tess.verts[o].data = [p[0], p[1]];
            tess.verts[o].index = index.unwrap_or(NIL);
        }
    }

    tess.project_polygon();
    tess.compute_interior();
    tess.tessellate_interior();
    tess.render_mesh_points()
}

/// X_ITE `SVGParser.triangulatePolygon`: winding rule NONZERO or ODD, normal (0, 0, 1), one contour
/// per entry with the vertex data being the point's index; the combine callback appends a vertex
/// (returned in the second list) and hands out the next index, starting at `next_index`. Returns
/// the indices of the output triangles in libtess order.
pub fn triangulate_svg(
    contours: &[Vec<([f64; 2], usize)>],
    winding_odd: bool,
    next_index: usize,
) -> (Vec<usize>, Vec<[f64; 3]>) {
    let mut tess = Tess::new();

    tess.winding_odd = winding_odd;
    tess.svg = true;
    tess.svg_next = next_index;

    for contour in contours {
        tess.last_edge = NIL;

        for (p, index) in contour {
            let x = p[0].clamp(-GLU_TESS_MAX_COORD, GLU_TESS_MAX_COORD);
            let y = p[1].clamp(-GLU_TESS_MAX_COORD, GLU_TESS_MAX_COORD);

            tess.add_vertex(x, y);
            let o = tess.org(tess.last_edge);
            tess.verts[o].index = *index;
        }
    }

    tess.project_polygon();
    tess.compute_interior();
    tess.tessellate_interior();

    let indices = tess.render_mesh_indices();

    (indices, std::mem::take(&mut tess.svg_new))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(t: &[[f64; 2]]) -> f64 {
        t.chunks(3)
            .map(|c| {
                ((c[1][0] - c[0][0]) * (c[2][1] - c[0][1])
                    - (c[2][0] - c[0][0]) * (c[1][1] - c[0][1]))
                    / 2.0
            })
            .sum()
    }

    #[test]
    fn square_is_two_triangles() {
        let t = triangulate(&[vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]]);
        assert_eq!(t.len(), 6);
        assert!((area(&t).abs() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn square_with_hole() {
        let outer = vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]];
        let hole = vec![[1.0, 1.0], [1.0, 3.0], [3.0, 3.0], [3.0, 1.0]];
        let t = triangulate(&[outer, hole]);
        assert_eq!(t.len() / 3, 8);
        assert!((area(&t).abs() - 12.0).abs() < 1e-9);
    }
}
