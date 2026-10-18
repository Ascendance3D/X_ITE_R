#!/usr/bin/env python3
"""REA batch 08 SVG fixtures for oracle/synthetic-rea-svg.

  python3 -I oracle/tools/gen_rea_b08_svg.py oracle/synthetic-rea-svg

Every file is synthetic and written here: nothing is copied from a historical world. The files
cover each element and attribute SVGParser.js reads, the shapes it does not know (so the
absence of output is checked), curve subdivision, libtess triangulation (fill rules, holes,
self-intersections, seeded random polygons), gradients, units, viewBox rules, invalid input.
The X_ITE baseline is made by `node oracle/x_ite-svg.mjs`; nothing here is a result.
"""
import gzip, math, os, random, sys

OUT = sys.argv[1] if len(sys.argv) > 1 else "oracle/synthetic-rea-svg"
os.makedirs(OUT, exist_ok=True)
NS = 'xmlns="http://www.w3.org/2000/svg"'
XL = 'xmlns:xlink="http://www.w3.org/1999/xlink"'
files = {}


def svg(name, body, w=200, h=200, root="", view=True, ns=True):
    attrs = []
    if ns:
        attrs += [NS, XL]
    if w is not None:
        attrs.append(f'width="{w}"')
    if h is not None:
        attrs.append(f'height="{h}"')
    if view and w is not None and h is not None and "viewBox" not in root:
        attrs.append(f'viewBox="0 0 {w} {h}"')
    if root:
        attrs.append(root)
    files[name] = f'<svg {" ".join(attrs)}>\n{body}\n</svg>\n'


def raw(name, text):
    files[name] = text


# ---- basic shapes ------------------------------------------------------------------------------
svg("shape-rect", '<rect x="10" y="20" width="80" height="50" fill="#f80"/>')
svg("shape-rect-stroke", '<rect x="10" y="20" width="80" height="50" fill="none" stroke="#08f" stroke-width="6"/>')
svg("shape-rect-both", '<rect x="10" y="20" width="80" height="50" fill="red" stroke="blue" stroke-width="2.5" fill-opacity="0.5" stroke-opacity="0.25"/>')
svg("shape-rect-rounded", '<rect x="10" y="20" width="120" height="80" rx="15" ry="25" fill="green" stroke="black"/>')
svg("shape-rect-rx-only", '<rect x="10" y="20" width="120" height="80" rx="30" fill="green"/>')
svg("shape-rect-ry-only", '<rect x="10" y="20" width="120" height="80" ry="30" fill="green" stroke="red" stroke-width="3"/>')
svg("shape-rect-rx-big", '<rect x="10" y="20" width="60" height="40" rx="500" fill="purple"/>')
svg("shape-rect-zero", '<rect x="10" y="20" width="0" height="0" fill="purple" stroke="red"/>')
svg("shape-rect-percent", '<rect x="10%" y="5%" width="50%" height="25%" fill="teal"/>', w=400, h=200)
svg("shape-circle", '<circle cx="100" cy="100" r="60" fill="navy"/>')
svg("shape-circle-stroke", '<circle cx="100" cy="100" r="60" fill="none" stroke="gold" stroke-width="10"/>')
svg("shape-circle-percent-r", '<circle cx="50%" cy="50%" r="25%" fill="navy" stroke="white"/>', w=300, h=200)
svg("shape-ellipse", '<ellipse cx="100" cy="100" rx="80" ry="40" fill="maroon" stroke="black" stroke-width="2"/>')
svg("shape-ellipse-tall", '<ellipse cx="100" cy="100" rx="20" ry="70" fill="maroon"/>')
svg("shape-ellipse-zero", '<ellipse cx="100" cy="100" rx="0" ry="70" fill="maroon" stroke="red"/>')
svg("shape-polyline", '<polyline points="10,10 60,80 110,20 160,90 190,30" fill="none" stroke="red" stroke-width="4"/>')
svg("shape-polyline-fill", '<polyline points="10,10 60,80 110,20 160,90 190,30" fill="lime" stroke="black"/>')
svg("shape-polyline-two", '<polyline points="10,10 60,80" stroke="red"/>')
svg("shape-polygon", '<polygon points="100,10 40,198 190,78 10,78 160,198" fill="lime" stroke="purple" stroke-width="3"/>')
svg("shape-polygon-evenodd", '<polygon points="100,10 40,198 190,78 10,78 160,198" fill="lime" fill-rule="evenodd"/>')
svg("shape-polygon-spaces", '<polygon points=" 10 10  100 10 100 100 " fill="orange"/>')
svg("shape-polygon-odd", '<polygon points="10,10 100,10 100,100 10,100 50" fill="orange"/>')
svg("shape-polygon-dup", '<polygon points="10,10 10,10 100,10 100,10 100,100 10,100 10,100" fill="orange" stroke="black"/>')
svg("shape-line-unknown", '<line x1="10" y1="10" x2="150" y2="150" stroke="red" stroke-width="4"/><rect width="20" height="20"/>')
svg("shape-text-unknown", '<text x="10" y="40">hello</text><rect width="20" height="20" fill="blue"/>')
svg("shape-unknown-elements", '<defs><rect width="5" height="5"/></defs><title>t</title><desc>d</desc><foo><rect width="9" height="9"/></foo><rect width="20" height="20" fill="blue"/>')
svg("shape-prefixed", '<svg:rect x="1" y="1" width="30" height="30"/><rect width="10" height="10" fill="red"/>', ns=False, root='xmlns:svg="http://www.w3.org/2000/svg" xmlns="http://www.w3.org/2000/svg"')

