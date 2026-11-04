#!/usr/bin/env python3
"""Generate oracle/synthetic-rea-file-http/: the route table of a local HTTP server and the worlds that load URL lists through it with
the pinned FileLoader.js (REA ledger entry sub:io.file-loader, X_ITE 16.5.0): redirects, statuses, content types, headers, content
encodings, refused schemes, and the `text/html` hand-off to `foreign`.

Every route and body is written here from first principles; nothing is copied from a third-party asset. The table (routes.json) is
served by oracle/x_ite-anim.mjs (script key `httpRoutes`) for the pinned page and by crates/x3d-http/src/route_server.rs for the port. The
worlds run one Script that loads the URL lists of its group one after the other, `Browser.createX3DFromURL` (fh-redirect, fh-status,
fh-diff) or `Browser.loadURL` (fh-foreign, which can only end in a hand-off or a failure: a loaded world would replace the Script).

  python3 -I oracle/tools/gen_file_http.py
"""
import gzip, json, os

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
out_dir = os.path.join(root, "oracle", "synthetic-rea-file-http")
os.makedirs(out_dir, exist_ok=True)


def vrml(title):
    return f'#VRML V2.0 utf8\nWorldInfo {{ title "{title}" }}\nGroup {{ children Shape {{ geometry Box {{ size 1 2 3 }} }} }}\n'


OK = vrml("loaded")
# The bodies of the routes that are handed off are empty: FileLoader never reads the body of a response it hands to `foreign`, and a response with an
# unread body keeps the recorder's request counter above zero, so `settle` would wait for it forever. The same holds for an error status.
HTML = ""
# A body that is read must not parse: an empty body is an empty scene, and loadURL would replace the world with it.
PAGE = "<!DOCTYPE html><html><body>page</body></html>\n"
DATA_URL = "data:model/vrml,%23VRML%20V2.0%20utf8%0AWorldInfo%20%7B%20title%20%22data%22%20%7D%0A"
routes = {}


def route(name, status=200, ct=None, body="", headers=None, **extra):
    h = {}
    if ct is not None:
        h["Content-Type"] = ct
    h.update(headers or {})
    r = {"status": status}
    if h:
        r["headers"] = h
    if isinstance(body, bytes):
        r["bodyHex"] = body.hex()
    elif body:
        r["body"] = body
    r.update(extra)
    routes[name] = r


def redirect(name, location, status=302):
    route(name, status, headers={"Location": location})


# --- targets
route("ok", ct="model/vrml", body=OK)
route("ok-cors", ct="model/vrml", body=vrml("cors"), headers={"Access-Control-Allow-Origin": "*"})
route("other", ct="model/vrml", body=vrml("other"))
route("html", ct="text/html", body=HTML)
# --- redirect statuses and Location forms
for code in (301, 302, 303, 307, 308):
    redirect(f"r{code}", "/h/ok", code)
redirect("r-rel", "ok")
redirect("r-rel-up", "../h/other")
redirect("r-abs", "{origin}/h/other")
redirect("r-protocol-relative", "//{hostport}/h/other")
redirect("r-query", "/h/ok?x=1#Frag")
redirect("r-fragment-only", "#Frag")
redirect("r-chain", "/h/r-chain-b", 301)
redirect("r-chain-b", "/h/r-chain-c", 302)
redirect("r-chain-c", "/h/ok", 307)
redirect("loop-a", "/h/loop-b")
redirect("loop-b", "/h/loop-a")
redirect("self", "/h/self")
for count, tag in ((20, "chain20"), (21, "chain21")):
    for n in range(count):
        redirect(f"{tag}/{n}", f"/h/{tag}/{n + 1}")
    route(f"{tag}/{count}", ct="model/vrml", body=OK)
