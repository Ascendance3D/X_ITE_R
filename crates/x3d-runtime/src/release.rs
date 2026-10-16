//! R0.3-D: the nodes of a disposed scene are released (UNBOUNDED_NODE_ARENA_RETENTION).
//!
//! X_ITE leaves it to the JavaScript garbage collector: a scene nobody refers to is collected, nodes
//! and all. This arena has no collector, so the runtime does what the collector would do, once per
//! frame and only for what [`Runtime::dispose_scene`] has given up:
//!
//! * The *candidates* are the nodes of the contexts that a disposed scene (or a replaced PROTO
//!   instance body) held, and the `X3DScene` objects that carried the `loadCount` of finished loads.
//! * A candidate that something outside the disposed contexts still refers to *stays*, and so does
//!   everything it refers to (DEF/USE identity: a node that a live field, a live route, a live
//!   context or a live PROTO declaration holds is not taken away, wherever it was parsed). A candidate
//!   that stays is looked at again at the next sweep: a route that is reconnected, or a field that is
//!   set to another value, lets it go.
//! * Every other candidate is released: its slot is emptied and may be handed out again by the next
//!   scene (`Scene::merge`) or node (`Scene::alloc_node`), and every table that mentions the node
//!   forgets it — fields, interests, queued events and tasks, the scripts' JavaScript wrappers.
//!
//! The sweep runs outside of event processing, at the end of a frame (see `finish_frame`), so no
//! iteration over a table that it edits is in progress. A node with an event that waits to be
//! processed stays until that event is delivered (the events a disposed Script's `shutdown ()` sent are
//! processed with the next frame, like any other event, and the sweep does not move them).
//!
//! What this does not release: execution contexts, PROTO and EXTERNPROTO declarations and the IMPORT
//! and EXPORT tables are append-only tables with their own ids. A disposed context is emptied (its
//! node, route and name tables are cleared) but its slot stays; the same goes for the ids in
//! `disposed_scenes` and `dead_ctxs`. This is a few hundred bytes per scene, reported as an open limit.
//! The entries such a context leaves in the scene-wide tables are ignored from then on
//! (`emptied_ctxs`): an EXPORT's local node or an IMPORT's Inline names a slot that may be reused.

use crate::*;
use std::collections::{HashMap, HashSet};

/// What one sweep did (see [`Runtime::release_disposed`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReleaseStats {
    /// Nodes whose slot was emptied.
    pub released: usize,
    /// Candidates that something live still refers to; they are looked at again next time.
    pub retained: usize,
}

/// Node references in a field value.
fn node_refs(value: &FieldValue) -> Vec<NodeId> {
    match value {
        FieldValue::SFNode(Some(n)) => vec![*n],
        FieldValue::MFNode(list) => list.iter().flatten().copied().collect(),
        _ => vec![],
    }
}

fn all_fields(node: &Node) -> impl Iterator<Item = &Field> {
    node.fields.iter().chain(node.user_fields.iter()).chain(node.private_fields.iter())
}

impl Runtime {
    /// A scene (or a PROTO body) was disposed: its nodes and contexts are released at the next sweep.
    pub(crate) fn queue_release(&mut self, ctxs: &[CtxId], nodes: &[NodeId]) {
        self.release_ctxs.extend_from_slice(ctxs);
        self.release_queue.extend(nodes.iter().copied().filter(|&n| self.scene.is_live(n) && !matches!(self.scene.node(n).kind, NodeKind::Internal(_))));
    }

    /// The nodes released since the last call, for the host's own tables (bindable stacks, caches):
    /// their ids may be handed out again.
    pub fn take_released(&mut self) -> Vec<NodeId> {
        std::mem::take(&mut self.released_log)
    }

    /// How many nodes are waiting for a sweep (queued candidates, including those that stay for now).
    pub fn release_pending(&self) -> usize {
        self.release_queue.len()
    }