# ---- paths ---------------------------------------------------------------------------------------
svg("path-lines", '<path d="M 10 10 L 100 10 L 100 100 L 10 100 Z" fill="red"/>')
svg("path-relative", '<path d="m 10 10 l 90 0 l 0 90 l -90 0 z" fill="red" stroke="black"/>')
svg("path-hv", '<path d="M10 10 H 100 V 100 h -50 v 50 H 10 Z" fill="red" stroke="black"/>')
svg("path-implicit-lineto", '<path d="M10 10 100 10 100 100 10 100 z" fill="red"/>')
svg("path-commas", '<path d="M10,10,100,10,100,100,10,100z" fill="red"/>')
svg("path-compact-numbers", '<path d="M10-10L100.5.5L100,100.25-5e1,100z" fill="red"/>')
svg("path-quadratic", '<path d="M 10 100 Q 100 10 190 100 T 100 190" fill="none" stroke="black" stroke-width="2"/><path d="m 10 100 q 90 -90 180 0 t -90 90" fill="blue"/>')
svg("path-quadratic-t-alone", '<path d="M 10 100 T 100 190 T 190 100" fill="none" stroke="black"/>')
svg("path-cubic", '<path d="M 10 100 C 10 10 190 10 190 100 S 100 190 10 100" fill="orange" stroke="black"/>')
svg("path-cubic-relative", '<path d="m 10 100 c 0 -90 180 -90 180 0 s -90 90 -180 0 z" fill="orange" stroke="black"/>')
svg("path-smooth-after-line", '<path d="M 10 100 L 50 100 S 100 10 190 100 Z" fill="orange"/>')
svg("path-arc-small", '<path d="M 50 100 A 50 50 0 0 0 150 100 Z" fill="teal"/>')
svg("path-arc-large", '<path d="M 50 100 A 50 50 0 1 1 150 100 Z" fill="teal"/>')
svg("path-arc-sweep", '<path d="M 50 100 A 50 50 0 0 1 150 100 L 100 190 Z" fill="teal"/><path d="M 50 20 A 50 50 0 1 0 150 20 Z" fill="red"/>')
svg("path-arc-ellipse-rotated", '<path d="M 20 100 A 80 30 30 0 1 180 100 Z" fill="teal" stroke="black"/>')
svg("path-arc-radii-scaled", '<path d="M 20 100 A 10 10 0 0 1 180 100 Z" fill="teal"/>')
svg("path-arc-zero-radius", '<path d="M 20 100 A 0 10 0 0 1 180 100 L 100 150 Z" fill="teal"/>')
svg("path-arc-same-point", '<path d="M 20 100 A 30 30 0 0 1 20 100 L 180 100 L 100 20 Z" fill="teal"/>')
svg("path-arc-relative", '<path d="m 20 100 a 40 40 0 0 1 80 0 a 40 40 0 0 1 80 0 z" fill="teal"/>')
svg("path-arc-compact-flags", '<path d="M20 100a40 40 0 11 80 0 40 40 0 01 80 0z" fill="teal"/>')
svg("path-arc-rotation-large", '<path d="M 20 100 A 80 30 400 1 0 180 100 Z" fill="teal"/><path d="M 20 150 A 80 30 -90 0 0 180 150 Z" fill="red"/>')
svg("path-multi-subpath", '<path d="M 10 10 L 90 10 L 90 90 L 10 90 Z M 30 30 L 70 30 L 70 70 L 30 70 Z" fill="navy"/>')
svg("path-hole-evenodd", '<path d="M 10 10 L 190 10 L 190 190 L 10 190 Z M 50 50 L 150 50 L 150 150 L 50 150 Z" fill="navy" fill-rule="evenodd" stroke="white"/>')
svg("path-hole-nonzero-reversed", '<path d="M 10 10 L 190 10 L 190 190 L 10 190 Z M 50 50 L 50 150 L 150 150 L 150 50 Z" fill="navy"/>')
svg("path-hole-nonzero-same-direction", '<path d="M 10 10 L 190 10 L 190 190 L 10 190 Z M 50 50 L 150 50 L 150 150 L 50 150 Z" fill="navy"/>')
svg("path-open-subpaths", '<path d="M 10 10 L 90 10 L 90 90 M 110 110 L 190 110 L 190 190 L 110 190" fill="red" stroke="black"/>')
svg("path-moveto-short", '<path d="M 10 10 L 20 20 M 30 30 L 90 30 L 90 90 L 30 90 Z" fill="red"/>')
svg("path-star-nonzero", '<path d="M 100 10 L 40 190 L 190 70 L 10 70 L 160 190 Z" fill="gold" stroke="black"/>')
svg("path-star-evenodd", '<path d="M 100 10 L 40 190 L 190 70 L 10 70 L 160 190 Z" fill="gold" fill-rule="evenodd" stroke="black"/>')
svg("path-bowtie", '<path d="M 10 10 L 190 190 L 190 10 L 10 190 Z" fill="gold"/>')
svg("path-self-overlap", '<path d="M 10 10 L 150 10 L 150 150 L 10 150 Z M 60 60 L 190 60 L 190 190 L 60 190 Z" fill="gold" fill-rule="evenodd"/>')
svg("path-three-nested", '<path d="M 5 5 H 195 V 195 H 5 Z M 25 25 H 175 V 175 H 25 Z M 45 45 H 155 V 155 H 45 Z M 65 65 H 135 V 135 H 65 Z" fill="navy" fill-rule="evenodd"/><path d="M 5 5 H 195 V 195 H 5 Z M 25 25 H 175 V 175 H 25 Z M 45 45 H 155 V 155 H 45 Z M 65 65 H 135 V 135 H 65 Z" transform="translate(0 0)" fill="red"/>')
svg("path-nan-inf", '<path d="M 10 10 L +inf 10 L 100 100 Z" fill="red"/><rect width="10" height="10"/>')
svg("path-bad-command", '<path d="M 10 10 L 100 10 X 100 100 L 10 100 Z" fill="red"/>')
svg("path-empty", '<path d="" fill="red"/><path fill="red"/><rect width="10" height="10"/>')
svg("path-lowercase-after-z", '<path d="M 10 10 L 100 10 L 100 100 z l 20 20 l 40 0 l 0 40 z" fill="red"/>')
svg("path-collinear", '<path d="M 10 10 L 50 10 L 100 10 Z" fill="red" stroke="black"/>')