# --- 3xx that is no redirect
route("r-nolocation", 301)
redirect("r-empty-location", "")
redirect("r300", "/h/ok", 300)
redirect("r304", "/h/ok", 304)
redirect("r-to-404", "/h/s404")
redirect("r-to-500", "/h/s500")
redirect("r-to-html", "/h/html")
# --- redirects that must fail
redirect("r-file", "file:///nonexistent/x.wrl")
redirect("r-data", DATA_URL)
redirect("r-ftp", "ftp://127.0.0.1/x.wrl")
redirect("r-javascript", "javascript:x")
redirect("r-ecmascript", "ecmascript:x")
redirect("r-blob", "blob:http://127.0.0.1/none")
redirect("r-about", "about:blank")
redirect("r-ws", "ws://{hostport}/h/ok")
redirect("r-creds", "http://user:pw@{hostport}/h/ok")
redirect("r-creds-user", "http://user@{hostport}/h/ok")
redirect("r-badurl", "http://[::1")
redirect("r-refused", "http://127.0.0.1:1/x.wrl")
# --- redirect across origins (the second server is a second port)
redirect("r-cross-cors", "{origin2}/h/ok-cors")
redirect("r-cross-nocors", "{origin2}/h/other")
# --- statuses
for code in (400, 403, 404, 410, 500, 502, 503):
    route(f"s{code}", code, ct="text/plain")
route("s204", 204)
route("s201", 201, ct="model/vrml", body=OK)
route("s200-empty", 200, ct="model/vrml")
route("s-html-404", 404, ct="text/html", body=HTML)
# --- content types: createX3DFromURL has no `foreign`, so none of them changes the result
for name, ct in (("model-vrml", "model/vrml"), ("x3d-vrml", "model/x3d-vrml"), ("x3d-xml", "model/x3d+xml"), ("octet", "application/octet-stream"),
                 ("plain", "text/plain"), ("html", "text/html"), ("xhtml", "application/xhtml+xml"), ("charset", "model/vrml; charset=utf-8"),
                 ("garbage", "garbage"), ("none", None)):
    route(f"ct-{name}", ct=ct, body=OK)
# --- headers
route("hdr-custom", ct="model/vrml", body=OK, headers={"X-Custom": "abc", "X-File-Size": "3", "X-Repeat": ["one", "two"]})
# --- content encodings and wire failures
GZ = gzip.compress(OK.encode(), mtime=0)
route("enc-gzip", ct="model/vrml", body=GZ, headers={"Content-Encoding": "gzip"})
route("enc-gzip-twice", ct="model/vrml", body=gzip.compress(GZ, mtime=0), headers={"Content-Encoding": "gzip"})
route("enc-file-gzip", ct="application/gzip", body=GZ)
route("enc-bad", ct="model/vrml", body=OK, headers={"Content-Encoding": "gzip"})
route("wire-truncated", ct="model/vrml", body=OK, truncateTo=20)
route("wire-hangup", hangup=True)
# --- known differences from the pinned page (fh-diff)
route("s418", 418, ct="text/plain")
route("s-reason", 404, ct="text/plain", reason="Gone Fishing")
route("s-reason-empty", 404, ct="text/plain", reason="")
# --- the text/html hand-off (fh-foreign): every body is HTML, so a candidate that is not handed off cannot be read
route("html-xhtml", ct="application/xhtml+xml", body=HTML)
route("html-charset", ct="text/html; charset=utf-8", body=HTML)
route("html-upper", ct="TEXT/HTML", body=PAGE)
route("html-space", ct="text/html ;charset=utf-8", body=PAGE)
route("html-plain", ct="text/plain", body=PAGE)
route("html-none", body=PAGE)
route("html-gzip", ct="text/html", body=gzip.compress(b"", mtime=0), headers={"Content-Encoding": "gzip"})
route("page.html", ct="text/html", body=HTML)
route("vrml-named.html", ct="model/vrml", body=OK)
redirect("r-html", "/h/html")
redirect("r-html-chain", "/h/r-html")
redirect("r-page-html", "/h/page.html")
redirect("r-html-cross", "{origin2}/h/html-cors")
route("html-cors", ct="text/html", body=HTML, headers={"Access-Control-Allow-Origin": "*"})

with open(os.path.join(out_dir, "routes.json"), "w") as f:
    json.dump(routes, f, indent=0, sort_keys=True)
    f.write("\n")

