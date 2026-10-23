//! The change fields of X3DExecutionContext and X3DScene (Execution/X3DExecutionContext.js, X3DScene.js).
//!
//! Pinned X_ITE 16.5.0 declares, as fields, `sceneGraph_changed` and `bbox_changed` (X3DExecutionContext) and `profile_changed` and
//! `metadata_changed` (X3DScene); all are SFTime fields set to the real clock. A Script observes them with `addFieldCallback`
//! (X3DField.processEvent calls the callbacks) and sees only *that* the field fired and when, never the time. The other names the
//! execution context API suggests (`_importedNodes_changed`, `_exportedNodes_changed`, `_protos_changed`, `_externprotos_changed`,
//! `_rootNodes_changed`, `_routes_changed`) do not exist in the pinned source; `_namedNodes_changed` is a plain number property
//! (see prelude.js), `_components_changed` and `_units_changed` are plain too.
//!
//! A field is set several times before X_ITE processes its event: it fires once (the field is tainted). One "round" here is one
//! processed event; a source that needs several rounds (see [`Runtime::root_nodes_rounds`]) delivers the callbacks that many times, in
//! a row, after the events that caused them.

use crate::*;
use std::collections::HashSet;

/// The change fields (the index is what the Script realm uses).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CtxField {
    SceneGraph = 0,
    BBox = 1,
    Profile = 2,
    Metadata = 3,
}

impl CtxField {
    pub fn from_index(i: u32) -> Option<CtxField> {
        [
            CtxField::SceneGraph,
            CtxField::BBox,
            CtxField::Profile,
            CtxField::Metadata,
        ]
        .get(i as usize)
        .copied()
    }
}

#[derive(Default)]
pub struct CtxEvents {
    /// A Script holds a callback on one of the fields: parent changes are compared only then (rebuild_parents).
    pub observed: bool,
    /// (context, field, rounds): events waiting for the end of the current event cascade.
    pub pending: Vec<(CtxId, CtxField, u32)>,
    /// Events tainted while a scene was parsed: delivered with the first events once a Script has a callback on the scene.
    pub parsed: Vec<(CtxId, CtxField, u32)>,
    /// The contexts a Script has a callback on.
    pub watched: HashSet<CtxId>,
    /// The world switched in this frame's finishEvents: what it left for the next frame waits one more release.
    pub hold: bool,
    /// Delivered with the finishEvents of the next frame.
    pub deferred: Vec<(CtxId, CtxField, u32)>,
    /// Scenes whose changes do not reach the context they were made in (`Browser.createScene ()`).
    pub no_forward: HashSet<CtxId>,
    /// PROTO declaration bodies whose root nodes have their SFNode (see [`Runtime::ctx_streamed`]), with the generation of
    /// their context slot.
    pub wrapped_bodies: HashSet<(CtxId, u32)>,
}

/// Events of one cascade overlap (the field is tainted once): the rounds are the largest of them. Events of different frames add up.
fn merge(
    list: &mut Vec<(CtxId, CtxField, u32)>,
    ctx: CtxId,
    field: CtxField,
    rounds: u32,
    sum: bool,
) {
    match list.iter_mut().find(|e| e.0 == ctx && e.1 == field) {
        Some(e) => e.2 = if sum { e.2 + rounds } else { e.2.max(rounds) },
        None => list.push((ctx, field, rounds)),
    }
}

impl Runtime {
    /// The context and the contexts its field is forwarded to (`_sceneGraph_changed .addFieldInterest (executionContext ._sceneGraph_changed)`).
    fn ctx_chain(&self, ctx: CtxId, forwarded: bool) -> Vec<CtxId> {
        let mut chain = vec![ctx];

        if !forwarded {
            return chain;
        }

        let mut c = ctx;

        while !self.ctx_events.no_forward.contains(&c) {
            match self.scene.contexts[c].parent {
                Some(p) if !chain.contains(&p) => {
                    chain.push(p);
                    c = p;
                }
                _ => break,
            }
        }

        chain
    }

    /// `ctx ._sceneGraph_changed = time` and friends: the field of `ctx` (and, for the graph and bbox fields, of the contexts around it)
    /// fires `rounds` times.
    pub fn ctx_event(&mut self, ctx: CtxId, field: CtxField, rounds: u32) {
        if ctx >= self.scene.contexts.len() {
            return;
        }

        let forwarded = matches!(field, CtxField::SceneGraph | CtxField::BBox);

        // Nothing is delivered before the world is live: what happens while it loads is processed with its first frame.
        let live = self.scene_live;

        for c in self.ctx_chain(ctx, forwarded) {
            if live {
                merge(&mut self.ctx_events.pending, c, field, rounds, false);
            } else {
                merge(&mut self.ctx_events.deferred, c, field, rounds, true);
            }
        }
    }