# ---- transforms ----------------------------------------------------------------------------------
for i, t in enumerate([
    "translate(30 40)", "translate(30)", "translate(30,40)", "translate(30 )",
    "rotate(30)", "rotate(45 50 50)", "rotate(-20, 100, 100)",
    "scale(2)", "scale(2 0.5)", "scale(-1 1)", "scale(1,2)", "scale(2 )",
    "skewX(20)", "skewY(-15)",
    "matrix(1 0 0 1 20 30)", "matrix(0.8,0.3,-0.3,0.8,40,10)", "matrix(1 0 0 1 20)",
    "translate(10 10) rotate(30) scale(1.5)", "translate(10,10),rotate(30),skewX(10)",
    "scale(2) translate(10 10)", "rotate(30) foo(1) translate(10 10)", "TRANSLATE(10 10)",
    "translate(10 10) rotate(45 1)", "matrix(1,0,0,1,0,0) translate(50 0)",
]):
    svg(f"transform-{i:02d}", f'<rect x="10" y="10" width="50" height="30" fill="blue" transform="{t}"/>')
svg("transform-group", '<g transform="translate(50 50) rotate(15)"><rect width="40" height="40" fill="red" transform="scale(1.5)"/><circle r="10" fill="blue"/></g>')
svg("transform-circle-ellipse", '<circle cx="50" cy="50" r="20" transform="rotate(30) scale(1 2)" fill="red" stroke="black"/><ellipse cx="100" cy="100" rx="40" ry="20" transform="skewX(30)" fill="blue" stroke="black" stroke-width="4"/>')
svg("transform-stroke-scaled", '<g transform="scale(3 1)"><rect x="5" y="20" width="30" height="40" fill="none" stroke="red" stroke-width="4"/></g><g transform="scale(0.1)"><rect x="5" y="20" width="300" height="400" fill="none" stroke="red" stroke-width="40"/></g>')
svg("transform-singular", '<rect width="50" height="50" fill="red" transform="scale(0 1)"/><rect width="50" height="50" fill="blue" transform="matrix(0 0 0 0 0 0)"/>')

# ---- style -----------------------------------------------------------------------------------------
svg("style-attribute", '<rect width="50" height="50" style="fill:#0f0;stroke:red;stroke-width:5"/>')
svg("style-precedence", '<rect width="50" height="50" fill="red" style="fill: blue; stroke : green ; stroke-width : 4"/>')
svg("style-inherit-group", '<g fill="green" stroke="red" stroke-width="3" opacity="0.5"><rect width="50" height="50"/><g opacity="0.5" fill-opacity="0.5"><circle cx="100" cy="100" r="30"/></g></g>')
svg("style-inherit-keywords", '<g fill="green"><rect width="50" height="50" fill="inherit"/><rect x="60" width="50" height="50" fill="unset"/><rect x="120" width="50" height="50" fill="default"/></g>')
svg("style-display-none", '<g display="none"><rect width="50" height="50" fill="red"/></g><rect x="60" width="50" height="50" display="none" fill="blue"/><g style="display:none"><circle r="5"/></g>')
svg("style-display-default", '<g display="none"><g display="default"><rect width="50" height="50"/></g></g><rect width="10" height="10" fill="red"/>')
svg("style-fill-none-transparent", '<rect width="50" height="50" fill="none" stroke="red"/><rect x="60" width="50" height="50" fill="transparent" stroke="blue"/>')
svg("style-stroke-transparent", '<rect width="50" height="50" stroke="transparent"/><rect x="60" width="50" height="50" stroke="none"/>')
svg("style-opacity-range", '<rect width="50" height="50" opacity="2"/><rect x="60" width="50" height="50" opacity="-1" fill-opacity="3" fill="red"/><rect y="60" width="50" height="50" opacity="0.3" fill-opacity="transparent"/>')
svg("style-stroke-width-forms", ''.join(f'<rect x="{10 + 40 * i}" width="30" height="30" fill="none" stroke="red" stroke-width="{v}"/>' for i, v in enumerate(["0", "0.5", "1", "2", "-3", "none", "default", "1e1", "2px", "3mm"])))
svg("style-colors-named", ''.join(f'<rect x="{10 * i}" width="9" height="9" fill="{c}"/>' for i, c in enumerate(["red", "Blue", "LIME", "rebeccapurple", "darkslategray", "transparent", "currentColor", "currentcolor"])))
svg("style-colors-hex", ''.join(f'<rect x="{10 * i}" width="9" height="9" fill="{c}"/>' for i, c in enumerate(["#f00", "#00ff00", "#0000ffcc", "#abcd", "#12", "#12345", "#zzz", "#123456789"])))
svg("style-colors-functions", ''.join(f'<rect x="{10 * i}" width="9" height="9" fill="{c}"/>' for i, c in enumerate(["rgb(255,0,0)", "rgb(0 128 255)", "rgba(10,20,30,0.5)", "rgb(100%,50%,0%)", "hsl(120,100%,50%)", "hsla(240,100%,25%,0.5)", "rgb(300,-5,2.6)", "rgb(1,2)", "foo(1,2,3)"])))
svg("style-colors-invalid-inherit", '<g fill="red"><rect width="40" height="40" fill="notacolor"/><rect x="50" width="40" height="40" fill="#12"/></g><g stroke="blue"><rect y="60" width="40" height="40" fill="none" stroke="nope" stroke-width="3"/></g>')
svg("style-fill-leading-space", '<rect width="40" height="40" fill=" red"/><rect x="50" width="40" height="40" style="fill: red ;"/>')
svg("style-rule-values", '<rect width="40" height="40" fill-rule="evenodd"/><rect x="50" width="40" height="40" fill-rule="bogus"/>')
svg("style-vector-effect", '<g transform="scale(4)"><rect x="2" y="2" width="20" height="20" fill="none" stroke="red" stroke-width="2" vector-effect="non-scaling-stroke"/></g>')
svg("style-linewidth-shared", '<rect width="30" height="30" fill="none" stroke="red" stroke-width="4"/><rect x="40" width="30" height="30" fill="none" stroke="blue" stroke-width="4"/><circle cx="100" cy="20" r="15" fill="none" stroke="green" stroke-width="4"/>')
svg("style-semicolons", '<rect width="40" height="40" style=";;fill:red;;stroke:blue;stroke-width:3;fill:url(http://x/y)"/>')
svg("style-stroke-on-all", '<g fill="none" stroke="black" stroke-width="3"><rect width="30" height="30"/><circle cx="60" cy="15" r="14"/><ellipse cx="110" cy="15" rx="20" ry="10"/><polyline points="10,50 40,80 70,50"/><polygon points="90,50 120,80 150,50"/><path d="M 10 100 Q 50 140 90 100"/></g>')

