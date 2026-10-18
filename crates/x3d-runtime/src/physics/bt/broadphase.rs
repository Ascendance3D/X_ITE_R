//! btDbvtBroadphase with btHashedOverlappingPairCache (Bullet 2.82, zlib license). The hash table
//! of the pair cache only accelerates lookups, so the pair array is a plain vector with the same
//! swap-with-last removal; the pair order (which decides the manifold and solver order) is
//! identical.

use super::body::BodyId;
use super::dbvt::{collide_tt, intersect, Aabb, Dbvt, NIL};
use super::math::*;

pub type ProxyId = usize;

const STAGECOUNT: usize = 2;
/// DBVT_BP_MARGIN (btScalar)0.05.
const DBVT_BP_MARGIN: f64 = 0.05_f32 as f64;

pub const DEFAULT_FILTER: i16 = 1;
pub const STATIC_FILTER: i16 = 2;
pub const ALL_FILTER: i16 = -1;

#[derive(Clone, Debug)]
pub struct Proxy {
    pub aabb_min: V3,
    pub aabb_max: V3,
    pub leaf: usize,
    /// 0 or 1: dynamic stage, STAGECOUNT: fixed set.
    pub stage: usize,
    pub unique_id: i32,
    pub client: BodyId,
    pub group: i16,
    pub mask: i16,
    links: [Option<ProxyId>; 2],
}

/// btBroadphasePair with its cached collision algorithm.
#[derive(Debug)]
pub struct Pair<A> {
    pub p0: ProxyId,
    pub p1: ProxyId,
    pub algo: Option<Box<A>>,
}

#[derive(Debug)]
pub struct DbvtBroadphase<A> {
    sets: [Dbvt; 2],
    pub proxies: Vec<Option<Proxy>>,
    /// Slots of `proxies` whose proxy was destroyed (a body that leaves the world and comes back must not grow the
    /// table; nothing orders proxies by slot: pairs are ordered by `unique_id`).
    free_proxies: Vec<ProxyId>,
    stage_roots: [Option<ProxyId>; STAGECOUNT + 1],
    stage_current: usize,
    fixed_left: i32,
    fupdates: i32,
    dupdates: i32,
    cupdates: i32,
    newpairs: i32,
    need_cleanup: bool,
    prediction: f64,
    gid: i32,
    cid: i32,
    pub pairs: Vec<Pair<A>>,
    /// Algorithms of removed or cleaned pairs, to be released by the owner.
    pub dropped: Vec<Box<A>>,
}

impl<A> Default for DbvtBroadphase<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A> DbvtBroadphase<A> {
    pub fn new() -> Self {
        DbvtBroadphase {
            sets: [Dbvt::new(), Dbvt::new()],
            proxies: vec![],
            free_proxies: vec![],
            stage_roots: [None; STAGECOUNT + 1],
            stage_current: 0,
            fixed_left: 0,
            fupdates: 1,
            dupdates: 0,
            cupdates: 10,
            newpairs: 1,
            need_cleanup: true,
            prediction: 0.0,
            gid: 0,
            cid: 0,
            pairs: vec![],
            dropped: vec![],
        }
    }

    pub fn proxy(&self, p: ProxyId) -> &Proxy {
        self.proxies[p].as_ref().expect("destroyed proxy")
    }

    fn px_mut(&mut self, p: ProxyId) -> &mut Proxy {
        self.proxies[p].as_mut().expect("destroyed proxy")
    }

    fn listappend(&mut self, p: ProxyId, stage: usize) {
        let head = self.stage_roots[stage];
        self.px_mut(p).links = [None, head];
        if let Some(h) = head {
            self.px_mut(h).links[0] = Some(p);
        }
        self.stage_roots[stage] = Some(p);
    }

    fn listremove(&mut self, p: ProxyId, stage: usize) {
        let [prev, next] = self.proxy(p).links;
        match prev {
            Some(pr) => self.px_mut(pr).links[1] = next,
            None => self.stage_roots[stage] = next,
        }
        if let Some(n) = next {
            self.px_mut(n).links[0] = prev;
        }
    }

    // --- btHashedOverlappingPairCache ---

    fn needs_broadphase_collision(&self, p0: ProxyId, p1: ProxyId) -> bool {
        let (a, b) = (self.proxy(p0), self.proxy(p1));
        let collides = (a.group & b.mask) != 0;
        collides && (b.group & a.mask) != 0
    }

    fn find_pair_index(&self, p0: ProxyId, p1: ProxyId) -> Option<usize> {
        self.pairs.iter().position(|p| p.p0 == p0 && p.p1 == p1)
    }