# ------------------------------------------------------------------------------------------------------------------ the worlds
U = lambda *names: ["/h/" + n for n in names]
cases = {
    "fh-redirect": ("full", [
        U("ok"), U("r301"), U("r302"), U("r303"), U("r307"), U("r308"), U("r-rel"), U("r-rel-up"), U("r-abs"), U("r-protocol-relative"),
        U("r-query"), U("r-fragment-only"), U("r-chain"), U("loop-a"), U("self"), U("chain20/0"), U("chain21/0"),
        U("r-nolocation"), U("r-empty-location"), U("r300"), U("r304"), U("r-to-404"), U("r-to-500"), U("r-to-html"),
        U("r-file"), U("r-data"), U("r-ftp"), U("r-javascript"), U("r-ecmascript"), U("r-blob"), U("r-about"), U("r-ws"),
        U("r-creds"), U("r-creds-user"), U("r-badurl"), U("r-refused"), U("r-cross-cors"),
        ["/h/r302#Frag"], ["/h/ok#Frag"], ["/h/ok?a=b"], ["/h/r302?x=1"], ["/h/r-query?y=2#G"],
        U("r-file", "ok"), U("r-to-404", "ok"), U("loop-a", "ok"), U("chain21/0", "r302"), U("r-creds", "r-data", "r302"),
    ]),
    "fh-status": ("full", [
        U("s400"), U("s403"), U("s404"), U("s410"), U("s500"), U("s502"), U("s503"), U("s204"), U("s201"), U("s200-empty"), U("s-html-404"),
        U("ct-model-vrml"), U("ct-x3d-vrml"), U("ct-x3d-xml"), U("ct-octet"), U("ct-plain"), U("ct-html"), U("ct-xhtml"), U("ct-charset"),
        U("ct-garbage"), U("ct-none"), U("hdr-custom"),
        U("enc-gzip"), U("enc-gzip-twice"), U("enc-file-gzip"), U("enc-bad"), U("wire-truncated"), U("wire-hangup"),
        U("missing"), U("s404", "s500", "ok"), U("wire-hangup", "ok"), U("enc-bad", "ok"),
    ]),
    "fh-diff": ("compact", [U("r-cross-nocors"), U("s418"), U("s-reason"), U("s-reason-empty")]),
    "fh-foreign": ("foreign", [
        [U("html"), []], [U("html-xhtml"), []], [U("html-charset"), []], [U("html-upper"), []], [U("html-space"), []], [U("html-plain"), []],
        [U("html-none"), []], [U("html-gzip"), []], [U("s-html-404"), []], [U("page.html"), []], [U("vrml-named.html"), []],
        [U("r-html"), []], [U("r-html-chain"), []], [U("r-page-html"), []], [U("r-html-cross"), []], [U("r-to-404"), []],
        [U("html"), ["target=_blank"]], [U("html"), ["target=_self"]], [U("html"), ["target="]], [U("html"), ["a=b", "target=_top"]],
        [U("html-plain"), ["target=_blank"]], [U("missing"), ["target=_blank"]], [U("r-html"), ["target=_parent"]],
        [U("missing", "html"), []], [U("html", "ok"), []], [U("html-plain", "html"), []], [U("s404", "html-xhtml"), []],
        [["/h/html#Frag"], []], [["/h/html?q=1"], []], [["/h/r-html?q=1#Frag"], []],
    ]),
}