# ---- structure --------------------------------------------------------------------------------------
svg("struct-group-nested", '<g transform="translate(10 10)"><g transform="translate(20 20)"><g><rect width="10" height="10" fill="red"/></g></g></g>')
svg("struct-empty-groups", '<g><g></g></g><g transform="translate(10 10)"/><rect width="10" height="10" fill="red"/>')
svg("struct-ids", '<g id="Group A"><rect id="1abc" width="10" height="10"/><rect id="r" width="10" height="10" x="20" stroke="red"/><circle id="r" cx="50" cy="5" r="5"/></g><g id="ViewBox"/><path id="p" d="M 0 100 L 50 100 L 25 50 Z" stroke="blue"/>')
svg("struct-use", '<defs><rect id="box" width="30" height="30" fill="red" stroke="black"/></defs><use href="#box"/><use href="#box" x="50" y="20"/><use xlink:href="#box" x="100" y="40" fill="blue"/>')
svg("struct-use-group", '<g id="g1" fill="green"><rect width="20" height="20"/><circle cx="40" cy="10" r="10" fill="red"/></g><use href="#g1" x="10" y="50"/><use href="#g1" transform="rotate(20)" y="100"/>')
svg("struct-use-missing", '<use href="#nothing"/><use/><use href=""/><use href="http://example.org/x.svg#a"/><rect width="10" height="10"/>')
svg("struct-use-path-cache", '<path id="pp" d="M 10 10 L 60 10 L 35 50 Z" fill="red" stroke="black"/><use href="#pp" x="70"/><use href="#pp" y="60"/>')
svg("struct-use-circle-cache", '<circle id="cc" cx="20" cy="20" r="15" fill="red" stroke="black" stroke-width="3"/><use href="#cc" x="50"/><use href="#cc" y="50" fill="blue"/>')
svg("struct-use-image", '<image id="im" href="tex.png" width="30" height="20"/><use href="#im" x="50"/><use href="#im" y="40"/>')
svg("struct-use-nested", '<rect id="a" width="10" height="10" fill="red"/><g id="b"><use href="#a" x="20"/></g><use href="#b" y="30"/><use href="#b" y="60" transform="scale(2)"/>')
svg("struct-use-style", '<g fill="red"><use href="#t"/></g><rect id="t" width="10" height="10" x="20"/>')
svg("struct-use-hash-forms", '<rect id="sp" width="10" height="10"/><use href="#sp" x="20"/><use href="  #sp" x="40"/><use href="foo.svg#sp" x="60"/><use href="#sp#x" x="80"/><use href="#SP" x="100"/>')
svg("struct-switch", '<switch><rect width="30" height="30" fill="red"/><circle cx="60" cy="15" r="15" fill="blue"/></switch><switch transform="translate(0 50)"><rect width="30" height="30" fill="green"/></switch>')
svg("struct-anchor", '<a href="http://example.org/page" title="T" target="_blank"><rect width="30" height="30" fill="red"/></a><a xlink:href="other.svg" xlink:title="XT"><circle cx="60" cy="15" r="15"/></a><a><rect y="50" width="10" height="10"/></a>')
svg("struct-image", '<image href="a.png" x="10" y="20" width="60" height="40"/><image xlink:href="b.jpg" x="100" y="20" width="50" height="50" transform="rotate(10)"/><image width="10" height="10"/>')
svg("struct-image-percent", '<image href="a.png" x="10%" y="10%" width="50%" height="50%"/>')
svg("struct-hidden-image", '<g display="none"><image href="a.png" width="10" height="10"/></g><image href="c.png" width="10" height="10" display="none"/>')

