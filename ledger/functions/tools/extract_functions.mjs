// The functions of the pinned X_ITE source, one JSON line each: node extract_functions.mjs <acorn dir> <pin src dir>
//
// Reads every .js file of the directory (sorted), parses it with acorn as a module (as a script when that fails) and prints
// one record for every function: declarations, function expressions, arrow functions, object and class methods, getters and
// setters. Nothing is executed. A file that does not parse gives one record with "parseError". The output depends only on the
// files, so two runs print the same bytes.
import { createHash } from "node:crypto";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, basename } from "node:path";
import { pathToFileURL } from "node:url";

const [acornDir, src] = process .argv .slice (2);
const acorn = await import (pathToFileURL (join (acornDir, "dist", "acorn.mjs")) .href);

function files (dir)
{
   return readdirSync (dir) .sort () .flatMap (name =>
   {
      const path = join (dir, name);

      return statSync (path) .isDirectory () ? files (path) : name .endsWith (".js") ? [path] : [ ];
   });
}

// The text of a member chain: `A .prototype .b` gives "A.prototype.b".
function chain (node)
{
   if (!node)
      return null;

   if (node .type === "Identifier")
      return node .name;

   if (node .type === "ThisExpression")
      return "this";

   if (node .type === "MemberExpression" && !node .computed)
   {
      const object = chain (node .object);

      return object ? `${object}.${node .property .name ?? node .property .value}` : null;
   }

   return null;
}

function key (node)
{
   if (node .computed)
      return node .key .type === "Literal" ? String (node .key .value) : `[${chain (node .key) ?? "computed"}]`;

   return node .key .name ?? String (node .key .value);
}

// The class or object that the object literal `object` (with the ancestors `path`) gives its members to.
function ownerOfObject (object, path, file)
{
   for (let i = path .length - 1; i >= 0; -- i)
   {
      const node = path [i];

      if (node .type === "CallExpression")
      {
         const callee = chain (node .callee);

         if (callee === "Object.assign" || callee === "Object.defineProperties" || callee === "Object.setPrototypeOf" || callee === "Object.defineProperty")
         {
            // The first argument names the target, possibly through a nested Object.setPrototypeOf (X .prototype, ...).
            let target = node .arguments [0];

            while (target ?.type === "CallExpression")
               target = target .arguments [0];

            const name = chain (target);

            if (name)
               return { owner: name .replace (/\.prototype$/, ""), isStatic: !name .endsWith (".prototype") && name !== "this" };
         }
      }

      if (node .type === "VariableDeclarator" && node .id .type === "Identifier" && node .init === (path [i + 1] ?? object))
         return { owner: node .id .name, isStatic: true };

      if (node .type === "AssignmentExpression" && node .right === (path [i + 1] ?? object))
      {
         const name = chain (node .left);

         if (name)
            return { owner: name .replace (/\.prototype$/, ""), isStatic: !name .endsWith (".prototype") };
      }

      if (node .type === "ExportDefaultDeclaration")
         return { owner: basename (file, ".js"), isStatic: true };

      // A descriptor object of Object.defineProperties: go on to the outer call.
      if (node .type === "Property" || node .type === "ObjectExpression")
         continue;

      if (node .type === "CallExpression" || node .type === "NewExpression" || node .type === "ReturnStatement" || node .type === "ArrayExpression")
         continue;

      break;
   }

   return null;
}

function enclosing (stack)
{
   return stack .length ? stack [stack .length - 1] : null;
}

