//! The event engine: X3DBaseNode.addEvent / addEventObject / addNodeEvent, X3DField.processEvent,
//! X3DChildObject.processEvent (nodes), and X3DRoutingContext processEvents.
//!
//! Events are processed in the browser's single tainted list, exactly like X_ITE: a field event
//! reads the *origin field's current value* when it is processed (X3DField.processEvent:
//! `this .set (field .getValue ())`; "There must be no copy, because the event is not executed
//! immediately").

use crate::*;

impl Runtime {
    /// X3DBaseNode.getExtendedEventHandling: Script and X3DPrototypeInstance return false.
    pub fn extended_event_handling(&self, node: NodeId) -> bool {
        match self.scene.node(node).kind {
            NodeKind::Instance(_) => false,
            NodeKind::Builtin(_) => self.builtin_name(node) != Some("Script"),
            _ => true,
        }
    }

    /// X3DField.addEvent → X3DChildObject.addEvent → parent X3DBaseNode.addEvent (field).
    pub fn add_event(&mut self, k: FKey) {
        if self.is_field_tainted(k) {
            self.scene.field_mut(k.0, k.1).modified = true;
            return;
        }

        self.set_field_tainted(k, true);
        self.add_event_object(k, Event { origin: k, visited: vec![] });
    }

    /// X3DBaseNode.addEventObject (field, event).
    pub fn add_event_object(&mut self, k: FKey, event: Event) {
        self.scene.field_mut(k.0, k.1).modified = true;

        // browser .addTaintedField (field, event)
        self.tainted_fields.push((k, event));

        if self.nrt(k.0).is_some_and(|n| n.tainted) {
            return;
        }

        let access_type = self.field(k).access;

        if access::is_input(access_type) || (self.extended_event_handling(k.0) && access::is_initializable(access_type)) {
            self.add_node_event(k.0);
        }
    }

    /// X3DBaseNode.addNodeEvent.
    pub fn add_node_event(&mut self, node: NodeId) {
        let n = self.nrt_mut(node);

        if n.tainted {
            return;
        }

        n.tainted = true;
        self.tainted_nodes.push(node);
    }

    /// `X3DChildObject .prototype .addEvent .call (node)`: an event on every field holding the node.
    pub fn add_node_parents_event(&mut self, node: NodeId) {
        let holders = self.parents.get(&node).cloned().unwrap_or_default();

        for h in holders {
            if let Holder::Field(k) = h {
                self.add_event(k);
            }
        }
    }

    /// Push a field without an event object (browser .addTaintedField (field)).
    pub fn add_tainted_field(&mut self, k: FKey) {
        self.tainted_fields.push((k, Event { origin: k, visited: vec![] }));
    }

    /// X3DField.processEvent (event).
    pub fn process_event(&mut self, k: FKey, mut event: Event) {
        if !event.visited.contains(&k) {
            event.visited.push(k);
        }

        self.set_field_tainted(k, false);

        if event.origin != k {
            let v = self.value(event.origin).clone();

            // X3DField.set for a different but compatible type is not reachable: routes and IS
            // relations require equal types.
            if v.field_type() == self.field(k).ty() {
                self.set_raw(k, v);
            }
        }

        // Process interests (snapshot, like X3DObject.processInterests over the current Map).
        let interests = self.frt.get(&k).map(|f| f.interests.clone()).unwrap_or_default();

        for i in interests {
            self.call_interest(i, k);
        }

        // Process routes.
        let field_interests = self.frt.get(&k).map(|f| f.field_interests.clone()).unwrap_or_default();

        for f in field_interests {
            if event.visited.contains(&f) {
                continue;
            }

            self.add_event_object(f, event.clone());
        }

        // Process field callbacks.
        if self.field_observed.contains(&k) {
            self.field_callbacks(k);
        }
    }