    /// Like [`Runtime::ctx_event`], delivered with the finishEvents of the next frame.
    pub fn ctx_event_deferred(&mut self, ctx: CtxId, field: CtxField, rounds: u32) {
        if ctx >= self.scene.contexts.len() {
            return;
        }

        let forwarded = matches!(field, CtxField::SceneGraph | CtxField::BBox);

        for c in self.ctx_chain(ctx, forwarded) {
            merge(&mut self.ctx_events.deferred, c, field, rounds, true);
        }
    }

    /// The rounds of a change of the `rootNodes` of `ctx`. The scene of the world is watched by X3DWorld (`rootNodes` interest →
    /// `layer0 ._children = rootNodes`), whose cascade of parent changes reaches the field in four processed events; any other scene
    /// changes its parents once. (Measured on the pin: ctxev-roots / ctxev-scene; the cascade itself is not ported.)
    pub fn root_nodes_rounds(&self, ctx: CtxId) -> u32 {
        if ctx == self.scene.world { 4 } else { 1 }
    }

    /// X3DBaseNode.parentsChanged: the `sceneGraph_changed` and `bbox_changed` of the context the node belongs to.
    pub(crate) fn ctx_parents_changed(&mut self, node: NodeId) {
        let ctx = self.scene.node(node).ctx;

        self.ctx_event(ctx, CtxField::SceneGraph, 1);
        self.ctx_event(ctx, CtxField::BBox, 1);
    }

    /// X3DExecutionContext.toVRMLStream and toJSONStream read `this .rootNodes` through the MFNode proxy, whose element read is
    /// `SFNodeCache .get (node)`: the first read of a node makes its SFNode, and `new SFNode (node)` adds itself as a parent of the
    /// node (X3DBaseNode.parentsChanged). The root nodes of the world and of PROTO instance bodies have their SFNode from the start
    /// (X3DWorld.set_rootNodes__, X3DPrototypeInstance.getInnerNode); those of a PROTO declaration's body get theirs the first time
    /// the declaration is streamed (X3DProtoDeclaration.toVRMLStream streams its body, nested declarations too). Each such body
    /// fires its fields once, and each of those events reaches the scene on its own (measured: ctxev-stream-vrml / -json).
    /// toXMLStream streams the MFNode without the proxy and makes no SFNode (ctxev-stream-xml).
    /// Call this when the scene with root context `ctx` is streamed as VRML or JSON.
    pub fn ctx_streamed(&mut self, ctx: CtxId) {
        if ctx >= self.scene.contexts.len() {
            return;
        }

        let bodies: Vec<CtxId> = self
            .scene
            .ctx(ctx)
            .protos
            .values()
            .iter()
            .map(|&p| self.scene.protos[p].body)
            .collect();

        for body in bodies {
            if self
                .ctx_events
                .wrapped_bodies
                .insert((body, self.scene.ctx_slots.generation(body)))
                && self.scene.ctx(body).root_nodes.iter().any(Option::is_some)
            {
                for field in [CtxField::SceneGraph, CtxField::BBox] {
                    for c in self.ctx_chain(body, true) {
                        if self.scene_live {
                            merge(&mut self.ctx_events.pending, c, field, 1, true);
                        } else {
                            merge(&mut self.ctx_events.deferred, c, field, 1, true);
                        }
                    }
                }
            }

            self.ctx_streamed(body);
        }
    }

    /// Run the callbacks of the pending events (X3DField.processEvent: `callback (this .valueOf ())`); true when something ran.
    pub(crate) fn deliver_ctx_events(&mut self) -> bool {
        if self.ctx_events.pending.is_empty() {
            return false;
        }

        if !self.ctx_events.observed {
            return false;
        }
        let Some(mut host) = self.script_host.take() else {
            return false;
        };

        let batch = std::mem::take(&mut self.ctx_events.pending);

        for (ctx, field, rounds) in batch {
            for _ in 0..rounds {
                host.ctx_event(self, ctx, field as u32);
            }
        }

        self.script_host = Some(host);
        self.dynamic_dirty = true;
        true
    }