# ---- root attributes --------------------------------------------------------------------------------
svg("root-default-size", '<rect width="50" height="50" fill="red"/>', w=None, h=None)
svg("root-width-only", '<rect width="50" height="50" fill="red"/>', w=None, h=None, root='width="400"')
svg("root-height-only", '<rect width="50" height="50" fill="red"/>', w=None, h=None, root='height="300"')
svg("root-viewbox", '<rect width="50" height="50" fill="red"/>', w=200, h=100, root='viewBox="0 0 50 25"')
svg("root-viewbox-offset", '<rect width="50" height="50" fill="red"/>', w=200, h=100, root='viewBox="-10 -20 100 50"')
svg("root-viewbox-wide", '<rect width="50" height="50" fill="red"/>', w=100, h=300, root='viewBox="0 0 400 100"')
svg("root-viewbox-tall", '<rect width="50" height="50" fill="red"/>', w=300, h=100, root='viewBox="0 0 100 400"')
svg("root-viewbox-only", '<rect width="50" height="50" fill="red"/>', w=None, h=None, root='viewBox="0 0 80 60"')
svg("root-viewbox-commas", '<rect width="50" height="50" fill="red"/>', w=200, h=100, root='viewBox="0,0,50,25"')
svg("root-viewbox-bad", '<rect width="50" height="50" fill="red"/>', w=200, h=100, root='viewBox="0 0 50"')
svg("root-viewbox-zero", '<rect width="50" height="50" fill="red"/>', w=200, h=100, root='viewBox="0 0 0 0"')
for name, wv, hv in [("units-mm", "100mm", "50mm"), ("units-cm", "10cm", "5cm"), ("units-in", "2in", "1in"), ("units-pt", "200pt", "100pt"), ("units-pc", "20pc", "10pc"),
                     ("units-em", "20em", "10em"), ("units-ex", "20ex", "10ex"), ("units-px", "200px", "100px"), ("units-percent", "50%", "25%"), ("units-bad", "abc", "xyz")]:
    svg(f"root-{name}", '<rect width="30" height="30" fill="red"/><circle cx="50%" cy="50%" r="10%" fill="blue"/>', w=None, h=None, root=f'width="{wv}" height="{hv}"')
svg("root-element-ignored", '<rect width="30" height="30" fill="red"/>', w=100, h=100, root='fill="blue" transform="translate(50 50)" display="none" style="fill:green"')
svg("root-no-namespace", '<rect width="30" height="30" fill="red"/>', ns=False)
svg("root-units-in-shapes", '<rect x="1em" y="1in" width="2cm" height="10mm" fill="red"/><circle cx="1pc" cy="2pt" r="3px" fill="blue"/><rect x="5ex" width="10" height="10"/>', w=300, h=300)