    /// Release what the disposed scenes held and nothing else refers to. Does nothing while events
    /// are being processed. See the module header for the rules.
    pub fn release_disposed(&mut self) -> ReleaseStats {
        let mut stats = ReleaseStats::default();

        if self.processing {
            return stats;
        }

        // The `X3DScene` objects of loads that are over carry nothing: they are candidates as soon as
        // no pending load needs them.
        let needed: HashSet<NodeId> = self.pending_scenes.iter().map(|p| p.scene_node).chain(self.pending_externs.iter().map(|p| p.scene_node)).collect();
        let loose: Vec<NodeId> = (0..self.scene.nodes.len() as NodeId)
            .filter(|n| matches!(self.scene.node(*n).kind, NodeKind::Internal("X3DScene")) && !needed.contains(n))
            .collect();

        self.release_queue.extend(loose);

        if self.release_queue.is_empty() {
            return stats;
        }

        let mut candidates: HashSet<NodeId> = std::mem::take(&mut self.release_queue).into_iter().filter(|&n| self.scene.is_live(n)).collect();
        let dead_ctxs: HashSet<CtxId> = self.release_ctxs.iter().copied().collect();

        // A context holds nothing once it is disposed: while it waits for its nodes (`dead_ctxs`), and
        // after it was emptied, when it has left `release_ctxs` but its PROTO, EXTERNPROTO, IMPORT and
        // EXPORT entries are still in the scene's tables, naming slots that other scenes may have taken.
        let emptied = &self.emptied_ctxs;
        let gone = |c: CtxId| dead_ctxs.contains(&c) || emptied.contains(&c);

        // --- what stays ---------------------------------------------------------------------------

        let mut keep: HashSet<NodeId> = HashSet::new();
        let note = |keep: &mut HashSet<NodeId>, n: NodeId| {
            if candidates.contains(&n) {
                keep.insert(n);
            }
        };

        // Fields of nodes that are not candidates.
        for (id, node) in self.scene.nodes.iter().enumerate() {
            if !self.scene.is_live(id as NodeId) || candidates.contains(&(id as NodeId)) {
                continue;
            }

            for f in all_fields(node) {
                for n in node_refs(&f.value) {
                    note(&mut keep, n);
                }
            }
        }

        // Events that wait to be processed (what `shutdown ()` of a disposed Script sent to a live node
        // is among them) are delivered first: their fields are read when they are processed.
        for (k, e) in &self.tainted_fields {
            note(&mut keep, k.0);
            note(&mut keep, e.origin.0);
        }

        for &n in &self.tainted_nodes {
            note(&mut keep, n);
        }

        // Live contexts: root nodes, names, routes.
        for (c, ctx) in self.scene.contexts.iter().enumerate() {
            if gone(c) {
                continue;
            }

            for n in ctx.root_nodes.iter().flatten().chain(ctx.named_nodes.values().iter()) {
                note(&mut keep, *n);
            }

            for r in &ctx.routes {
                note(&mut keep, r.source_node);
                note(&mut keep, r.destination_node);

                if let Some(((s, _), (d, _))) = r.conn {
                    note(&mut keep, s);
                    note(&mut keep, d);
                }
            }
        }

        // Live PROTO and EXTERNPROTO declarations (default values), IMPORT and EXPORT.
        for p in self.scene.protos.iter().filter(|p| !gone(p.ctx)) {
            for f in &p.fields {
                for n in node_refs(&f.value) {
                    note(&mut keep, n);
                }
            }
        }

        for e in self.scene.externprotos.iter().filter(|e| !gone(e.ctx)) {
            for f in &e.fields {
                for n in node_refs(&f.value) {
                    note(&mut keep, n);
                }
            }
        }

        for i in self.scene.imported.iter().filter(|i| !gone(i.ctx)) {
            note(&mut keep, i.inline);
            note(&mut keep, i.instance);
        }

        for e in self.scene.exported.iter().filter(|e| !gone(e.ctx)) {
            note(&mut keep, e.local);
        }

        // What a node that stays refers to stays too: its children, the body of an instance.
        let mut by_ctx: HashMap<CtxId, Vec<NodeId>> = HashMap::new();

        for &n in &candidates {
            by_ctx.entry(self.scene.node(n).ctx).or_default().push(n);
        }

        let mut work: Vec<NodeId> = keep.iter().copied().collect();

        while let Some(n) = work.pop() {
            let mut next: Vec<NodeId> = all_fields(self.scene.node(n)).flat_map(|f| node_refs(&f.value)).collect();

            if let Some(body) = self.instance_bodies.get(&n) {
                for (c, list) in &by_ctx {
                    if self.ctx_within(*c, *body) {
                        next.extend(list.iter().copied());
                    }
                }
            }

            for m in next {
                if candidates.contains(&m) && keep.insert(m) {
                    work.push(m);
                }
            }
        }

        // --- the release --------------------------------------------------------------------------

        let release: HashSet<NodeId> = candidates.iter().copied().filter(|n| !keep.contains(n)).collect();

        stats.retained = keep.len();
        candidates.retain(|n| keep.contains(n));
        self.release_queue = candidates.into_iter().collect();
        self.release_queue.sort_unstable();

        // A disposed context is emptied once none of its nodes stays.
        let kept_ctxs: HashSet<CtxId> = self.release_queue.iter().map(|&n| self.scene.node(n).ctx).collect();
        let empty: Vec<CtxId> = dead_ctxs.iter().copied().filter(|c| !kept_ctxs.contains(c)).collect();

        self.release_ctxs.retain(|c| kept_ctxs.contains(c));

        if release.is_empty() && empty.is_empty() {
            return stats;
        }

        self.forget_nodes(&release);

        for &n in &release {
            self.scene.release_node(n);
            self.nrt_mut(n);
            self.nrt[n as usize] = NodeRt::default();
            self.touch(n);
        }

        for &c in &empty {
            self.empty_context(c);
        }

        self.rebuild_parents();

        let mut ids: Vec<NodeId> = release.into_iter().collect();

        ids.sort_unstable();
        stats.released = ids.len();
        self.released_total += ids.len() as u64;
        self.release_epoch += 1;
        self.released_log.extend_from_slice(&ids);
        stats
    }

