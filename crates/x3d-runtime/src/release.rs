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
//! A disposed context is emptied once none of its nodes stays (its node, route and name tables are
//! cleared). The entries it leaves in the scene-wide tables are ignored from then on (`emptied_ctxs`):
//! an EXPORT's local node or an IMPORT's Inline names a node slot that may be reused.
//!
//! # Runtime-created nodes (REA batch 06R repair)
//!
//! Nodes a Script creates (`createNode`, `createVrmlFromString`, `new SFNode (…)`) are in no disposed
//! scene: a ParticleSystem removed from a live Group's `children` was never a candidate, kept animating
//! (`animateParticles` interest, live watcher, buffers) and its seven nodes, 54 field records and
//! ~32 KiB were never given back. X_ITE holds interests as `WeakRef`s (`X3DObject.js:53,69,85`,
//! `X3DBaseNode.js:187-199`, `ParticleSystem.js:260-281`), so an unreachable node is collected and stops.
//!
//! The runtime lists them in `dynamic_nodes` (and the context of a `createVrmlFromString` scene in
//! `dynamic_ctxs`) and makes them *candidates* of the same sweep, with the same keep rules, when:
//!
//! * a node-valued field was written, a ROUTE deleted, a Script callback ran, or 30 frames passed
//!   (JavaScript garbage raises no event) since the last look; and
//! * no Script holds the node. The host answers [`ScriptHost::held_nodes`] after a garbage collection: live
//!   `SFNode` wrappers (the prelude's caches are `WeakRef`s, like X_ITE's `SFNodeCache`), the nodes in
//!   `MFNode` / `SFNode` values that live JavaScript objects hold, and the node of a live field object.
//!   A host that cannot answer (`None`) holds every runtime-created node, which is the old behavior.
//!
//! A runtime-created node stays when a live field (a Group's `children`, a Script field, a PROTO field), a
//! name, a ROUTE of a live context (both ends, as an X_ITE `Route` is held by both fields), a pending event,
//! a Script, or a node that stays refers to it. A `createVrmlFromString` scene is one unit (an X_ITE node
//! holds its scene): while one of its nodes stays all of its nodes do, and its routes keep their ends; it
//! is not a root itself. When its last node goes, its context is emptied and its slot released.
//!
//! Not candidates (they stay as before): Script, Inline, Anchor, PROTO and EXTERNPROTO instances, bindable
//! nodes (Viewpoint kinds, NavigationInfo, Background, Fog, ...), layers; and a `createVrmlFromString`
//! scene that has PROTO, EXTERNPROTO, IMPORT or EXPORT or one of those nodes. The host's tables are told
//! with [`Runtime::take_released`], and a freed id is handed out again like any released one; no Script
//! wrapper or value can name it (they were the reason the node was held).
//!
//! # Scenes loaded from a URL (issue #172)
//!
//! A scene of `createX3DFromURL` / `createVrmlFromURL` that passes the same simple-scene gate (one context, no PROTO,
//! EXTERNPROTO, IMPORT or EXPORT, every node eligible) joins `dynamic_ctxs` and is swept like a `createVrmlFromString`
//! scene, by the same code. X_ITE keeps nothing of it once the load is over (`removeLoadingObject (fileLoader)`,
//! Browser/X3DBrowser.js): the garbage collector frees it when the promise's value and the fields its root nodes went to let go.
//!
//! * A Script's scene becomes a candidate when its outcome is delivered (the promise resolves, or the createVrmlFromURL
//!   event is sent), not before: a scene that still loads is a live context. The Script holds an X3DScene object of it
//!   (`dynamic_scenes`), so the host is also asked whether JavaScript still reaches that object
//!   ([`ScriptHost::held_scenes`]). It is asked first, before the nodes: the X3DScene object holds the objects of its
//!   named nodes, so they are garbage only once the scene object is. A held scene object keeps the whole scene.
//! * The page's scene becomes a candidate when the page takes the outcome ([`Runtime::take_page_browser_settled`]).
//!   Until then the outcome holds it; after that the page's node ids hold nothing, as for its `createVrmlFromString`,
//!   and the scene stays only while something in the world refers to one of its nodes.
//!
//! When such a scene goes, its change-field events and its state in the change-field tables are dropped, and the host
//! forgets its X3DScene object and callbacks ([`ScriptHost::release_scenes`]), so nothing reaches the scene that takes
//! the context slot next.
//!
//! # R0.3-E: table slots
//!
//! Execution contexts, PROTO and EXTERNPROTO declarations and IMPORT and EXPORT records are tables
//! with position ids, like the node arena. After the sweep empties contexts, [`Runtime::reclaim_tables`]
//! releases the slots of emptied contexts and of the records they own, so that the next scene
//! (`Scene::merge`, a released run of its size) or body (`Scene::new_context`) takes them again.
//!
//! Who holds each id, and the rule that makes reuse safe (a stale id must never reach a new object):
//!
//! | id | created by | held by | released when |
//! | --- | --- | --- | --- |
//! | `CtxId` | `merge` (a run), `new_context` (PROTO body, instance body) | `ExecutionContext.parent`, `Outer::Proto`'s declaration, `Node.ctx`, record `ctx` / `body`, `inline_scenes`, `instance_bodies`, `pending_scenes`, `frame_waiters`, `pending_externs`, `extern_frame_waiters`, `ExternState.scene`, `extern_cache` (`Ready`), `release_ctxs`, `dead_ctxs`, `disposed_scenes`, `emptied_ctxs`, `scene_infos`, `inline_live` | emptied, and none of the holders names it |
//! | `ProtoId` | `merge` (a run), `new_proto` (parser) | a context's `protos` (also copied into instance bodies of other scenes), `Outer::Proto`, `NodeKind::Instance`, `FieldRef.proto` of fields, `extern_loaded`, `Holder::ProtoField` (rebuilt) | owner context emptied and none of the holders names it |
//! | `ExternId` | `merge` (a run), `new_extern_proto` (parser) | a context's `externprotos` (also in bodies), `NodeKind::Instance`, `extern_states`, `extern_loaded`, `extern_load_events`, `Task::ExternLoad` (tasks, fetches), `pending_externs`, `extern_frame_waiters`, `CacheEntry::Pending` waiters, `Holder::ExternField` (rebuilt) | owner context emptied and none of the holders names it |
//! | `ImportedId` | `merge` (a run), `add_imported_node` | a context's `imported`, `NodeKind::Imported` of its instance node | owner context emptied and none of the holders names it |
//! | `ExportedId` | `merge` (a run), `update_exported_node` | a context's `exported` | owner context emptied and none of the holders names it |
//!
//! None of these ids crosses into JavaScript (the Script host addresses nodes, and contexts through
//! the Script node) or into the viewer (which rebuilds what it needs every frame).
//!
//! Two methods together keep a stale id away from a new object:
//!
//! * **Every holder is cleared before the slot is released.** `reclaim_tables` collects every id
//!   that a live node, a context, a live record or a runtime table names, and releases only what is
//!   in none of them. A context also stays while a record it owns stays (`ProtoDecl.ctx` is the parent
//!   of new instance bodies). What stays is looked at again at the next sweep.
//! * **Generations for the holders that outlive a load.** Each slot has a generation, bumped on
//!   release. The scene cache stores `(ctx, generation)` and a lookup that finds another generation
//!   evicts the entry. EXTERNPROTO request identities are runtime-wide, so a late fetch for a released
//!   declaration can never match the request of the declaration that takes the slot.
//!
//! When a context slot is released it leaves `emptied_ctxs`, `dead_ctxs` and `disposed_scenes`, so these
//! sets are bounded by the scenes that are disposed and still held. A released declaration loses its
//! `extern_states` and `extern_loaded` entries.

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
    node.fields
        .iter()
        .chain(node.user_fields.iter())
        .chain(node.private_fields.iter())
}

