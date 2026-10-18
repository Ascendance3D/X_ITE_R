//! btDbvt of Bullet 2.82 (BulletCollision/BroadphaseCollision/btDbvt.cpp, zlib license, Nathanael
//! Presson): the dynamic bounding volume tree that backs btDbvtBroadphase and the compound shapes.
//!
//! One deliberate deviation: `sort()` in `optimizeIncremental` compares node *addresses*
//! (`p > n`). The address of a btDbvtNode depends on the allocator of the host (dlmalloc in
//! ammo.js's heap). Here an address is a counter value from a private bump allocator with a LIFO
//! free list, which reproduces the order for trees that are only grown and shrunk through the
//! public insert/remove/update calls on an otherwise quiet heap.

use super::math::*;

pub const NIL: usize = usize::MAX;

/// btDbvtAabbMm.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub mi: V3,
    pub mx: V3,
}

impl Aabb {
    pub fn from_mm(mi: V3, mx: V3) -> Aabb {
        Aabb { mi: r3(mi), mx: r3(mx) }
    }

    pub fn center(&self) -> V3 {
        scale(add(self.mi, self.mx), 0.5)
    }

    pub fn contain(&self, a: &Aabb) -> bool {
        self.mi[0] <= a.mi[0] && self.mi[1] <= a.mi[1] && self.mi[2] <= a.mi[2] && self.mx[0] >= a.mx[0] && self.mx[1] >= a.mx[1] && self.mx[2] >= a.mx[2]
    }

    /// Expand (e): mi -= e, mx += e.
    pub fn expand(&mut self, e: V3) {
        self.mi = r3(sub(self.mi, e));
        self.mx = r3(add(self.mx, e));
    }

    pub fn signed_expand(&mut self, e: V3) {
        for i in 0..3 {
            if e[i] > 0.0 {
                self.mx[i] = f(self.mx[i] + e[i]);
            } else {
                self.mi[i] = f(self.mi[i] + e[i]);
            }
        }
    }
}

/// Intersect (a, b).
pub fn intersect(a: &Aabb, b: &Aabb) -> bool {
    a.mi[0] <= b.mx[0] && a.mx[0] >= b.mi[0] && a.mi[1] <= b.mx[1] && a.mx[1] >= b.mi[1] && a.mi[2] <= b.mx[2] && a.mx[2] >= b.mi[2]
}

/// Proximity (a, b).
fn proximity(a: &Aabb, b: &Aabb) -> f64 {
    let d = sub(add(a.mi, a.mx), add(b.mi, b.mx));
    d[0].abs() + d[1].abs() + d[2].abs()
}

/// Select (o, a, b), generic implementation.
fn select(o: &Aabb, a: &Aabb, b: &Aabb) -> usize {
    if proximity(o, a) < proximity(o, b) { 0 } else { 1 }
}

/// Merge (a, b).
pub fn merge(a: &Aabb, b: &Aabb) -> Aabb {
    let mut r = *a;
    for i in 0..3 {
        r.mi[i] = if a.mi[i] < b.mi[i] { a.mi[i] } else { b.mi[i] };
        r.mx[i] = if a.mx[i] > b.mx[i] { a.mx[i] } else { b.mx[i] };
    }
    r
}

fn not_equal(a: &Aabb, b: &Aabb) -> bool {
    a.mi != b.mi || a.mx != b.mx
}

#[derive(Clone, Debug)]
pub struct Node {
    pub volume: Aabb,
    pub parent: usize,
    /// Internal node: both children; leaf: `childs[1] == NIL`.
    pub childs: [usize; 2],
    /// Leaf payload (`dataAsInt`, a compound child index or a broadphase proxy id).
    pub data: usize,
    addr: u64,
}

impl Node {
    pub fn is_leaf(&self) -> bool {
        self.childs[1] == NIL
    }
}

#[derive(Clone, Debug)]
pub struct Dbvt {
    pub nodes: Vec<Node>,
    pub root: usize,
    free_node: usize,
    pub lkhd: i32,
    pub leaves: i32,
    pub opath: u32,
    next_addr: u64,
    free_addrs: Vec<u64>,
    /// Arena slots that can be reused.
    spare: Vec<usize>,
}

impl Default for Dbvt {
    fn default() -> Self {
        Dbvt::new()
    }
}

impl Dbvt {
    pub fn new() -> Dbvt {
        Dbvt { nodes: vec![], root: NIL, free_node: NIL, lkhd: -1, leaves: 0, opath: 0, next_addr: 1, free_addrs: vec![], spare: vec![] }
    }

    pub fn empty(&self) -> bool {
        self.root == NIL
    }

    fn indexof(&self, n: usize) -> usize {
        usize::from(self.nodes[self.nodes[n].parent].childs[1] == n)
    }

    fn alloc_addr(&mut self) -> u64 {
        self.free_addrs.pop().unwrap_or_else(|| {
            let a = self.next_addr;
            self.next_addr += 1;
            a
        })
    }