    /// Remove every mention of the nodes in `dead` from the runtime's tables.
    fn forget_nodes(&mut self, dead: &HashSet<NodeId>) {
        if dead.is_empty() {
            return;
        }

        self.frt.retain(|k, _| !dead.contains(&k.0));

        for f in self.frt.values_mut() {
            f.interests.retain(|i| !dead.contains(&i.node));
            f.field_interests.retain(|k| !dead.contains(&k.0));
            f.references.retain(|k| !dead.contains(&k.0));
        }

        for n in &mut self.nrt {
            n.interests.retain(|i| !dead.contains(&i.node));
        }

        self.tainted_fields.retain(|(k, e)| !dead.contains(&k.0) && !dead.contains(&e.origin.0));
        self.tainted_nodes.retain(|n| !dead.contains(n));

        let alive = |t: &Task| match t {
            Task::ScriptLoad(n) | Task::ScriptLoaded(n, _) | Task::UrlLoad(n, _) | Task::ScriptFetch(n) => !dead.contains(n),
            Task::ExternLoad(..) => true,
        };

        self.tasks.retain(alive);
        self.fetches.retain(alive);
        self.timeouts.retain(|t| !dead.contains(&t.node));
        self.bind_modifications.retain(|n, _| !dead.contains(n));
        self.instance_update.retain(|n| !dead.contains(n));
        self.instance_bodies.retain(|n, _| !dead.contains(n));
        self.inline_scenes.retain(|n, _| !dead.contains(n));
        self.live_watchers.retain(|(n, _)| !dead.contains(n));
        self.followers.retain(|n| !dead.contains(n));
        self.scripts.retain(|n| !dead.contains(n));
        self.pending_scenes.retain(|p| !dead.contains(&p.node) && !dead.contains(&p.scene_node));
        self.frame_waiters.retain(|w| !dead.contains(&w.node));
        self.scene.setup_order.retain(|n| !dead.contains(n));

        if let Some(mut host) = self.script_host.take() {
            let tombstone = self.tombstone_node();
            let ids: Vec<NodeId> = dead.iter().copied().collect();

            host.release_nodes(self, &ids, tombstone);
            self.script_host = Some(host);
        }
    }

    /// The node that a Script's stale wrapper of a released node points to: it has no fields.
    fn tombstone_node(&mut self) -> NodeId {
        match self.tombstone {
            Some(n) => n,
            None => {
                let n = self.create_internal_node("X3DReleased");

                self.tombstone = Some(n);
                n
            }
        }
    }

    /// A disposed context keeps its slot and loses its content.
    fn empty_context(&mut self, c: CtxId) {
        let ctx = &mut self.scene.contexts[c];

        ctx.parent = None;
        ctx.outer = Outer::Inline(None);
        ctx.root_nodes = vec![];
        ctx.named_nodes = Default::default();
        ctx.protos = Default::default();
        ctx.externprotos = Default::default();
        ctx.routes = vec![];
        ctx.imported = Default::default();
        ctx.exported = Default::default();

        self.scene.scene_infos.remove(&c);
        self.inline_live.remove(&c);
        self.emptied_ctxs.insert(c);

        for p in self.scene.protos.iter_mut().filter(|p| p.ctx == c) {
            p.fields = vec![];
        }

        for e in self.scene.externprotos.iter_mut().filter(|e| e.ctx == c) {
            e.fields = vec![];
        }
    }
}