impl Runtime {
    /// A scene (or a PROTO body) was disposed: its nodes and contexts are released at the next sweep.
    pub(crate) fn queue_release(&mut self, ctxs: &[CtxId], nodes: &[NodeId]) {
        self.release_ctxs.extend_from_slice(ctxs);
        self.release_queue
            .extend(nodes.iter().copied().filter(|&n| {
                self.scene.is_live(n) && !matches!(self.scene.node(n).kind, NodeKind::Internal(_))
            }));
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
        let needed: HashSet<NodeId> = self
            .pending_scenes
            .iter()
            .map(|p| p.scene_node)
            .chain(self.pending_externs.iter().map(|p| p.scene_node))
            .chain(
                self.pending_browser
                    .iter()
                    .chain(self.browser_frame_waiters.iter())
                    .map(|p| p.scene_node),
            )
            .collect();
        let loose: Vec<NodeId> = (0..self.scene.nodes.len() as NodeId)
            .filter(|n| {
                matches!(self.scene.node(*n).kind, NodeKind::Internal("X3DScene"))
                    && !needed.contains(n)
            })
            .collect();

        self.release_queue.extend(loose);

        // Runtime-created nodes are candidates too (see the module header); the Script host is asked about those that
        // nothing else keeps, below.
        let dynamic = self.dynamic_candidates();

        if self.release_queue.is_empty() && dynamic.is_empty() {
            // Emptied contexts that something held at an earlier sweep may be free now.
            self.reclaim_tables();
            return stats;
        }

        let queued: HashSet<NodeId> = std::mem::take(&mut self.release_queue)
            .into_iter()
            .filter(|&n| self.scene.is_live(n))
            .collect();
        let mut candidates: HashSet<NodeId> = queued
            .iter()
            .copied()
            .chain(dynamic.iter().copied())
            .collect();

        // Contexts of `createVrmlFromString` scenes whose nodes are all candidates: they are not roots.
        let ctx_map = self.dynamic_ctx_map();
        let transient: Vec<CtxId> = ctx_map
            .iter()
            .filter(|(_, nodes)| !nodes.is_empty() && nodes.iter().all(|n| dynamic.contains(n)))
            .map(|(c, _)| *c)
            .collect();
        let transient_set: HashSet<CtxId> = transient.iter().copied().collect();
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
            if gone(c) || transient_set.contains(&c) {
                continue;
            }

            for n in ctx
                .root_nodes
                .iter()
                .flatten()
                .chain(ctx.named_nodes.values().iter())
            {
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

        // Live PROTO and EXTERNPROTO declarations (default values), IMPORT and EXPORT. Released slots
        // name nothing.
        let slots = (&self.scene.imported_slots, &self.scene.exported_slots);

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

        for (_, i) in self
            .scene
            .imported
            .iter()
            .enumerate()
            .filter(|(k, i)| !slots.0.is_free(*k) && !gone(i.ctx))
        {
            note(&mut keep, i.inline);
            note(&mut keep, i.instance);
        }

        for (_, e) in self
            .scene
            .exported
            .iter()
            .enumerate()
            .filter(|(k, e)| !slots.1.is_free(*k) && !gone(e.ctx))
        {
            note(&mut keep, e.local);
        }

        // What a node that stays refers to stays too: its children, the body of an instance.
        let mut by_ctx: HashMap<CtxId, Vec<NodeId>> = HashMap::new();

        for &n in &candidates {
            by_ctx.entry(self.scene.node(n).ctx).or_default().push(n);
        }

        let mut work: Vec<NodeId> = keep.iter().copied().collect();

        self.close_keep(&candidates, &by_ctx, &mut keep, &mut work);

        // A node keeps its scene, and a route keeps both of its ends: for the scenes that
        // `createVrmlFromString` made (their own roots and routes are not roots) until nothing changes.
        let transient_nodes: Vec<(CtxId, Vec<NodeId>)> = transient
            .iter()
            .map(|&c| (c, ctx_map[&c].clone()))
            .collect();

        self.close_transient(&candidates, &by_ctx, &transient_nodes, &mut keep, &mut work);

        // A scene that `createX3DFromURL` / `createVrmlFromURL` gave a Script stays while the Script holds its X3DScene
        // object (X_ITE: the scene holds its root nodes). The host is asked only about scenes none of whose nodes stays, and
        // before it is asked about the nodes: the X3DScene object holds objects of its named nodes, so a node object is
        // only garbage once the scene object is.
        let ask_scenes: Vec<CtxId> = {
            let mut ask: Vec<CtxId> = transient_nodes
                .iter()
                .filter(|(c, nodes)| {
                    self.dynamic_scenes.contains(c) && !nodes.iter().any(|n| keep.contains(n))
                })
                .map(|(c, _)| *c)
                .collect();
            ask.sort_unstable();
            ask
        };

        if !ask_scenes.is_empty() {
            let held = self.script_held_scenes(&ask_scenes);

            for (c, nodes) in &transient_nodes {
                if held.contains(c) {
                    for &n in nodes {
                        if keep.insert(n) {
                            work.push(n);
                        }
                    }
                }
            }

            self.close_keep(&candidates, &by_ctx, &mut keep, &mut work);
            self.close_transient(&candidates, &by_ctx, &transient_nodes, &mut keep, &mut work);
        }

        // A runtime-created node that would go stays while a Script holds it (X_ITE: until the garbage collector frees it). The
        // host is asked only now, so that it looks only at what nothing else keeps.
        let ask: Vec<NodeId> = {
            let mut ask: Vec<NodeId> = dynamic
                .iter()
                .copied()
                .filter(|n| !keep.contains(n))
                .collect();
            ask.sort_unstable();
            ask
        };

        if !ask.is_empty() {
            for n in self.script_held(&ask) {
                if keep.insert(n) {
                    work.push(n);
                }
            }

            self.close_keep(&candidates, &by_ctx, &mut keep, &mut work);
            self.close_transient(&candidates, &by_ctx, &transient_nodes, &mut keep, &mut work);
        }

        // --- the release --------------------------------------------------------------------------

        let release: HashSet<NodeId> = candidates
            .iter()
            .copied()
            .filter(|n| !keep.contains(n))
            .collect();

        stats.retained = keep.len();
        candidates.retain(|n| keep.contains(n) && queued.contains(n));
        self.release_queue = candidates.into_iter().collect();
        self.release_queue.sort_unstable();

        // A disposed context is emptied once none of its nodes stays.
        let kept_ctxs: HashSet<CtxId> = self
            .release_queue
            .iter()
            .map(|&n| self.scene.node(n).ctx)
            .collect();
        let mut empty: Vec<CtxId> = dead_ctxs
            .iter()
            .copied()
            .filter(|c| !kept_ctxs.contains(c))
            .collect();

        // A `createVrmlFromString` scene, or a simple scene loaded from a URL, goes with its last node.
        let mut gone_scenes: Vec<CtxId> = vec![];

        for (c, nodes) in &transient_nodes {
            if nodes.iter().all(|n| release.contains(n)) {
                empty.push(*c);
                self.dynamic_ctxs.remove(c);

                if self.dynamic_scenes.remove(c) {
                    gone_scenes.push(*c);
                }

                self.forget_scene_events(*c);
            }
        }

        self.release_ctxs.retain(|c| kept_ctxs.contains(c));

        if release.is_empty() && empty.is_empty() {
            if self.reclaim_tables() > 0 {
                self.rebuild_parents();
            }

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

        if !gone_scenes.is_empty()
            && let Some(mut host) = self.script_host.take()
        {
            gone_scenes.sort_unstable();
            host.release_scenes(self, &gone_scenes);
            self.script_host = Some(host);
        }

        self.reclaim_tables();
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

        // RigidBodyPhysics: bodies and constraints out of the worlds that stay, Ammo objects given back.
        self.physics_release(dead);

        self.dynamic_nodes.retain(|n| !dead.contains(n));
        self.provider_warned.retain(|n| !dead.contains(n));
        self.frt.retain(|k, _| !dead.contains(&k.0));

        for f in self.frt.values_mut() {
            f.interests.retain(|i| !dead.contains(&i.node));
            f.field_interests.retain(|k| !dead.contains(&k.0));
            f.references.retain(|k| !dead.contains(&k.0));
        }

        for n in &mut self.nrt {
            n.interests.retain(|i| !dead.contains(&i.node));
        }

        self.tainted_fields
            .retain(|(k, e)| !dead.contains(&k.0) && !dead.contains(&e.origin.0));
        self.tainted_nodes.retain(|n| !dead.contains(n));
        self.scene.raw_tokens.retain(|k, _| !dead.contains(&k.0));
        self.font_style_font.retain(|n, _| !dead.contains(n));
        self.font_style_loads.retain(|l| !dead.contains(&l.node));
        self.font_style_snapshot.retain(|n, _| !dead.contains(n));
        self.text_font_styles.retain(|n, _| !dead.contains(n));
        self.default_font_texts.retain(|n| !dead.contains(n));
        self.decoded_images.retain(|(n, ..)| !dead.contains(n));
        self.rendered_now.retain(|n| !dead.contains(n));

        let alive = |t: &Task| match t {
            Task::ScriptLoad(n)
            | Task::ScriptLoaded(n, _)
            | Task::UrlLoad(n, _)
            | Task::ScriptFetch(n, _)
            | Task::ScriptCandidate(n, ..) => !dead.contains(n),
            Task::ExternLoad(..) | Task::BrowserLoad(_) | Task::BrowserParse(_) => true,
        };

        self.tasks.retain(alive);
        self.fetches.retain(alive);
        self.timeouts.retain(|t| !dead.contains(&t.node));
        self.bind_modifications.retain(|n, _| !dead.contains(n));
        self.instance_update.retain(|n| !dead.contains(n));
        self.instance_bodies.retain(|n, _| !dead.contains(n));
        self.inline_scenes.retain(|n, _| !dead.contains(n));
        self.inline_geometries.retain(|n, _| !dead.contains(n));
        self.font_library_fonts.retain(|n, _| !dead.contains(n));
        self.font_style_fonts.retain(|n, _| !dead.contains(n));
        self.pending_font_styles.retain(|p| !dead.contains(&p.node));
        self.shaders.parts.retain(|n, _| !dead.contains(n));
        self.shaders.composed.retain(|n, _| !dead.contains(n));
        self.shaders.selected_count.retain(|n, _| !dead.contains(n));
        self.shaders.appearances.retain(|n, _| !dead.contains(n));
        for a in self.shaders.appearances.values_mut() {
            a.nodes.retain(|s| !dead.contains(s));
            a.selected = a.selected.filter(|s| !dead.contains(s));
        }
        self.background_textures.retain(|n, _| !dead.contains(n));
        for textures in self.background_textures.values_mut() {
            textures.retain(|(_, t)| !dead.contains(t));
        }
        self.live_watchers.retain(|(n, _)| !dead.contains(n));
        self.followers.retain(|n| !dead.contains(n));
        self.key_sensors.retain(|n| !dead.contains(n));
        self.transform_sensors.retain(|n| !dead.contains(n));
        self.pick_sensors.retain(|n| !dead.contains(n));
        self.layer_object_types.retain(|n, _| !dead.contains(n));
        self.nurbs_scales.retain(|n, _| !dead.contains(n));
        self.viewpoint_groups.retain(|n, _| !dead.contains(n));
        self.hanim_humanoids.retain(|n, _| !dead.contains(n));
        self.hanim_poses.retain(|n, _| !dead.contains(n));
        self.hanim_motions.retain(|n, _| !dead.contains(n));
        self.sound_release(&dead);
        self.time_states.retain(|n, _| !dead.contains(n));
        self.nurbs_set_members.retain(|n, _| !dead.contains(n));
        for members in self.nurbs_set_members.values_mut() {
            members.retain(|n| !dead.contains(n));
        }
        for list in self.world_infos.values_mut() {
            list.retain(|n| !dead.contains(n));
        }
        self.world_infos.retain(|_, list| !list.is_empty());
        self.scripts.retain(|n| !dead.contains(n));
        self.field_observed.retain(|k| !dead.contains(&k.0));
        self.page_observed.retain(|k| !dead.contains(&k.0));
        self.js_mirror.retain(|k, _| !dead.contains(&k.0));
        self.page_mirror.retain(|k, _| !dead.contains(&k.0));
        self.pending_field_events.retain(|k| !dead.contains(&k.0));
        // A queued event of a released node must not reach the node that takes its id.
        if let Some(events) = &mut self.page_field_events {
            events.retain(|(k, _)| !dead.contains(&k.0));
        }
        if let Some(released) = &mut self.page_released {
            released.extend(dead.iter().copied());
        }
        self.pending_scenes
            .retain(|p| !dead.contains(&p.node) && !dead.contains(&p.scene_node));
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
        // X3DTextContext keeps a WeakMap per execution context: a reused slot starts with no fonts.
        self.font_libraries.remove(&c);
        self.emptied_ctxs.insert(c);

        for p in self.scene.protos.iter_mut().filter(|p| p.ctx == c) {
            p.fields = vec![];
        }

        for e in self.scene.externprotos.iter_mut().filter(|e| e.ctx == c) {
            e.fields = vec![];
        }
    }
}

impl Runtime {
    /// May the sweep release this node when nothing refers to it? Not the nodes the host keeps tables
    /// for beyond the runtime's own (bindable stacks, layers), nor Script, Inline and PROTO instances:
    /// those stay as they always did.
    pub(crate) fn dynamic_eligible(&self, n: NodeId) -> bool {
        if !self.scene.is_live(n) || !matches!(self.scene.node(n).kind, NodeKind::Builtin(_)) {
            return false;
        }

        let name = self.builtin_name(n).unwrap_or("");

        !(name.ends_with("Viewpoint")
            || matches!(
                name,
                "Script"
                    | "Inline"
                    | "InlineGeometry"
                    | "NavigationInfo"
                    | "Background"
                    | "TextureBackground"
                    | "Fog"
                    | "LocalFog"
                    | "ViewpointGroup"
                    | "Layer"
                    | "LayerSet"
                    | "Viewport"
                    | "LayoutLayer"
                    | "ScreenGroup"
                    | "Anchor"
            ))
    }

    /// The runtime-created nodes of each `createVrmlFromString` scene that is still tracked.
    fn dynamic_ctx_map(&self) -> HashMap<CtxId, Vec<NodeId>> {
        let mut map: HashMap<CtxId, Vec<NodeId>> =
            self.dynamic_ctxs.iter().map(|&c| (c, vec![])).collect();

        for &n in &self.dynamic_nodes {
            if let Some(list) = map.get_mut(&self.scene.node(n).ctx) {
                list.push(n);
            }
        }

        map
    }

    /// The runtime-created nodes that are due for a look.
    fn dynamic_candidates(&mut self) -> HashSet<NodeId> {
        let live: Vec<NodeId> = self
            .dynamic_nodes
            .iter()
            .copied()
            .filter(|&n| {
                self.scene.is_live(n) && !matches!(self.scene.node(n).kind, NodeKind::Internal(_))
            })
            .collect();

        self.dynamic_nodes = live.iter().copied().collect();

        if live.is_empty() || self.processing {
            return HashSet::new();
        }

        // Reachability changes with a node-valued field write, a ROUTE change, a Script call and with
        // JavaScript garbage; the last raises no event, so look now and then anyway.
        if !self.dynamic_dirty {
            self.dynamic_idle += 1;

            if self.dynamic_idle < 30 {
                return HashSet::new();
            }
        }

        self.dynamic_idle = 0;
        self.dynamic_dirty = false;

        live.into_iter().collect()
    }

    /// Which of `nodes` a Script holds: the host's answer, and the scene of a node it holds. A host that cannot say holds them
    /// all ([`ScriptHost::held_nodes`]).
    fn script_held(&mut self, nodes: &[NodeId]) -> HashSet<NodeId> {
        if self.scripts.is_empty() {
            return HashSet::new();
        }

        let Some(mut host) = self.script_host.take() else {
            return nodes.iter().copied().collect();
        };

        let answer = host.held_nodes(self, nodes);

        self.script_host = Some(host);

        let Some(list) = answer else {
            return nodes.iter().copied().collect();
        };

        let mut held: HashSet<NodeId> = list.into_iter().collect();

        for list in self.dynamic_ctx_map().values() {
            if list.iter().any(|n| held.contains(n)) {
                held.extend(list.iter().copied());
            }
        }

        held
    }

    /// Which of the scenes `ctxs` a Script holds through its X3DScene object. A host that cannot say holds them all
    /// ([`ScriptHost::held_scenes`]).
    fn script_held_scenes(&mut self, ctxs: &[CtxId]) -> HashSet<CtxId> {
        if self.scripts.is_empty() {
            return HashSet::new();
        }

        let Some(mut host) = self.script_host.take() else {
            return ctxs.iter().copied().collect();
        };

        let answer = host.held_scenes(self, ctxs);

        self.script_host = Some(host);

        match answer {
            Some(list) => list.into_iter().collect(),
            None => ctxs.iter().copied().collect(),
        }
    }

    /// A released scene's context id may be handed out again: its change-field events and its state in the change-field
    /// tables must not reach the scene that takes the slot.
    fn forget_scene_events(&mut self, c: CtxId) {
        let e = &mut self.ctx_events;

        e.no_forward.remove(&c);
        e.watched.remove(&c);
        e.pending.retain(|x| x.0 != c);
        e.parsed.retain(|x| x.0 != c);
        e.deferred.retain(|x| x.0 != c);
    }

    /// A node keeps its scene, and a route keeps both of its ends: for the scenes that `createVrmlFromString` made (their own
    /// roots and routes are not roots) until nothing changes.
    fn close_transient(
        &self,
        candidates: &HashSet<NodeId>,
        by_ctx: &HashMap<CtxId, Vec<NodeId>>,
        transient_nodes: &[(CtxId, Vec<NodeId>)],
        keep: &mut HashSet<NodeId>,
        work: &mut Vec<NodeId>,
    ) {
        loop {
            let mut grew = false;

            for (c, nodes) in transient_nodes {
                if nodes.iter().any(|n| keep.contains(n)) {
                    for &n in nodes {
                        if keep.insert(n) {
                            work.push(n);
                            grew = true;
                        }
                    }
                }

                for r in &self.scene.contexts[*c].routes {
                    let (a, b) = (r.source_node, r.destination_node);
                    let a_live = !candidates.contains(&a) || keep.contains(&a);
                    let b_live = !candidates.contains(&b) || keep.contains(&b);

                    if a_live && !b_live && keep.insert(b) {
                        work.push(b);
                        grew = true;
                    }

                    if b_live && !a_live && keep.insert(a) {
                        work.push(a);
                        grew = true;
                    }
                }
            }

            if !grew {
                break;
            }

            self.close_keep(candidates, by_ctx, keep, work);
        }
    }

    /// Everything a node that stays refers to stays too: its children, the body of an instance.
    fn close_keep(
        &self,
        candidates: &HashSet<NodeId>,
        by_ctx: &HashMap<CtxId, Vec<NodeId>>,
        keep: &mut HashSet<NodeId>,
        work: &mut Vec<NodeId>,
    ) {
        while let Some(n) = work.pop() {
            let mut next: Vec<NodeId> = all_fields(self.scene.node(n))
                .flat_map(|f| node_refs(&f.value))
                .collect();

            if let Some(body) = self.instance_bodies.get(&n) {
                for (c, list) in by_ctx {
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
    }
}

/// Every table id that something live names (see the module header).
#[derive(Default)]
struct Held {
    ctxs: HashSet<CtxId>,
    protos: HashSet<ProtoId>,
    externs: HashSet<ExternId>,
    imported: HashSet<ImportedId>,
    exported: HashSet<ExportedId>,
}

impl Runtime {
    /// Contexts that the sweep emptied and whose slots are not released yet.
    pub fn emptied_contexts(&self) -> usize {
        self.emptied_ctxs.len()
    }

    /// The ids that a live node, a context, a live record or a runtime table names. A record is live
    /// while its owner context is not emptied; a released slot names nothing.
    fn held_table_ids(&self) -> Held {
        let s = &self.scene;
        let emptied = &self.emptied_ctxs;
        let mut h = Held::default();

        for (id, node) in s.nodes.iter().enumerate() {
            if !s.is_live(id as NodeId) {
                continue;
            }

            h.ctxs.insert(node.ctx);

            match node.kind {
                NodeKind::Instance(ProtoRef::Proto(p)) => {
                    h.protos.insert(p);
                }
                NodeKind::Instance(ProtoRef::Extern(e)) => {
                    h.externs.insert(e);
                }
                NodeKind::Imported(i) => {
                    h.imported.insert(i);
                }
                _ => {}
            }

            for f in all_fields(node) {
                h.protos.extend(f.references.iter().map(|r| r.proto));
            }
        }

        for (c, ctx) in s.contexts.iter().enumerate() {
            if s.ctx_slots.is_free(c) {
                continue;
            }

            h.ctxs.extend(ctx.parent);

            if let Outer::Proto(p) = ctx.outer {
                h.protos.insert(p);
            }

            h.protos.extend(ctx.protos.values().iter().copied());
            h.externs.extend(ctx.externprotos.values().iter().copied());
            h.imported.extend(ctx.imported.values().iter().copied());
            h.exported.extend(ctx.exported.values().iter().copied());
        }

        // Runtime tables.
        h.ctxs.extend(self.inline_scenes.values().copied());
        h.ctxs.extend(self.instance_bodies.values().copied());
        h.ctxs.extend(self.release_ctxs.iter().copied());
        h.ctxs.extend(self.pending_scenes.iter().map(|p| p.ctx));
        h.ctxs.extend(self.frame_waiters.iter().map(|w| w.ctx));
        h.ctxs.extend(
            self.pending_browser
                .iter()
                .chain(self.browser_frame_waiters.iter())
                .map(|p| p.ctx),
        );
        h.ctxs.insert(self.scene.world);
        h.ctxs
            .extend(self.world_state.waiters.iter().filter_map(|w| {
                if let crate::browser_load::WorldScene::Ctx(c) = w.scene {
                    Some(c)
                } else {
                    None
                }
            }));

        for p in self
            .pending_externs
            .iter()
            .chain(self.extern_frame_waiters.iter())
        {
            h.ctxs.insert(p.ctx);
            h.externs.insert(p.ext);
        }

        for (_, entry) in &self.extern_cache {
            match entry {
                CacheEntry::Ready(c, _) => {
                    h.ctxs.insert(*c);
                }
                CacheEntry::Pending(waiters) => h.externs.extend(waiters.iter().map(|w| w.0)),
            }
        }

        h.externs.extend(self.extern_load_events.iter().copied());

        for t in self.tasks.iter().chain(self.fetches.iter()) {
            if let Task::ExternLoad(e, _) = t {
                h.externs.insert(*e);
            }
        }

        // Live records are held; a held declaration holds what it names.
        for (e, decl) in s.externprotos.iter().enumerate() {
            if !s.extern_slots.is_free(e) && !emptied.contains(&decl.ctx) {
                h.externs.insert(e);
            }
        }

        for &e in &h.externs {
            h.ctxs.insert(s.externprotos[e].ctx);

            if let Some((c, _)) = self.extern_states.get(&e).and_then(|st| st.scene) {
                h.ctxs.insert(c);
            }

            if let Some(Some(p)) = self.extern_loaded.get(&e) {
                h.protos.insert(*p);
            }
        }

        for (p, decl) in s.protos.iter().enumerate() {
            if !s.proto_slots.is_free(p) && !emptied.contains(&decl.ctx) {
                h.protos.insert(p);
            }
        }

        for &p in &h.protos {
            h.ctxs.insert(s.protos[p].ctx);
            h.ctxs.insert(s.protos[p].body);
        }

        for (i, rec) in s.imported.iter().enumerate() {
            if !s.imported_slots.is_free(i) && !emptied.contains(&rec.ctx) {
                h.imported.insert(i);
            }
        }

        for (x, rec) in s.exported.iter().enumerate() {
            if !s.exported_slots.is_free(x) && !emptied.contains(&rec.ctx) {
                h.exported.insert(x);
            }
        }

        h.ctxs.extend(h.imported.iter().map(|&i| s.imported[i].ctx));
        h.ctxs.extend(h.exported.iter().map(|&x| s.exported[x].ctx));
        h
    }

    /// Release the table slots that emptied contexts left and nothing names any more (R0.3-E). Returns
    /// how many slots were released.
    pub(crate) fn reclaim_tables(&mut self) -> usize {
        if self.emptied_ctxs.is_empty() {
            return 0;
        }

        let h = self.held_table_ids();
        let s = &self.scene;
        let owned = |c: CtxId| self.emptied_ctxs.contains(&c);

        let protos: Vec<ProtoId> = (0..s.protos.len())
            .filter(|&p| {
                !s.proto_slots.is_free(p) && owned(s.protos[p].ctx) && !h.protos.contains(&p)
            })
            .collect();
        let externs: Vec<ExternId> = (0..s.externprotos.len())
            .filter(|&e| {
                !s.extern_slots.is_free(e)
                    && owned(s.externprotos[e].ctx)
                    && !h.externs.contains(&e)
            })
            .collect();
        let imported: Vec<ImportedId> = (0..s.imported.len())
            .filter(|&i| {
                !s.imported_slots.is_free(i) && owned(s.imported[i].ctx) && !h.imported.contains(&i)
            })
            .collect();
        let exported: Vec<ExportedId> = (0..s.exported.len())
            .filter(|&x| {
                !s.exported_slots.is_free(x) && owned(s.exported[x].ctx) && !h.exported.contains(&x)
            })
            .collect();

        let mut ctxs: Vec<CtxId> = self
            .emptied_ctxs
            .iter()
            .copied()
            .filter(|c| *c != ROOT && !h.ctxs.contains(c))
            .collect();

        ctxs.sort_unstable();

        for &e in &externs {
            self.extern_states.remove(&e);
            self.extern_loaded.remove(&e);
            self.scene.release_extern(e);
        }

        for &p in &protos {
            self.scene.release_proto(p);
        }

        for &i in &imported {
            self.scene.release_imported(i);
        }

        for &x in &exported {
            self.scene.release_exported(x);
        }

        for &c in &ctxs {
            self.emptied_ctxs.remove(&c);
            self.dead_ctxs.remove(&c);
            self.disposed_scenes.remove(&c);
            self.inline_live.remove(&c);
            self.scene.release_ctx(c);
        }

        let n = protos.len() + externs.len() + imported.len() + exported.len() + ctxs.len();

        self.table_slots_released += n as u64;
        n
    }
}