    /// addOverlappingPair.
    pub fn add_overlapping_pair(&mut self, p0: ProxyId, p1: ProxyId) {
        if !self.needs_broadphase_collision(p0, p1) {
            return;
        }

        let (mut p0, mut p1) = (p0, p1);
        if self.proxy(p0).unique_id > self.proxy(p1).unique_id {
            std::mem::swap(&mut p0, &mut p1);
        }

        if self.find_pair_index(p0, p1).is_some() {
            return;
        }

        self.pairs.push(Pair { p0, p1, algo: None });
    }

    /// removeOverlappingPair (cleans the algorithm, then swaps the last pair into the slot).
    pub fn remove_overlapping_pair(&mut self, p0: ProxyId, p1: ProxyId) {
        let (mut p0, mut p1) = (p0, p1);
        if self.proxy(p0).unique_id > self.proxy(p1).unique_id {
            std::mem::swap(&mut p0, &mut p1);
        }

        if let Some(i) = self.find_pair_index(p0, p1) {
            let mut pair = self.pairs.swap_remove(i);
            if let Some(a) = pair.algo.take() {
                self.dropped.push(a);
            }
        }
    }

    /// cleanProxyFromPairs: free the cached algorithms of the pairs that contain the proxy.
    pub fn clean_proxy_from_pairs(&mut self, proxy: ProxyId) {
        for i in 0..self.pairs.len() {
            if self.pairs[i].p0 == proxy || self.pairs[i].p1 == proxy {
                if let Some(a) = self.pairs[i].algo.take() {
                    self.dropped.push(a);
                }
            }
        }
    }

    /// removeOverlappingPairsContainingProxy (processAllOverlappingPairs with a removing callback).
    fn remove_pairs_containing_proxy(&mut self, proxy: ProxyId) {
        let mut i = 0;
        while i < self.pairs.len() {
            if self.pairs[i].p0 == proxy || self.pairs[i].p1 == proxy {
                let (a, b) = (self.pairs[i].p0, self.pairs[i].p1);
                self.remove_overlapping_pair(a, b);
            } else {
                i += 1;
            }
        }
    }

    // --- btDbvtBroadphase ---

    /// createProxy.
    pub fn create_proxy(&mut self, aabb_min: V3, aabb_max: V3, client: BodyId, group: i16, mask: i16) -> ProxyId {
        let aabb = Aabb::from_mm(aabb_min, aabb_max);
        self.gid += 1;
        let proxy = Some(Proxy { aabb_min: r3(aabb_min), aabb_max: r3(aabb_max), leaf: NIL, stage: self.stage_current, unique_id: self.gid, client, group, mask, links: [None, None] });
        let id = match self.free_proxies.pop() {
            Some(id) => {
                self.proxies[id] = proxy;
                id
            }
            None => {
                self.proxies.push(proxy);
                self.proxies.len() - 1
            }
        };
        let leaf = self.sets[0].insert(aabb, id);
        self.px_mut(id).leaf = leaf;
        let stage = self.stage_current;
        self.listappend(id, stage);

        // m_deferedcollide is false.
        let mut found0 = vec![];
        let mut found1 = vec![];
        let root0 = self.sets[0].root;
        self.sets[0].collide_tv(root0, &aabb, &mut found0);
        let root1 = self.sets[1].root;
        self.sets[1].collide_tv(root1, &aabb, &mut found1);

        for (k, nodes) in [found0, found1].into_iter().enumerate() {
            for n in nodes {
                let other = self.sets[k].nodes[n].data;
                // btDbvtTreeCollider::Process (na, nb): na != nb
                if !(k == 0 && n == leaf) {
                    self.add_overlapping_pair(other, id);
                    self.newpairs += 1;
                }
            }
        }

        id
    }

    /// destroyProxy.
    pub fn destroy_proxy(&mut self, id: ProxyId) {
        let (stage, leaf) = {
            let p = self.proxy(id);
            (p.stage, p.leaf)
        };

        if stage == STAGECOUNT {
            self.sets[1].remove(leaf);
        } else {
            self.sets[0].remove(leaf);
        }

        self.listremove(id, stage);
        self.remove_pairs_containing_proxy(id);
        self.proxies[id] = None;
        self.free_proxies.push(id);
        self.need_cleanup = true;
    }