PRE = """#X3D V3.3 utf8
PROFILE Immersive
COMPONENT Scripting:1
# Synthetic REA oracle world (generated by oracle/tools/gen_file_http.py): %(title)s.
# %(what)s
%(clock)sDEF OUT Script {
  inputOutput MFString out [ ]
  %(cycle)surl "ecmascript:
    var CASES = %(cases)s;
    var res = [];
    function strip (s) { s = String (s); var o = '', k, e; while ((k = s .indexOf ('http://')) >= 0 && (e = s .indexOf ('/', k + 7)) >= 0) { o += s .substring (0, k); s = s .substring (e + 1); } return o + s; }
    function put (i, v) { res [i] = i + ': ' + v; var o = []; for (var k = 0; k < CASES .length; ++ k) o .push (k in res ? res [k] : k + ': ...'); out = new MFString (... o); }
    function title (t) { var q = String .fromCharCode (34), m = t .indexOf ('title ' + q); return m < 0 ? 'no title' : t .substring (m + 7, t .indexOf (q, m + 7)); }
    function run (i) {
      if (i >= CASES .length) return;
      var p;
%(call)s
    }
    %(start)s
  "
}
%(route)s"""
CALL = {
    "full": """      try { p = Browser .createX3DFromURL (new MFString (... CASES [i])); } catch (e) { put (i, 'THROW ' + strip (e .message)); run (i + 1); return; }
      p .then (function (sc) {
        var head = [sc .encoding, sc .specificationVersion, sc .profile && sc .profile .name, sc .rootNodes .length, strip (sc .worldURL)] .join ('/');
        var text; try { text = sc .toVRMLString (); } catch (e) { text = 'PRINT ERR ' + e .message; }
        put (i, 'RES ' + head + String .fromCharCode (10) + text);
        run (i + 1);
      }, function (e) { put (i, 'REJ ' + strip (e && e .message)); run (i + 1); });""",
    "compact": """      try { p = Browser .createX3DFromURL (new MFString (... CASES [i])); } catch (e) { put (i, 'THROW ' + strip (e .message)); run (i + 1); return; }
      p .then (function (sc) { put (i, 'RES ' + strip (sc .worldURL) + ' title ' + title (sc .toVRMLString ())); run (i + 1); },
               function (e) { put (i, 'REJ ' + strip (e && e .message)); run (i + 1); });""",
    "foreign": """      try { p = Browser .loadURL (new MFString (... CASES [i] [0]), new MFString (... CASES [i] [1])); } catch (e) { put (i, 'THROW ' + strip (e .message)); idle = true; return; }
      p .then (function () { put (i, 'RES'); idle = true; }, function (e) { put (i, 'REJ ' + strip (e && e .message)); idle = true; });""",
}
WHAT = {
    "full": "Each URL list goes through Browser.createX3DFromURL, one after the other; `out` lists the scene head and printed scene or the rejection of each.",
    "compact": "Each URL list goes through Browser.createX3DFromURL; `out` lists the world URL and the title of the scene, or the rejection.",
    "foreign": "Each URL list goes through Browser.loadURL with a parameter list; a hand-off to `foreign` resolves, and the recorder lists it as `foreign`.",
}

# Browser.loadURL replaces what the page's own loadURL is still loading ("Loading of X3D file aborted."), so a world that calls it starts at
# its fourth TimeSensor cycle, after the page has loaded the world (as oracle/tools/gen_rea_b09_loadurl.py does).
CLOCK = "DEF CLOCK TimeSensor { loop TRUE cycleInterval 0.016666666666666666 }\n"
# One case per clock tick: a `location = href` that follows another one at once replaces it before the request is made, so the recorder
# would lose the first hand-off. The clock ticks every frame; the first three ticks belong to the page's own load.
START = {"foreign": "var cycles = 0, next = 0, idle = true; function cycle (t) { if (++ cycles > 3 && idle && next < CASES .length) { idle = false; run (next ++); } }"}

for name, (mode, lists) in cases.items():
    js = json.dumps(lists, ensure_ascii=True)
    # The case array sits inside a VRML string: \\ and \" are the only escapes the lexer reads.
    cases_text = js.replace("\\", "\\\\").replace('"', '\\"')
    timed = mode == "foreign"
    text = PRE % {"title": name, "what": WHAT[mode], "cases": cases_text, "call": CALL[mode], "clock": CLOCK if timed else "",
                  "cycle": "inputOnly SFTime cycle\n  " if timed else "", "start": START.get(mode, "function initialize () { run (0); }"),
                  "route": "ROUTE CLOCK.cycleTime TO OUT.cycle\n" if timed else ""}
    with open(os.path.join(out_dir, name + ".wrl"), "w") as f:
        f.write(text)
    print(name, len(lists))

script = {"size": [512, 512], "t0": 946684800, "dt": 1 / 60, "pinFrameRate": True, "settle": True, "frames": 60,
          "capture": [], "dump": [1, 60], "events": [], "httpRoutes": "oracle/synthetic-rea-file-http/routes.json"}
with open(os.path.join(out_dir, "fh.json"), "w") as f:
    json.dump(script, f)
script.update({"foreign": True, "frames": 100, "dump": [1, 100]})
with open(os.path.join(out_dir, "fh-foreign.json"), "w") as f:
    json.dump(script, f)