    fn new_slot(&mut self, node: Node) -> usize {
        match self.spare.pop() {
            Some(i) => {
                self.nodes[i] = node;
                i
            }
            None => {
                self.nodes.push(node);
                self.nodes.len() - 1
            }
        }
    }

    /// createnode (pdbvt, parent, data) with the single cached free node.
    fn createnode(&mut self, parent: usize, volume: Aabb, data: usize) -> usize {
        if self.free_node != NIL {
            let i = self.free_node;
            self.free_node = NIL;
            let addr = self.nodes[i].addr;
            self.nodes[i] = Node { volume, parent, childs: [NIL, NIL], data, addr };
            i
        } else {
            let addr = self.alloc_addr();
            self.new_slot(Node { volume, parent, childs: [NIL, NIL], data, addr })
        }
    }

    /// deletenode: free the previously cached node and cache this one.
    fn deletenode(&mut self, node: usize) {
        if self.free_node != NIL {
            let a = self.nodes[self.free_node].addr;
            self.free_addrs.push(a);
            self.spare.push(self.free_node);
        }
        self.free_node = node;
    }

    fn insertleaf(&mut self, mut root: usize, leaf: usize) {
        if self.root == NIL {
            self.root = leaf;
            self.nodes[leaf].parent = NIL;
            return;
        }

        if !self.nodes[root].is_leaf() {
            loop {
                let [c0, c1] = self.nodes[root].childs;
                let which = select(&self.nodes[leaf].volume, &self.nodes[c0].volume, &self.nodes[c1].volume);
                root = [c0, c1][which];
                if self.nodes[root].is_leaf() {
                    break;
                }
            }
        }

        let prev = self.nodes[root].parent;
        let vol = merge(&self.nodes[leaf].volume, &self.nodes[root].volume);
        let node = self.createnode(prev, vol, 0);
        // createnode (..., volume0, volume1, 0) leaves childs[1] = 0 until set below.

        if prev != NIL {
            let idx = self.indexof(root);
            self.nodes[prev].childs[idx] = node;
            self.nodes[node].childs = [root, leaf];
            self.nodes[root].parent = node;
            self.nodes[leaf].parent = node;
            let mut node = node;
            let mut prev = prev;
            loop {
                if !self.nodes[prev].volume.contain(&self.nodes[node].volume) {
                    let [a, b] = self.nodes[prev].childs;
                    self.nodes[prev].volume = merge(&self.nodes[a].volume, &self.nodes[b].volume);
                } else {
                    break;
                }
                node = prev;
                prev = self.nodes[node].parent;
                if prev == NIL {
                    break;
                }
            }
        } else {
            self.nodes[node].childs = [root, leaf];
            self.nodes[root].parent = node;
            self.nodes[leaf].parent = node;
            self.root = node;
        }
    }

    fn removeleaf(&mut self, leaf: usize) -> usize {
        if leaf == self.root {
            self.root = NIL;
            return NIL;
        }

        let parent = self.nodes[leaf].parent;
        let prev = self.nodes[parent].parent;
        let sibling = self.nodes[parent].childs[1 - self.indexof(leaf)];

        if prev != NIL {
            let idx = self.indexof(parent);
            self.nodes[prev].childs[idx] = sibling;
            self.nodes[sibling].parent = prev;
            self.deletenode(parent);
            let mut prev = prev;
            while prev != NIL {
                let pb = self.nodes[prev].volume;
                let [a, b] = self.nodes[prev].childs;
                self.nodes[prev].volume = merge(&self.nodes[a].volume, &self.nodes[b].volume);
                if not_equal(&pb, &self.nodes[prev].volume) {
                    prev = self.nodes[prev].parent;
                } else {
                    break;
                }
            }
            if prev != NIL { prev } else { self.root }
        } else {
            self.root = sibling;
            self.nodes[sibling].parent = NIL;
            self.deletenode(parent);
            self.root
        }
    }

    /// btDbvt::insert (volume, data); returns the leaf.
    pub fn insert(&mut self, volume: Aabb, data: usize) -> usize {
        let leaf = self.createnode(NIL, volume, data);
        let root = self.root;
        self.insertleaf(root, leaf);
        self.leaves += 1;
        leaf
    }

    /// btDbvt::update (leaf, volume).
    pub fn update(&mut self, leaf: usize, volume: Aabb) {
        let mut root = self.removeleaf(leaf);
        if root != NIL {
            if self.lkhd >= 0 {
                let mut i = 0;
                while i < self.lkhd && self.nodes[root].parent != NIL {
                    root = self.nodes[root].parent;
                    i += 1;
                }
            } else {
                root = self.root;
            }
        }
        self.nodes[leaf].volume = volume;
        self.insertleaf(root, leaf);
    }

    /// btDbvt::update (leaf, volume, velocity, margin).
    pub fn update_margin(&mut self, leaf: usize, mut volume: Aabb, velocity: V3, margin: f64) -> bool {
        if self.nodes[leaf].volume.contain(&volume) {
            return false;
        }
        volume.expand([margin; 3]);
        volume.signed_expand(velocity);
        self.update(leaf, volume);
        true
    }