    /// setAabb.
    pub fn set_aabb(&mut self, id: ProxyId, aabb_min: V3, aabb_max: V3) {
        let aabb = Aabb::from_mm(aabb_min, aabb_max);
        let mut docollide = false;
        let (stage, leaf) = {
            let p = self.proxy(id);
            (p.stage, p.leaf)
        };

        if stage == STAGECOUNT {
            // fixed -> dynamic set
            self.sets[1].remove(leaf);
            let nl = self.sets[0].insert(aabb, id);
            self.px_mut(id).leaf = nl;
            docollide = true;
        } else {
            // dynamic set
            if intersect(&self.sets[0].nodes[leaf].volume, &aabb) {
                // Moving
                let p = self.proxy(id);
                let delta = sub(aabb_min, p.aabb_min);
                let mut velocity = scale(r3(scale(sub(p.aabb_max, p.aabb_min), 0.5)), self.prediction);
                if delta[0] < 0.0 {
                    velocity[0] = -velocity[0];
                }
                if delta[1] < 0.0 {
                    velocity[1] = -velocity[1];
                }
                if delta[2] < 0.0 {
                    velocity[2] = -velocity[2];
                }
                if self.sets[0].update_margin(leaf, aabb, r3(velocity), DBVT_BP_MARGIN) {
                    docollide = true;
                }
            } else {
                // Teleporting
                self.sets[0].update(leaf, aabb);
                docollide = true;
            }
        }

        self.listremove(id, stage);
        {
            let cur = self.stage_current;
            let p = self.px_mut(id);
            p.aabb_min = r3(aabb_min);
            p.aabb_max = r3(aabb_max);
            p.stage = cur;
        }
        let cur = self.stage_current;
        self.listappend(id, cur);

        if docollide {
            self.need_cleanup = true;
            let leaf = self.proxy(id).leaf;
            let mut found = vec![];
            // m_sets[1].collideTTpersistentStack (m_sets[1].m_root, proxy->leaf, collider)
            let (r1, r0) = (self.sets[1].root, self.sets[0].root);
            collide_tt(&self.sets[1], r1, &self.sets[0], leaf, false, &mut found);
            let n1 = found.len();
            collide_tt(&self.sets[0], r0, &self.sets[0], leaf, true, &mut found);

            for (i, (na, nb)) in found.into_iter().enumerate() {
                let set_a = if i < n1 { 1 } else { 0 };
                if !(set_a == 0 && na == nb) {
                    let pa = self.sets[set_a].nodes[na].data;
                    let pb = self.sets[0].nodes[nb].data;
                    self.add_overlapping_pair(pa, pb);
                    self.newpairs += 1;
                }
            }
        }
    }

    /// collide (called by calculateOverlappingPairs).
    pub fn calculate_overlapping_pairs(&mut self) {
        // optimize
        let passes0 = 1 + (self.sets[0].leaves * self.dupdates) / 100;
        self.sets[0].optimize_incremental(passes0);

        if self.fixed_left != 0 {
            let count = 1 + (self.sets[1].leaves * self.fupdates) / 100;
            self.sets[1].optimize_incremental(1 + (self.sets[1].leaves * self.fupdates) / 100);
            self.fixed_left = (self.fixed_left - count).max(0);
        }

        // dynamic -> fixed set
        self.stage_current = (self.stage_current + 1) % STAGECOUNT;
        let mut current = self.stage_roots[self.stage_current];

        if current.is_some() {
            while let Some(c) = current {
                let next = self.proxy(c).links[1];
                let stage = self.proxy(c).stage;
                self.listremove(c, stage);
                self.listappend(c, STAGECOUNT);
                let leaf = self.proxy(c).leaf;
                self.sets[0].remove(leaf);
                let cur_aabb = Aabb::from_mm(self.proxy(c).aabb_min, self.proxy(c).aabb_max);
                let nl = self.sets[1].insert(cur_aabb, c);
                let p = self.px_mut(c);
                p.leaf = nl;
                p.stage = STAGECOUNT;
                current = next;
            }

            self.fixed_left = self.sets[1].leaves;
            self.need_cleanup = true;
        }

        // clean up
        if self.need_cleanup && !self.pairs.is_empty() {
            let n = self.pairs.len() as i32;
            let mut ni = n.min(self.newpairs.max((n * self.cupdates) / 100));
            let mut i = 0;

            while i < ni {
                let len = self.pairs.len() as i32;
                let idx = ((self.cid + i) % len) as usize;
                let (pa, pb) = (self.pairs[idx].p0, self.pairs[idx].p1);
                let (la, lb) = {
                    let (a, b) = (self.proxy(pa), self.proxy(pb));
                    (self.leaf_volume(a), self.leaf_volume(b))
                };

                if !intersect(&la, &lb) {
                    self.remove_overlapping_pair(pa, pb);
                    ni -= 1;
                    i -= 1;
                }

                i += 1;
            }

            if !self.pairs.is_empty() {
                self.cid = (self.cid + ni) % self.pairs.len() as i32;
            } else {
                self.cid = 0;
            }
        }

        self.newpairs = 1;
        self.need_cleanup = false;
    }

    fn leaf_volume(&self, p: &Proxy) -> Aabb {
        let set = if p.stage == STAGECOUNT { 1 } else { 0 };
        self.sets[set].nodes[p.leaf].volume
    }
}