# ---- gradients --------------------------------------------------------------------------------------
RC = '<rect x="20" y="20" width="160" height="100" fill="url(#g)"/>'
svg("grad-linear-default", '<defs><linearGradient id="g"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs>' + RC)
svg("grad-linear-diagonal", '<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="red"/><stop offset="0.5" stop-color="#0f0"/><stop offset="1" stop-color="blue"/></linearGradient></defs>' + RC)
svg("grad-linear-percent", '<defs><linearGradient id="g" x1="10%" y1="0%" x2="90%" y2="100%"><stop offset="20%" stop-color="yellow"/><stop offset="80%" stop-color="purple"/></linearGradient></defs>' + RC)
svg("grad-linear-opacity", '<defs><linearGradient id="g"><stop offset="0" stop-color="red" stop-opacity="1"/><stop offset="1" stop-color="red" stop-opacity="0"/></linearGradient></defs>' + RC)
svg("grad-linear-userspace", '<defs><linearGradient id="g" gradientUnits="userSpaceOnUse" x1="20" y1="20" x2="180" y2="20"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs>' + RC)
svg("grad-linear-transform", '<defs><linearGradient id="g" gradientTransform="rotate(45 0.5 0.5)"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs>' + RC)
svg("grad-linear-transform-scale", '<defs><linearGradient id="g" gradientTransform="scale(0.5 1) translate(0.2 0)"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs>' + RC)
svg("grad-linear-repeat", '<defs><linearGradient id="g" x2="0.25" spreadMethod="repeat"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs>' + RC)
svg("grad-linear-reflect", '<defs><linearGradient id="g" x2="0.25" spreadMethod="reflect"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs>' + RC)
svg("grad-linear-pad-short", '<defs><linearGradient id="g" x1="0.3" x2="0.6"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs>' + RC)
svg("grad-linear-stops-unordered", '<defs><linearGradient id="g"><stop offset="0.8" stop-color="red"/><stop offset="0.2" stop-color="blue"/><stop offset="0.5" stop-color="lime"/></linearGradient></defs>' + RC)
svg("grad-linear-hard-stop", '<defs><linearGradient id="g"><stop offset="0" stop-color="red"/><stop offset="0.5" stop-color="red"/><stop offset="0.5" stop-color="blue"/><stop offset="1" stop-color="blue"/></linearGradient></defs>' + RC)
svg("grad-linear-one-stop", '<defs><linearGradient id="g"><stop offset="0.5" stop-color="red"/></linearGradient></defs>' + RC)
svg("grad-linear-no-stops", '<defs><linearGradient id="g"/></defs>' + RC)
svg("grad-linear-degenerate", '<defs><linearGradient id="g" x1="0.5" x2="0.5"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs>' + RC)
svg("grad-linear-stop-style", '<defs><linearGradient id="g"><stop offset="0" style="stop-color:red;stop-opacity:0.5"/><stop offset="1" style="stop-color:#00f"/></linearGradient></defs>' + RC)
svg("grad-linear-stop-default-black", '<defs><linearGradient id="g"><stop offset="0"/><stop offset="1" stop-color="white"/></linearGradient></defs>' + RC)
svg("grad-linear-stop-out-of-range", '<defs><linearGradient id="g"><stop offset="-1" stop-color="red"/><stop offset="2" stop-color="lime"/><stop offset="0" stop-color="blue"/><stop offset="1" stop-color="yellow"/></linearGradient></defs>' + RC + '<rect x="20" y="130" width="50" height="50" fill="green"/>')
svg("grad-linear-stop-style-leak", '<defs><linearGradient id="g"><stop offset="-1" stop-color="red" stop-opacity="0.2"/><stop offset="1" stop-color="blue"/></linearGradient></defs>' + RC + '<g><rect x="20" y="130" width="50" height="50"/></g>')
svg("grad-linear-href", '<defs><linearGradient id="base"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient><linearGradient id="g" href="#base" x2="0.5"/></defs>' + RC)
svg("grad-linear-href-xlink", '<defs><linearGradient id="base" x1="0.2"><stop offset="0" stop-color="red"/></linearGradient><linearGradient id="g" xlink:href="#base"><stop offset="1" stop-color="blue"/></linearGradient></defs>' + RC)
svg("grad-linear-href-chain", '<defs><linearGradient id="a"><stop offset="0" stop-color="red"/></linearGradient><linearGradient id="b" href="#a"><stop offset="0.5" stop-color="lime"/></linearGradient><linearGradient id="g" href="#b"><stop offset="1" stop-color="blue"/></linearGradient></defs>' + RC)
svg("grad-linear-href-radial", '<defs><radialGradient id="r"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></radialGradient><linearGradient id="g" href="#r"/></defs>' + RC)
svg("grad-linear-on-circle", '<defs><linearGradient id="g"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs><circle cx="100" cy="100" r="80" fill="url(#g)"/><ellipse cx="100" cy="100" rx="40" ry="20" fill="url(&quot;#g&quot;)"/>')
svg("grad-linear-on-path", '<defs><linearGradient id="g"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs><path d="M 20 20 L 180 20 L 100 180 Z" fill="url(#g)" stroke="black"/>')
svg("grad-linear-on-star-evenodd", '<defs><linearGradient id="g"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs><path d="M 100 10 L 40 190 L 190 70 L 10 70 L 160 190 Z" fill="url(#g)" fill-rule="evenodd"/>')
svg("grad-linear-on-polygon-holes", '<defs><linearGradient id="g"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs><path d="M 10 10 L 190 10 L 190 190 L 10 190 Z M 50 50 L 150 50 L 150 150 L 50 150 Z" fill="url(#g)" fill-rule="evenodd"/>')
svg("grad-linear-rounded-rect", '<defs><linearGradient id="g"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs><rect x="20" y="20" width="160" height="100" rx="20" fill="url(#g)"/>')
svg("grad-shared-two-shapes", '<defs><linearGradient id="g"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs><rect x="10" y="10" width="80" height="80" fill="url(#g)"/><rect x="100" y="10" width="80" height="80" fill="url(#g)"/>')
svg("grad-stroke-url", '<defs><linearGradient id="g"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs><rect x="20" y="20" width="100" height="100" fill="none" stroke="url(#g)" stroke-width="5"/>')
svg("grad-missing-ref", '<rect x="20" y="20" width="100" height="100" fill="url(#nope)"/><rect x="30" y="30" width="10" height="10" fill="url(nohash)"/>')
svg("grad-pattern-unsupported", '<defs><pattern id="p" width="10" height="10"><rect width="5" height="5"/></pattern></defs><rect x="20" y="20" width="100" height="100" fill="url(#p)"/>')
svg("grad-ref-not-gradient", '<rect id="plain" width="10" height="10"/><rect x="20" y="20" width="100" height="100" fill="url(#plain)"/>')
svg("grad-radial-default", '<defs><radialGradient id="g"><stop offset="0" stop-color="white"/><stop offset="1" stop-color="black"/></radialGradient></defs>' + RC)
svg("grad-radial-focal", '<defs><radialGradient id="g" cx="0.5" cy="0.5" r="0.5" fx="0.3" fy="0.3"><stop offset="0" stop-color="white"/><stop offset="1" stop-color="navy"/></radialGradient></defs>' + RC)
svg("grad-radial-fr", '<defs><radialGradient id="g" fr="0.2"><stop offset="0" stop-color="yellow"/><stop offset="1" stop-color="red"/></radialGradient></defs>' + RC)
svg("grad-radial-userspace", '<defs><radialGradient id="g" gradientUnits="userSpaceOnUse" cx="100" cy="70" r="80"><stop offset="0" stop-color="white"/><stop offset="1" stop-color="green"/></radialGradient></defs>' + RC)
svg("grad-radial-repeat", '<defs><radialGradient id="g" r="0.2" spreadMethod="repeat"><stop offset="0" stop-color="white"/><stop offset="1" stop-color="black"/></radialGradient></defs>' + RC)
svg("grad-radial-reflect", '<defs><radialGradient id="g" r="0.2" spreadMethod="reflect"><stop offset="0" stop-color="white"/><stop offset="1" stop-color="black"/></radialGradient></defs>' + RC)
svg("grad-radial-pad", '<defs><radialGradient id="g" r="0.2" spreadMethod="pad"><stop offset="0" stop-color="white"/><stop offset="1" stop-color="black"/></radialGradient></defs>' + RC)
svg("grad-radial-transform", '<defs><radialGradient id="g" gradientTransform="scale(1 0.5) translate(0 0.5)"><stop offset="0" stop-color="white"/><stop offset="1" stop-color="black"/></radialGradient></defs>' + RC)
svg("grad-radial-opacity", '<defs><radialGradient id="g"><stop offset="0" stop-color="red" stop-opacity="0"/><stop offset="1" stop-color="red"/></radialGradient></defs>' + RC)
svg("grad-radial-href", '<defs><radialGradient id="base" r="0.3"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></radialGradient><radialGradient id="g" href="#base" cx="0.2"/></defs>' + RC)
svg("grad-radial-href-linear", '<defs><linearGradient id="base" x1="0.4"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient><radialGradient id="g" href="#base"/></defs>' + RC)
svg("grad-radial-r-percent", '<defs><radialGradient id="g" r="40%" cx="25%"><stop offset="0" stop-color="white"/><stop offset="1" stop-color="black"/></radialGradient></defs>' + RC)
svg("grad-radial-r-zero", '<defs><radialGradient id="g" r="0"><stop offset="0" stop-color="white"/><stop offset="1" stop-color="black"/></radialGradient></defs>' + RC)
svg("grad-radial-on-circle", '<defs><radialGradient id="g"><stop offset="0" stop-color="white"/><stop offset="1" stop-color="red"/></radialGradient></defs><circle cx="100" cy="100" r="80" fill="url(#g)"/>')
svg("grad-use-gradient-rect", '<defs><linearGradient id="g"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient><rect id="r" width="60" height="40" fill="url(#g)"/></defs><use href="#r" x="10" y="10"/><use href="#r" x="10" y="80"/>')
svg("grad-shapes-mix", '<defs><linearGradient id="g"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs><g fill="url(#g)"><rect width="50" height="50"/><circle cx="100" cy="25" r="25"/><polygon points="150,0 190,50 150,50"/></g>')
svg("grad-radial-negative-r", '<defs><radialGradient id="g" r="-0.5"><stop offset="0" stop-color="white"/><stop offset="1" stop-color="black"/></radialGradient></defs>' + RC)
svg("grad-radial-negative-fr", '<defs><radialGradient id="g" fr="-0.1"><stop offset="0" stop-color="white"/><stop offset="1" stop-color="black"/></radialGradient></defs>' + RC)
svg("grad-linear-infinite", '<defs><linearGradient id="g" x1="+inf"><stop offset="0" stop-color="white"/><stop offset="1" stop-color="black"/></linearGradient></defs>' + RC)
svg("grad-stop-offset-forms", '<defs><linearGradient id="g"><stop offset="abc" stop-color="red"/><stop offset=" 0.5" stop-color="lime"/><stop offset="75%" stop-color="blue"/><stop stop-color="yellow"/></linearGradient></defs>' + RC)
svg("grad-stop-color-forms", '<defs><linearGradient id="g"><stop offset="0" stop-color="currentColor"/><stop offset="0.3" stop-color="bogus"/><stop offset="0.6" stop-color="#abc"/><stop offset="1" stop-color="rgba(0,0,255,0.5)"/></linearGradient></defs>' + RC)
svg("grad-userspace-transform", '<defs><linearGradient id="g" gradientUnits="userSpaceOnUse" x1="0" x2="100" gradientTransform="rotate(30) translate(10 0)"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs>' + RC)
svg("grad-radial-userspace-transform", '<defs><radialGradient id="g" gradientUnits="userSpaceOnUse" cx="100" cy="70" r="60" fx="80" fy="60" gradientTransform="skewX(20)"><stop offset="0" stop-color="white"/><stop offset="0.6" stop-color="orange"/><stop offset="1" stop-color="black"/></radialGradient></defs>' + RC)
svg("grad-big-userspace", '<defs><linearGradient id="g" gradientUnits="userSpaceOnUse" x1="-1000" y1="0" x2="1000" y2="0"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs>' + RC)

