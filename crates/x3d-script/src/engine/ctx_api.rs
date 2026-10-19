//! The write side of X3DExecutionContext / X3DScene for scripts (Execution/X3DExecutionContext.js, X3DScene.js): imported nodes,
//! proto and extern proto declarations, routes, exported nodes, named nodes and the unique-name helpers. The messages are X_ITE's.

use super::*;

/// X3DCast (X3DConstants .X3DNode, value, false): a node, not null.
fn node_arg(a: &[Value<'_>], i: usize) -> Option<NodeId> {
    a.get(i).and_then(arg_node).flatten()
}

/// Log what the scene layer warned about.
fn flush_warnings() {
    for w in std::mem::take(&mut rt().scene.warnings) {
        rt().log_warn(w);
    }
}

fn changed() {
    rt().scene_graph_changed();
}

/// The exported node `node` of the Inline in an imported record: X3DImportedNode.getExportedNode.
fn exported_of(imp: usize) -> Option<NodeId> {
    let r = rt();
    let rec = &r.scene.imported[imp];
    let scene = *r.inline_scenes.get(&rec.inline)?;

    r.scene.get_exported_node(scene, &rec.exported_name)
}

/// X3DExecutionContext.getNodeOrImportedNode.
fn node_or_imported(c: CtxId, node: NodeId) -> NodeId {
    let ids: Vec<usize> = rt().scene.ctx(c).imported.values().to_vec();

    for id in ids {
        if exported_of(id) == Some(node) {
            return rt().scene.imported[id].instance;
        }
    }

    node
}

/// X3DImportedNode.dispose: the record leaves the context and the routes of its instance are deleted.
fn remove_imported(c: CtxId, name: &str) -> bool {
    let Some(id) = rt().scene.remove_imported_node(c, name) else {
        return false;
    };

    let instance = rt().scene.imported[id].instance;
    let routes: Vec<usize> = rt().scene.ctx(c).routes.iter().filter(|r| r.source_node == instance || r.destination_node == instance).map(|r| r.id).collect();

    for r in routes {
        rt().delete_route_by_id(c, r);
    }

    rt().dynamic_dirty_mark();
    true
}

/// A proto declaration record for scripts: (name, fields, has_metadata, urls).
fn proto_ids(c: CtxId, extern_: bool) -> Vec<usize> {
    if extern_ { rt().scene.ctx(c).externprotos.values().to_vec() } else { rt().scene.ctx(c).protos.values().to_vec() }
}

pub(super) fn op<'js>(ctx: &Ctx<'js>, a: &[Value<'js>]) -> rquickjs::Result<Value<'js>> {
    let undefined = Value::new_undefined(ctx.clone());
    let c = ctx_of_script(num_arg(ctx, a, 0));
    let what = str_arg(ctx, a, 1);
    let a = &a[2..];
    let str_at = |i: usize| str_arg(ctx, a, i);

    macro_rules! string {
        ($v:expr) => {
            Ok(rquickjs::String::from_str(ctx.clone(), &$v)?.into_value())
        };
    }

    match what.as_str() {
        "unique" => {
            let name = str_at(1);
            let r = &rt().scene;
            let v = match str_at(0).as_str() {
                "named" => r.get_unique_name(c, &name),
                "imported" => r.get_unique_import_name(c, &name),
                "proto" => r.get_unique_proto_name(c, &name),
                "extern" => r.get_unique_extern_proto_name(c, &name),
                _ => {
                    let root = r.scene_root(c);
                    r.get_unique_export_name_of(root, &name)
                }
            };
            string!(v)
        }
        // --- named nodes: X3DExecutionContext.addNamedNode / updateNamedNode / removeNamedNode
        "named" => {
            let (mode, name) = (str_at(0), str_at(1));

            if mode == "remove" {
                rt().scene.remove_named_node(c, &name);
                changed();
                return Ok(undefined);
            }

            let verb = if mode == "add" { "add" } else { "update" };
            let Some(node) = node_arg(a, 2) else {
                return throw(ctx, &format!("Couldn't {verb} named node: node must be of type X3DNode."));
            };

            if rt().scene.is_imported(node) {
                return throw(ctx, "Couldn't add named node: node cannot be of type X3DImportedNodeInstance.");
            }

            if rt().scene.node(node).ctx != c {
                return throw(ctx, &format!("Couldn't {verb} named node: node does not belong to this execution context."));
            }

            if name.is_empty() {
                return throw(ctx, &format!("Couldn't {verb} named node: node name is empty."));
            }

            if mode == "add" {
                if rt().scene.get_named_node(c, &name).is_some() {
                    return throw(ctx, &format!("Couldn't add named node: node named '{name}' is already in use."));
                }

                let own = rt().scene.node(node).name.clone();

                if rt().scene.get_named_node(c, &own) == Some(node) {
                    return throw(ctx, &format!("Couldn't add named node: node named '{own}' already has a name."));
                }

                // node .setName (name), then the array takes it.
                rt().scene.node_mut(node).name = name.clone();
                let _ = rt().scene.ctx_mut(c).named_nodes.add(&name, node);
            } else {
                rt().scene.update_named_node(c, &name, node);
            }

            changed();
            Ok(undefined)
        }
        // --- imported nodes
        "importedGet" => {
            let name = str_at(0);

            match rt().scene.ctx(c).imported.get(&name) {
                Some(id) => Ok(node_js(ctx, Some(rt().scene.imported[id].instance))),
                None => throw(ctx, &format!("Imported node '{name}' not found.")),
            }
        }
        "localNode" => {
            let name = str_at(0);

            match rt().scene.get_local_node(c, &name) {
                Some(n) => Ok(node_js(ctx, Some(n))),
                None => throw(ctx, &format!("Unknown named or imported node '{name}'.")),
            }
        }
        "nodeOrImported" => match node_arg(a, 0) {
            Some(n) => Ok(node_js(ctx, Some(node_or_imported(c, n)))),
            None => throw(ctx, "Couldn't get localized node: node must be of type X3DNode."),
        },
        "importedAdd" | "importedUpdate" => {
            let (exported, imported_arg) = (str_at(1), a.get(2).filter(|v| !v.is_undefined()).map(|v| coerce_string(ctx, v)));
            let imported = imported_arg.unwrap_or_else(|| exported.clone());
            let description = a.get(3).map(|v| coerce_string(ctx, v)).unwrap_or_default();

            if what == "importedAdd" && rt().scene.ctx(c).imported.has(&imported) {
                return throw(ctx, &format!("Couldn't add imported node: imported name '{imported}' already in use."));
            }

            let inline = node_arg(a, 0).filter(|&n| matches!(rt().scene.node(n).kind, NodeKind::Builtin(_)) && rt().scene.type_name(n) == "Inline");

            let Some(inline) = inline else {
                return throw(ctx, "Node must be of type Inline node.");
            };

            if rt().scene.node(inline).ctx != c {
                return throw(ctx, "Couldn't update imported node: Inline node does not belong to this execution context.");
            }

            if exported.is_empty() {
                return throw(ctx, "Couldn't update imported node: exported name is empty.");
            }

            if imported.is_empty() {
                return throw(ctx, "Couldn't update imported node: imported name is empty.");
            }

            remove_imported(c, &imported);

            match rt().scene.add_imported_node(c, inline, &exported, &imported, &description) {
                Ok(id) => {
                    let instance = rt().scene.imported[id].instance;

                    rt().nrt_mut(instance);
                    changed();
                    Ok(undefined)
                }
                Err(e) => throw(ctx, &e),
            }
        }
        "importedRename" => {
            let (old, new) = (str_at(0), str_at(1));

            match rt().scene.rename_imported_node(c, &old, &new) {
                Ok(()) => {
                    changed();
                    Ok(undefined)
                }
                Err(e) => throw(ctx, &e),
            }
        }
        "importedRemove" => {
            let name = str_at(0);

            if remove_imported(c, &name) {
                changed();
            }

            Ok(undefined)
        }
        "rootSet" => {
            // X3DScene.setRootNodes (value): rootNodes .assign (value)
            if let Some(FieldValue::MFNode(list)) = value_arg(a, 0) {
                rt().scene.ctx_mut(c).root_nodes = list;
                rt().rebuild_parents();
            }

            Ok(undefined)
        }
        "importedExported" => {
            let id = num_arg(ctx, a, 0) as usize;

            match exported_of(id) {
                Some(n) => Ok(node_js(ctx, Some(n))),
                None => throw(ctx, &format!("Exported node '{}' not found.", rt().scene.imported[id].exported_name)),
            }
        }
        "importedDescription" => {
            rt().scene.imported[num_arg(ctx, a, 0) as usize].description = str_at(1);
            Ok(undefined)
        }
        // --- exported nodes
        "exportedUpdate" => {
            let (name, description) = (str_at(0), a.get(2).map(|v| coerce_string(ctx, v)).unwrap_or_default());
            let root = rt().scene.scene_root(c);

            if str_at(3) == "add" && rt().scene.ctx(root).exported.has(&name) {
                return throw(ctx, &format!("Couldn't add exported node: exported name '{name}' already in use."));
            }

            let Some(node) = node_arg(a, 1) else {
                return throw(ctx, "Couldn't update exported node: node must be of type X3DNode.");
            };

            match rt().scene.update_exported_node(c, &name, node, &description) {
                Ok(()) => {
                    changed();
                    Ok(undefined)
                }
                Err(e) => throw(ctx, &e),
            }
        }
        "exportedRemove" => {
            let name = str_at(0);
            let root = rt().scene.scene_root(c);

            if rt().scene.ctx(root).exported.has(&name) {
                rt().scene.ctx_mut(root).exported.remove(&name);
                rt().dynamic_dirty_mark();
                changed();
            }

            Ok(undefined)
        }
        // --- routes (identified by their id)
        "routeAdd" | "routeGet" => {
            let (sf, df) = (str_at(1), str_at(3));
            let adding = what == "routeAdd";
            let who = if adding { "source node" } else { "sourceNode" };
            let (Some(src), Some(dst)) = (node_arg(a, 0), node_arg(a, 2)) else {
                let which = if node_arg(a, 0).is_none() { who } else if adding { "destination node" } else { "destinationNode" };
                return throw(ctx, &format!("Bad ROUTE specification: {which} must be of type X3DNode."));
            };
            let (src, dst) = (node_or_imported(c, src), node_or_imported(c, dst));

            if adding {
                if let Err(e) = rt().add_route(c, src, &sf, dst, &df) {
                    return throw(ctx, &e);
                }
            } else if let Err(e) = rt().scene.route_ends(src, &sf, dst, &df) {
                return throw(ctx, &e);
            }

            match rt().scene.find_route_index(c, src, &sf, dst, &df) {
                Some(i) => Ok(Value::new_number(ctx.clone(), rt().scene.ctx(c).routes[i].id as f64)),
                None => Ok(Value::new_null(ctx.clone())),
            }
        }
        "routeDelete" => {
            let id = num_arg(ctx, a, 0) as usize;
            Ok(Value::new_bool(ctx.clone(), rt().delete_route_by_id(c, id)))
        }
        "routeList" => {
            let routes: Vec<_> = rt().scene.ctx(c).routes.iter().map(|r| (r.id, r.source_node, r.source_field.clone(), r.destination_node, r.destination_field.clone())).collect();
            let arr = Array::new(ctx.clone())?;

            for (i, (id, sn, sf, dn, df)) in routes.into_iter().enumerate() {
                let o = Object::new(ctx.clone())?;
                o.set("id", id as f64)?;
                o.set("sourceNode", node_js(ctx, Some(sn)))?;
                o.set("sourceField", sf)?;
                o.set("destinationNode", node_js(ctx, Some(dn)))?;
                o.set("destinationField", df)?;
                arr.set(i, o)?;
            }

            Ok(arr.into_value())
        }
        // --- proto and extern proto declarations
        "protoIds" => {
            let arr = Array::new(ctx.clone())?;

            for (i, id) in proto_ids(c, a.first().is_some_and(coerce_bool)).into_iter().enumerate() {
                arr.set(i, id as f64)?;
            }

            Ok(arr.into_value())
        }
        "protoGet" => {
            let (extern_, name) = (a.first().is_some_and(coerce_bool), str_at(1));
            let found = if extern_ { rt().scene.ctx(c).externprotos.get(&name) } else { rt().scene.ctx(c).protos.get(&name) };

            match found {
                Some(id) => Ok(Value::new_number(ctx.clone(), id as f64)),
                None if extern_ => throw(ctx, &format!("Extern proto declaration '{name}' not found.")),
                None => throw(ctx, &format!("Proto declaration '{name}' not found.")),
            }
        }
        "protoOp" => {
            // (extern, mode add|update|remove, name, id)
            let (extern_, mode, name) = (a.first().is_some_and(coerce_bool), str_at(1), str_at(2));
            let id = a.get(3).map(|v| coerce_f64(ctx, v)).filter(|n| !n.is_nan()).map(|n| n as usize);
            let kind = if extern_ { "extern proto" } else { "proto" };
            let decl_ctx = |i: usize| if extern_ { rt().scene.externprotos[i].ctx } else { rt().scene.protos[i].ctx };
            let decl_name = |i: usize| if extern_ { rt().scene.externprotos[i].name.clone() } else { rt().scene.protos[i].name.clone() };
            let has = |n: &str| if extern_ { rt().scene.ctx(c).externprotos.has(n) } else { rt().scene.ctx(c).protos.has(n) };
            let current = |n: &str| if extern_ { rt().scene.ctx(c).externprotos.get(n) } else { rt().scene.ctx(c).protos.get(n) };

            if mode == "remove" {
                if extern_ {
                    rt().scene.ctx_mut(c).externprotos.remove(&name);
                } else {
                    rt().scene.ctx_mut(c).protos.remove(&name);
                }

                rt().dynamic_dirty_mark();
                changed();
                return Ok(undefined);
            }

            let verb = if mode == "add" { "add" } else { "update" };
            let Some(id) = id else {
                let class = if extern_ { "X3DExternProtoDeclaration" } else { "X3DProtoDeclaration" };
                return throw(ctx, &format!("Couldn't {verb} {kind} declaration: {kind} must be of type {class}."));
            };

            if decl_ctx(id) != c {
                return throw(ctx, &format!("Couldn't {verb} {kind} declaration: {kind} does not belong to this execution context."));
            }

            if mode == "add" {
                if has(&name) {
                    return throw(ctx, &format!("Couldn't add {kind} declaration: {kind} '{name}' already in use."));
                }

                let own = decl_name(id);

                if current(&own) == Some(id) {
                    return throw(ctx, &format!("Couldn't add {kind} declaration: {kind} '{own}' already added."));
                }
            }

            if name.is_empty() {
                return throw(ctx, &format!("Couldn't {verb} {kind} declaration: {kind} name is empty."));
            }

            if mode == "add" {
                // The warnings of the add path (the scene layer words the update path).
                let other = if extern_ { rt().scene.ctx(c).protos.has(&name) } else { rt().scene.ctx(c).externprotos.has(&name) };

                if other {
                    let text = if extern_ { format!("Added extern proto '{name}' will be overridden by proto with same name.") } else { format!("Added proto '{name}' will override extern proto with same name.") };
                    rt().log_warn(text);
                }

                if rt().scene.is_concrete_node(&name) {
                    rt().log_warn(format!("Added {kind} '{name}', but will not override built-in node of same type if profile/components include this node."));
                }

                if extern_ {
                    let _ = rt().scene.ctx_mut(c).externprotos.add(&name, id);
                    rt().scene.externprotos[id].name = name.clone();
                } else {
                    let _ = rt().scene.ctx_mut(c).protos.add(&name, id);
                    rt().scene.protos[id].name = name.clone();
                }
            } else if extern_ {
                rt().scene.update_extern_proto_declaration(c, &name, id);
            } else {
                rt().scene.update_proto_declaration(c, &name, id);
            }

            flush_warnings();
            changed();
            Ok(undefined)
        }
        "protoInfo" => {
            let (extern_, id) = (a.first().is_some_and(coerce_bool), num_arg(ctx, a, 1) as usize);
            let r = &rt().scene;
            let (name, fields, has_metadata, urls) = if extern_ {
                let d = &r.externprotos[id];
                (d.name.clone(), d.fields.clone(), d.has_metadata, Some(d.url.clone()))
            } else {
                let d = &r.protos[id];
                (d.name.clone(), d.fields.clone(), d.has_metadata, None)
            };
            let o = Object::new(ctx.clone())?;

            o.set("name", name.as_str())?;
            o.set("isExternProto", urls.is_some())?;

            let fa = Array::new(ctx.clone())?;
            let mut defs: Vec<(String, i32, &'static str, FieldValue)> = vec![];

            if has_metadata {
                defs.push(("metadata".into(), access::INPUT_OUTPUT as i32, "SFNode", FieldValue::SFNode(None)));
            }

            defs.extend(fields.iter().map(|f| (f.name.clone(), f.access as i32, f.ty().name(), f.value.clone())));

            for (j, (fname, access, ty, value)) in defs.into_iter().enumerate() {
                let fo = Object::new(ctx.clone())?;
                fo.set("name", fname.as_str())?;
                fo.set("accessType", access)?;
                fo.set("dataType", ty)?;
                fo.set("value", new_value_js(ctx, value))?;
                fa.set(j, fo)?;
            }

            o.set("fields", fa)?;

            if let Some(urls) = urls {
                o.set("urls", new_value_js(ctx, FieldValue::MFString(urls)))?;
            }

            Ok(o.into_value())
        }
        other => throw(ctx, &format!("x3d-script: unknown context op '{other}'")),
    }
}