function describe (node, path, file, stack)
{
   const parent = path [path .length - 1];
   const record = { owner: null, name: null, kind: "function", isStatic: false };

   if (node .type === "FunctionDeclaration")
   {
      record .name = node .id ?.name ?? "default";
      record .kind = /^[A-Z]/ .test (record .name) ? "constructor-or-function" : "function";
   }
   else if (parent ?.type === "MethodDefinition")
   {
      const cls = path .findLast (n => n .type === "ClassDeclaration" || n .type === "ClassExpression");

      record .owner    = cls ?.id ?.name ?? basename (file, ".js");
      record .name     = key (parent);
      record .kind     = parent .kind === "method" ? "method" : parent .kind;
      record .isStatic = parent .static;
   }
   else if (parent ?.type === "Property" && parent .value === node)
   {
      const
         object = path [path .length - 2],
         above  = path .slice (0, -2),
         found  = ownerOfObject (object, above, file),
         name   = key (parent);

      // { get () {}, set () {} } of a property descriptor: the property name is the key of the Property above.
      const descriptor = above [above .length - 1];

      if ((name === "get" || name === "set" || name === "value") && descriptor ?.type === "Property" && descriptor .value === object && parent .kind === "init"
         && path .some (n => n .type === "CallExpression" && /^Object\.definePropert/ .test (chain (n .callee) ?? "")))
      {
         record .name = key (descriptor);
         record .kind = name === "value" ? "method" : name === "get" ? "get" : "set";
      }
      else
      {
         record .name = name;
         record .kind = parent .kind === "init" ? "method" : parent .kind;
      }

      record .owner    = found ?.owner ?? null;
      record .isStatic = found ?.isStatic ?? false;

      if (!found)
         record .kind = "object-member";
   }
   else if (parent ?.type === "VariableDeclarator" && parent .init === node && parent .id .type === "Identifier")
   {
      record .name = parent .id .name;
   }
   else if (parent ?.type === "AssignmentExpression" && parent .right === node)
   {
      const name = chain (parent .left);

      if (name)
      {
         const parts = name .split (".");

         record .name     = parts .pop ();
         record .isStatic = parts [parts .length - 1] !== "prototype" && parts [0] !== "this";
         record .owner    = parts .filter (p => p !== "prototype") .join (".") || null;
         record .kind     = record .owner ? "method" : "function";
      }
   }
   else if (parent ?.type === "ExportDefaultDeclaration")
   {
      record .name = node .id ?.name ?? basename (file, ".js");
   }

   if (record .name === null)
   {
      // A function without a name of its own: a callback or an immediately called function, part of the function around it.
      record .kind = node .type === "ArrowFunctionExpression" ? "anonymous-arrow" : "anonymous-function";
      record .name = node .id ?.name ?? null;
   }

   const outer = enclosing (stack);

   // A named function inside a function is local to it.
   if (outer && !record .kind .startsWith ("anonymous") && !record .owner)
      record .kind = "local-function";

   return record;
}

function walk (node, path, file, text, stack, out)
{
   if (!node || typeof node .type !== "string")
      return;

   const isFunction = node .type === "FunctionDeclaration" || node .type === "FunctionExpression" || node .type === "ArrowFunctionExpression";
   let pushed = false;

   if (isFunction)
   {
      const
         record = describe (node, path, file, stack),
         outer  = enclosing (stack),
         body   = text .slice (node .start, node .end),
         entry  = {
            file,
            line: node .loc .start .line,
            endLine: node .loc .end .line,
            owner: record .owner,
            name: record .name,
            kind: record .kind,
            static: record .isStatic,
            async: node .async,
            generator: node .generator,
            params: node .params .length,
            enclosing: outer,
            sha256: createHash ("sha256") .update (body) .digest ("hex"),
         };

      out .push (entry);

      const label = record .kind .startsWith ("anonymous")
         ? outer
         : [record .owner, record .name] .filter (Boolean) .join (".") + `@${entry .line}`;

      stack .push (label);
      pushed = true;
   }

   path .push (node);

   for (const k of Object .keys (node))
   {
      if (k === "loc" || k === "start" || k === "end" || k === "type")
         continue;

      const child = node [k];

      if (Array .isArray (child))
         child .forEach (c => walk (c, path, file, text, stack, out));
      else if (child && typeof child === "object")
         walk (child, path, file, text, stack, out);
   }

   path .pop ();

   if (pushed)
      stack .pop ();
}

for (const path of files (src))
{
   const
      file = relative (src, path) .split ("\\") .join ("/"),
      text = readFileSync (path, "utf8"),
      head = { file, bytes: Buffer .byteLength (text), lines: text .split ("\n") .length, sha256: createHash ("sha256") .update (text) .digest ("hex") };

   let ast = null, error = null;

   for (const sourceType of ["module", "script"])
   {
      try
      {
         ast = acorn .parse (text, { ecmaVersion: "latest", sourceType, locations: true, allowHashBang: true, allowReturnOutsideFunction: true });
         head .sourceType = sourceType;
         break;
      }
      catch (e)
      {
         error = String (e .message);
      }
   }

   if (!ast)
   {
      console .log (JSON .stringify ({ ... head, parseError: error }));
      continue;
   }

   const out = [ ];

   walk (ast, [ ], file, text, [ ], out);
   console .log (JSON .stringify ({ ... head, functions: out .length }));

   for (const entry of out)
      console .log (JSON .stringify (entry));
}