    /// The field callbacks of `k` (X3DField.processEvent: `callback (this .valueOf ())`).
    fn field_callbacks(&mut self, k: FKey) {
        match self.script_host.take() {
            Some(mut host) => {
                for p in std::mem::take(&mut self.pending_field_events) {
                    host.field_event(self, p);
                }

                host.field_event(self, k);
                self.script_host = Some(host);
            }
            None => self.pending_field_events.push(k),
        }
    }

    /// X3DChildObject.processEvent for a node: untaint and call node interests.
    fn process_node_event(&mut self, node: NodeId) {
        self.nrt_mut(node).tainted = false;

        let interests = self.nrt(node).map(|n| n.interests.clone()).unwrap_or_default();

        for i in interests {
            self.call_interest(i, (node, FieldLoc::Priv(usize::MAX)));
        }
    }

    /// X3DRoutingContext [processEvents].
    pub fn process_events(&mut self) {
        if self.processing {
            return;
        }

        self.processing = true;
        self.deliver_parsed_ctx_events();

        loop {
            loop {
                // Process field events. The tainted BrowserOptions fields run their interests here too.
                loop {
                    self.browser_options.process_events();

                    if self.browser_options.take_sound_intensity_changed() {
                        self.sound_set_intensity();
                    }

                    let tainted = std::mem::take(&mut self.tainted_fields);

                    for (k, event) in tainted {
                        self.process_event(k, event);
                    }

                    if self.tainted_fields.is_empty() && !self.browser_options.has_pending_events() {
                        break;
                    }
                }

                // Process node events.
                loop {
                    let tainted = std::mem::take(&mut self.tainted_nodes);

                    for node in tainted {
                        self.process_node_event(node);
                    }

                    if !(self.tainted_fields.is_empty() && !self.tainted_nodes.is_empty()) {
                        break;
                    }
                }

                if self.tainted_fields.is_empty() {
                    break;
                }
            }

            // The callbacks of the execution context change fields (X3DField.processEvent) run after the events that set them.
            if !self.deliver_ctx_events() {
                break;
            }
        }

        self.processing = false;
    }

    pub fn has_pending_events(&self) -> bool {
        !self.tainted_fields.is_empty() || !self.tainted_nodes.is_empty() || self.browser_options.has_pending_events()
    }

    /// X3DBrowserContext [_traverse] without rendering: the event phases of one frame. The scene
    /// loaded by createX3DFromURL is not the browser's world, so no traversal touches it.
    pub fn frame(&mut self) {
        for name in ["prepareEvents", "timePrepareEvents", "timeEvents", "cameraEvents", "sensorEvents", "displayEvents", "displayFinishEvents", "finishEvents"] {
            self.process_phase(name);
        }
    }
}

impl Runtime {
    /// One named browser event phase of X3DBrowserContext [_traverse]: add the browser's tainted
    /// field and process events. Phases: prepareEvents, timePrepareEvents, timeEvents,
    /// cameraEvents, sensorEvents, displayEvents, displayFinishEvents, finishEvents.
    pub fn process_phase(&mut self, name: &str) {
        // The `loadData` events of EXTERNPROTO declarations (X3DUrlObject: `_loadData = time`) are
        // processed with the first events of the frame.
        if name == "prepareEvents" {
            for e in std::mem::take(&mut self.extern_load_events) {
                self.extern_load_data(e);
            }
        }

        let k = self.private_field(self.browser_node, name).expect("browser event field");
        self.add_tainted_field(k);
        self.process_events();

        if name == "prepareEvents" {
            self.apply_decoded_images();
        }

        // The `nextFrame ()` promises resolve on finishEvents; their continuations (Inline.setInternalScene)
        // and the fetches they start run right after the frame, before the next one.
        if name == "finishEvents" {
            self.finish_frame();

            // The change events of the frame's end run their callbacks with the events that follow.
            if self.release_deferred_ctx_events() {
                self.process_events();
            }
        }
    }
}