    /// btDbvt::update (leaf, volume, velocity).
    pub fn update_velocity(&mut self, leaf: usize, mut volume: Aabb, velocity: V3) -> bool {
        if self.nodes[leaf].volume.contain(&volume) {
            return false;
        }
        volume.signed_expand(velocity);
        self.update(leaf, volume);
        true
    }

    /// btDbvt::update (leaf, lookahead).
    fn update_lookahead(&mut self, leaf: usize, lookahead: i32) {
        let mut root = self.removeleaf(leaf);
        if root != NIL {
            if lookahead >= 0 {
                let mut i = 0;
                while i < lookahead && self.nodes[root].parent != NIL {
                    root = self.nodes[root].parent;
                    i += 1;
                }
            } else {
                root = self.root;
            }
        }
        self.insertleaf(root, leaf);
    }

    /// btDbvt::remove.
    pub fn remove(&mut self, leaf: usize) {
        self.removeleaf(leaf);
        self.deletenode(leaf);
        self.leaves -= 1;
    }

    /// The `sort (n, r)` helper of optimizeIncremental.
    fn sort(&mut self, n: usize) -> usize {
        let p = self.nodes[n].parent;

        if p != NIL && self.nodes[p].addr > self.nodes[n].addr {
            let i = self.indexof(n);
            let j = 1 - i;
            let s = self.nodes[p].childs[j];
            let q = self.nodes[p].parent;

            if q != NIL {
                let idx = self.indexof(p);
                self.nodes[q].childs[idx] = n;
            } else {
                self.root = n;
            }

            self.nodes[s].parent = n;
            self.nodes[p].parent = n;
            self.nodes[n].parent = q;
            let nc = self.nodes[n].childs;
            self.nodes[p].childs = nc;
            self.nodes[nc[0]].parent = p;
            self.nodes[nc[1]].parent = p;
            self.nodes[n].childs[i] = p;
            self.nodes[n].childs[j] = s;
            let (pv, nv) = (self.nodes[p].volume, self.nodes[n].volume);
            self.nodes[p].volume = nv;
            self.nodes[n].volume = pv;
            return p;
        }

        n
    }

    /// btDbvt::optimizeIncremental.
    pub fn optimize_incremental(&mut self, passes: i32) {
        let mut passes = if passes < 0 { self.leaves } else { passes };

        if self.root != NIL && passes > 0 {
            loop {
                let mut node = self.root;
                let mut bit: u32 = 0;

                while !self.nodes[node].is_leaf() {
                    let s = self.sort(node);
                    node = self.nodes[s].childs[((self.opath >> bit) & 1) as usize];
                    bit = (bit + 1) & 31;
                }

                self.update_lookahead(node, -1);
                self.opath = self.opath.wrapping_add(1);
                passes -= 1;
                if passes == 0 {
                    break;
                }
            }
        }
    }

    /// btDbvt::collideTV: leaves whose volume intersects `volume`, in stack order.
    pub fn collide_tv(&self, root: usize, volume: &Aabb, out: &mut Vec<usize>) {
        if root == NIL {
            return;
        }

        let mut stack = vec![root];
        while let Some(n) = stack.pop() {
            if intersect(&self.nodes[n].volume, volume) {
                if !self.nodes[n].is_leaf() {
                    stack.push(self.nodes[n].childs[0]);
                    stack.push(self.nodes[n].childs[1]);
                } else {
                    out.push(n);
                }
            }
        }
    }
}

/// btDbvt::collideTTpersistentStack / collideTT on two (possibly different) trees, emitting leaf
/// pairs (leaf of tree0, leaf of tree1) in policy order. `same` is true when both roots belong to
/// the same tree and may be the same node (the `p.a == p.b` case).
pub fn collide_tt(t0: &Dbvt, root0: usize, t1: &Dbvt, root1: usize, same_tree: bool, out: &mut Vec<(usize, usize)>) {
    if root0 == NIL || root1 == NIL {
        return;
    }

    let mut stack: Vec<(usize, usize)> = vec![(root0, root1)];

    while let Some((a, b)) = stack.pop() {
        if same_tree && a == b {
            if !t0.nodes[a].is_leaf() {
                let c = t0.nodes[a].childs;
                stack.push((c[0], c[0]));
                stack.push((c[1], c[1]));
                stack.push((c[0], c[1]));
            }
        } else if intersect(&t0.nodes[a].volume, &t1.nodes[b].volume) {
            let (la, lb) = (t0.nodes[a].is_leaf(), t1.nodes[b].is_leaf());
            if !la {
                let ca = t0.nodes[a].childs;
                if !lb {
                    let cb = t1.nodes[b].childs;
                    stack.push((ca[0], cb[0]));
                    stack.push((ca[1], cb[0]));
                    stack.push((ca[0], cb[1]));
                    stack.push((ca[1], cb[1]));
                } else {
                    stack.push((ca[0], b));
                    stack.push((ca[1], b));
                }
            } else if !lb {
                let cb = t1.nodes[b].childs;
                stack.push((a, cb[0]));
                stack.push((a, cb[1]));
            } else {
                out.push((a, b));
            }
        }
    }
}