# ---- tessellation stress ------------------------------------------------------------------------------
rng = random.Random(8080)
for i in range(24):
    n = rng.randint(5, 40)
    pts = " ".join(f"{rng.uniform(5, 195):.2f},{rng.uniform(5, 195):.2f}" for _ in range(n))
    rule = "evenodd" if i % 2 else "nonzero"
    svg(f"tess-random-{i:02d}", f'<polygon points="{pts}" fill="#369" fill-rule="{rule}" stroke="black"/>')
for i in range(6):
    d = ""
    for _ in range(rng.randint(2, 5)):
        n = rng.randint(3, 12)
        cx, cy, r = rng.uniform(40, 160), rng.uniform(40, 160), rng.uniform(15, 60)
        pts = []
        for k in range(n):
            a = 2 * math.pi * k / n + rng.uniform(-0.3, 0.3)
            rr = r * rng.uniform(0.4, 1.0)
            pts.append((cx + rr * math.cos(a), cy + rr * math.sin(a)))
        d += "M " + " L ".join(f"{x:.2f} {y:.2f}" for x, y in pts) + " Z "
    svg(f"tess-multi-{i:02d}", f'<path d="{d}" fill="#963" fill-rule="{"evenodd" if i % 2 else "nonzero"}"/>')
svg("tess-grid-overlap", ''.join('<path d="M %d 10 L %d 190 L %d 190 L %d 10 Z" fill="red" fill-rule="evenodd"/>' % (10 + 15 * i, 10 + 15 * i + 30, 40 + 15 * i + 30, 40 + 15 * i) for i in range(6)))
svg("tess-spiral", '<path d="M ' + " L ".join(f"{100 + (5 + 4 * k) * math.cos(k / 3.0):.3f} {100 + (5 + 4 * k) * math.sin(k / 3.0):.3f}" for k in range(60)) + ' Z" fill="orange" stroke="black"/>')
svg("tess-comb", '<path d="M 10 10 ' + ' '.join(f"L {20 + 20 * k} 100 L {30 + 20 * k} 10" for k in range(8)) + ' L 180 190 L 10 190 Z" fill="navy"/>')
svg("tess-cross-intersections", '<path d="M 0 100 L 200 100 L 200 110 L 0 110 Z M 100 0 L 110 0 L 110 200 L 100 200 Z M 20 20 L 180 180 L 190 170 L 30 10 Z" fill="red"/>')
svg("tess-curves-blob", '<path d="M 100 20 C 180 20 190 120 120 160 S 20 150 40 90 Q 50 30 100 20 Z" fill="teal" stroke="black" stroke-width="2"/>')
svg("tess-circle-arcs", '<path d="M 100 20 A 80 80 0 1 1 99.9 20 Z M 100 60 A 40 40 0 1 0 100.1 60 Z" fill="teal" fill-rule="evenodd"/>')
svg("tess-large-coords", '<path d="M -1e6 -1e6 L 1e6 -1e6 L 1e6 1e6 L -1e6 1e6 Z" fill="teal"/>')
svg("tess-tiny", '<path d="M 0 0 L 0.001 0 L 0.001 0.001 L 0 0.001 Z" fill="teal"/><path d="M 0 0 L 1e-10 0 L 1e-10 1e-10 Z" fill="red" stroke="black"/>')