    /// A world starts (its nodes are set up, its Scripts initialized): the events the parser left pending are processed with the
    /// first events, `profile_changed` and `metadata_changed` of a world that is not the first one, `sceneGraph_changed` once, and
    /// the cascade of X3DWorld (`set_rootNodes__`, see [`Runtime::root_nodes_rounds`]) fires it three more times in the next frames.
    pub(crate) fn ctx_world_started(&mut self, replaced: bool) {
        let w = self.scene.world;

        self.ctx_events.hold = replaced;

        // A PROTO instance realizes its body after the world's setup: one `sceneGraph_changed` each. (A PROTO declaration's body fires
        // when a scene stream first reads it: see `ctx_streamed`.)
        let instances = self
            .scene
            .nodes
            .iter()
            .filter(|n| n.ctx == w && matches!(n.kind, NodeKind::Instance(_)))
            .count() as u32;
        let later = self.root_nodes_rounds(w) - 1 + instances;
        for field in [CtxField::SceneGraph, CtxField::BBox] {
            // A replaced world's first event was queued when its scene was parsed (see `ctx_scene_parsed`).
            if !replaced || !self.scene.scene_infos.contains_key(&w) {
                merge(&mut self.ctx_events.pending, w, field, 1, false);
            }

            merge(&mut self.ctx_events.deferred, w, field, later, true);
        }

        // The Scripts of the world have registered their callbacks: the first event reaches them before anything else happens.
        if !replaced {
            self.deliver_ctx_events();
        }
    }

    /// A scene the Browser parsed (loadURL, replaceWorld, createX3DFromURL): the parser's PROFILE statement, META statements and
    /// the new nodes taint the scene's change fields before any Script of the scene exists; the callbacks they register
    /// while the scene is set up receive those events with the first events processed afterward.
    pub(crate) fn ctx_scene_parsed(&mut self, ctx: CtxId) {
        let (statement, meta) = match self.scene.scene_infos.get(&ctx) {
            Some(i) => (i.profile_statement, !i.meta.is_empty()),
            None => return,
        };

        if statement {
            merge(
                &mut self.ctx_events.parsed,
                ctx,
                CtxField::Profile,
                1,
                false,
            );
        }

        if meta {
            merge(
                &mut self.ctx_events.parsed,
                ctx,
                CtxField::Metadata,
                1,
                false,
            );
        }

        merge(
            &mut self.ctx_events.parsed,
            ctx,
            CtxField::SceneGraph,
            1,
            false,
        );
        merge(&mut self.ctx_events.parsed, ctx, CtxField::BBox, 1, false);
    }

    /// The start of X3DRoutingContext [processEvents]: what the parser tainted reaches the callbacks that exist by now.
    pub(crate) fn deliver_parsed_ctx_events(&mut self) {
        if self.ctx_events.parsed.is_empty() {
            return;
        }

        let (ready, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut self.ctx_events.parsed)
            .into_iter()
            .partition(|e| self.ctx_events.watched.contains(&e.0));

        self.ctx_events.parsed = waiting;

        if ready.is_empty() {
            return;
        }

        if let Some(mut host) = self.script_host.take() {
            for (ctx, field, rounds) in ready {
                for _ in 0..rounds {
                    host.ctx_event(self, ctx, field as u32);
                }
            }

            self.script_host = Some(host);
            self.dynamic_dirty = true;
        }
    }

    /// Events set by the continuations that run after a frame (see `process_phase`): pending for the next frame's first events.
    pub(crate) fn ctx_events_after_frame(&mut self, after: Vec<(CtxId, CtxField, u32)>) {
        if !self.ctx_events.observed {
            return;
        }

        for (c, f, n) in after {
            merge(&mut self.ctx_events.pending, c, f, n, false);
        }
    }

    /// The events kept for the next frame become pending (the end of the frame).
    pub(crate) fn release_deferred_ctx_events(&mut self) -> bool {
        if !self.scene_live {
            return false;
        }

        if std::mem::take(&mut self.ctx_events.hold) {
            return false;
        }

        // Events nobody watches are gone with the frame; the parsed ones wait for their scene to become the world.
        let worlds: Vec<CtxId> = self
            .pending_browser
            .iter()
            .chain(self.browser_frame_waiters.iter())
            .filter(|p| p.world)
            .map(|p| p.ctx)
            .collect();

        self.ctx_events.parsed.retain(|e| worlds.contains(&e.0));

        if !self.ctx_events.observed {
            self.ctx_events.pending.clear();
        }

        for (c, f, n) in std::mem::take(&mut self.ctx_events.deferred) {
            merge(&mut self.ctx_events.pending, c, f, n, false);
        }

        self.ctx_events.observed && !self.ctx_events.pending.is_empty()
    }
}