# ---- invalid and odd input ------------------------------------------------------------------------------
raw("bad-not-xml.svg", "<svg width=100><rect/>")
raw("bad-unclosed.svg", '<svg xmlns="http://www.w3.org/2000/svg"><rect width="1" height="1">')
raw("bad-mismatched.svg", '<svg xmlns="http://www.w3.org/2000/svg"><g></rect></svg>')
raw("bad-root-not-svg.svg", '<html xmlns="http://www.w3.org/1999/xhtml"><svg xmlns="http://www.w3.org/2000/svg"/></html>')
raw("bad-root-prefixed.svg", '<s:svg xmlns:s="http://www.w3.org/2000/svg" width="10" height="10"><s:rect width="5" height="5"/></s:svg>')
raw("bad-root-uppercase.svg", '<SVG xmlns="http://www.w3.org/2000/svg" width="10" height="10"/>')
raw("bad-empty.svg", "")
raw("bad-text.svg", "hello world")
raw("bad-entity.svg", '<svg xmlns="http://www.w3.org/2000/svg"><rect width="&undefined;"/></svg>')
raw("bad-dup-attribute.svg", '<svg xmlns="http://www.w3.org/2000/svg"><rect width="1" width="2"/></svg>')
raw("ok-empty-svg.svg", '<svg xmlns="http://www.w3.org/2000/svg"/>')
raw("ok-comments-pi.svg", '<?xml version="1.0" encoding="UTF-8"?>\n<!-- c --><svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><!-- x --><?pi data?><rect width="40" height="40"/><![CDATA[ignored]]></svg>\n')
raw("ok-doctype.svg", '<?xml version="1.0"?>\n<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">\n<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><rect width="40" height="40"/></svg>\n')
raw("ok-bom.svg", '﻿<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><rect width="40" height="40"/></svg>')
raw("ok-leading-whitespace.svg", '\n\n   <svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><rect width="40" height="40"/></svg>')
raw("ok-attribute-entities.svg", '<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><g id="a&amp;b &#65;"><rect width="4&#48;" height="&#x32;0" fill="&#35;f00"/></g></svg>')
raw("ok-nbsp-numbers.svg", '<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><rect x="1&#160;" width="10" height="10"/><path d="M0 0&#10;L 10 0&#9;L 10 10&#13;Z"/></svg>')
svg("odd-negative-sizes", '<rect x="50" y="50" width="-30" height="-20" fill="red" stroke="blue"/><circle cx="20" cy="20" r="-5" fill="red" stroke="blue"/><ellipse cx="60" cy="60" rx="-5" ry="10" fill="red"/>')
svg("odd-exponent-numbers", '<rect x="1e1" y="1E1" width="2.5e1" height=".5e2" fill="red"/><rect x="+5" y="-0" width="0x10" height="1e" fill="blue"/>')
svg("odd-large-document", ''.join(f'<rect x="{(i % 20) * 10}" y="{(i // 20) * 10}" width="8" height="8" fill="hsl({i * 7 % 360},83%,47%)"/>' for i in range(400)))

# ---- compressed ----------------------------------------------------------------------------------------
GZ = svg  # name only

for name, text in list(files.items()):
    fname = name if name.endswith(".svg") else name + ".svg"
    with open(os.path.join(OUT, fname), "w", encoding="utf-8", newline="") as f:
        f.write(text)

with open(os.path.join(OUT, "ok-gzip.svg"), "wb") as f:
    f.write(gzip.compress(files["shape-rect"].encode(), mtime=0))

print(f"{len(files) + 1} files")
