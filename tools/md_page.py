#!/usr/bin/env python3
"""Write an HTML page of the package from one of its Markdown documents, in the package's design (site/theme.css).

md_page.py SOURCE.md OUT.html --root REL --current NAV --eyebrow TEXT --tag TEXT --source-link REL

Standard library only, so the package build needs nothing new. It converts the Markdown subset the package documents use:
ATX headings, paragraphs, "-" and "*" lists (nested by indentation), numbered lists, fenced code blocks, pipe tables,
block quotes, and inline code, bold, italic and links. Raw HTML in the text is escaped, not passed through.
The Markdown file stays in the package; the page links it.
"""
import argparse, html, re

ap = argparse.ArgumentParser()
ap.add_argument("source")
ap.add_argument("out")
ap.add_argument("--root", required=True, help="relative URL of the package root from the page, e.g. '../' or ''")
ap.add_argument("--current", required=True, help="the navigation entry of this page")
ap.add_argument("--eyebrow", required=True)
ap.add_argument("--tag", required=True, help="the release tag of the header: the channel and version, e.g. 'Canary X.Y.Z'")
ap.add_argument("--source-link", required=True, help="relative URL of the Markdown file from the page")
args = ap.parse_args()

text = open(args.source, encoding="utf-8").read()
slugs = {}


def slug(s):
    base = re.sub(r"[^a-z0-9]+", "-", re.sub(r"<[^>]+>", "", s).lower()).strip("-") or "section"
    n = slugs.get(base, 0)
    slugs[base] = n + 1
    return base if n == 0 else f"{base}-{n}"


def inline(s):
    out, pos = [], 0
    # Code spans first: their content is literal.
    for m in re.finditer(r"(`+)(.+?)\1", s):
        out.append(rich(s[pos:m.start()]))
        out.append(f"<code>{html.escape(m.group(2).strip())}</code>")
        pos = m.end()
    out.append(rich(s[pos:]))
    return "".join(out)


def rich(s):
    # The text is escaped once here; a link target is part of it, so only its quotes are escaped again (a second
    # html.escape would turn "&amp;" into "&amp;amp;").
    s = html.escape(s, quote=False)
    s = re.sub(r"\[([^\]]+)\]\(([^)\s]+)\)", lambda m: f'<a href="{m.group(2).replace(chr(34), "&quot;")}">{m.group(1)}</a>', s)
    s = re.sub(r"\*\*(.+?)\*\*", r"<b>\1</b>", s)
    s = re.sub(r"(?<![\w*])\*(?!\s)(.+?)(?<!\s)\*(?![\w*])", r"<em>\1</em>", s)
    return s


def table(rows):
    cells = [[c.strip() for c in r.strip().strip("|").split("|")] for r in rows]
    head, body = cells[0], [r for r in cells[2:]]
    h = "".join(f"<th>{inline(c)}</th>" for c in head)
    b = "".join("<tr>" + "".join(f"<td>{inline(c)}</td>" for c in r) + "</tr>" for r in body)
    return f'<div class="table-wrap"><table><thead><tr>{h}</tr></thead><tbody>{b}</tbody></table></div>'


def lists(items):
    """items: (indent, ordered, text). Nest by indentation."""
    out, stack = [], []
    for indent, ordered, body in items:
        while stack and indent < stack[-1][0]:
            out.append(f"</li></{stack.pop()[1]}>")
        if not stack or indent > stack[-1][0]:
            tag = "ol" if ordered else "ul"
            stack.append((indent, tag))
            out.append(f"<{tag}><li>{inline(body)}")
        else:
            out.append(f"</li><li>{inline(body)}")
    while stack:
        out.append(f"</li></{stack.pop()[1]}>")
    return "".join(out)


lines = text.split("\n")
blocks, toc, title = [], [], None
i = 0
item = re.compile(r"^(\s*)([-*]|\d+\.)\s+(.*)$")
while i < len(lines):
    line = lines[i]
    if not line.strip():
        i += 1
    elif line.startswith("```"):
        j = i + 1
        while j < len(lines) and not lines[j].startswith("```"):
            j += 1
        blocks.append(f"<pre><code>{html.escape(chr(10).join(lines[i + 1:j]))}</code></pre>")
        i = j + 1
    elif m := re.match(r"^(#{1,4})\s+(.*)$", line):
        level, body = len(m.group(1)), inline(m.group(2).strip())
        if level == 1 and title is None:
            title = m.group(2).strip()
        ident = slug(body)
        if level == 2:
            toc.append((ident, body))
        blocks.append(f'<h{level} id="{ident}">{body}</h{level}>')
        i += 1
    elif line.lstrip().startswith("|"):
        j = i
        while j < len(lines) and lines[j].lstrip().startswith("|"):
            j += 1
        blocks.append(table(lines[i:j]))
        i = j
    elif line.startswith(">"):
        j = i
        while j < len(lines) and lines[j].startswith(">"):
            j += 1
        blocks.append("<blockquote><p>" + inline(" ".join(l[1:].strip() for l in lines[i:j])) + "</p></blockquote>")
        i = j
    elif item.match(line):
        items = []
        while i < len(lines) and lines[i].strip():
            m = item.match(lines[i])
            if m:
                items.append([len(m.group(1)), m.group(2)[0].isdigit(), m.group(3)])
            else:
                items[-1][2] += " " + lines[i].strip()  # a continuation line
            i += 1
        blocks.append(lists(items))
    elif re.match(r"^-{3,}\s*$", line):
        blocks.append("<hr>")
        i += 1
    else:
        j = i
        while j < len(lines) and lines[j].strip() and not re.match(r"^(#{1,4}\s|```|\s*\||>|\s*([-*]|\d+\.)\s)", lines[j]):
            j += 1
        blocks.append("<p>" + inline(" ".join(l.strip() for l in lines[i:j])) + "</p>")
        i = j

r = args.root
nav = [(r or "./", "Overview"), (f"{r}preview/", "Live Preview"), (f"{r}example/", "Examples"),
       (f"{r}docs/api.html", "API"), (f"{r}compatibility.html", "Compatibility")]
CURRENT = ' aria-current="page"'
nav_html = "\n".join(f'      <a href="{h}"{CURRENT if t == args.current else ""}>{t}</a>' for h, t in nav)
toc_html = "\n".join(f'        <li><a href="#{a}">{t}</a></li>' for a, t in toc)
title_text = html.escape(title or args.current)
heading = inline(title or args.current)

page = f'''<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta http-equiv="Content-Security-Policy" content="default-src 'self'; script-src 'none'; style-src 'self'; img-src 'self' data:; font-src 'self'; object-src 'none'; base-uri 'self'">
<meta name="theme-color" content="#0b0f17">
<title>{title_text} · X_ITE_R</title>
<link rel="stylesheet" href="{r}site/theme.css">
</head>
<body>
<a class="skip" href="#main">Skip to the content</a>

<header class="top">
  <div class="top-in">
    <a class="brand" href="{r or './'}" aria-label="X_ITE_R start page">
      <img src="{r}site/exciter-wide.png" width="131" height="42" alt="eXciteR">
      <span class="brand-name">X_ITE_R</span>
      <span class="tag">{html.escape(args.tag)}</span>
    </a>
    <nav aria-label="Main">
{nav_html}
    </nav>
    <a class="repo" href="https://github.com/Ascendance3D/X_ITE_R"><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M2 3h12v10H2z M4 6l2 2-2 2 M8 10h4" fill="none" stroke="currentColor" stroke-width="1.3"/></svg>Ascendance3D/X_ITE_R</a>
  </div>
</header>

<div class="page-head">
  <div class="wrap">
    <div class="solo">
      <p class="eyebrow">{html.escape(args.eyebrow)}</p>
      <h1>{heading}</h1>
    </div>
  </div>
</div>

<div class="wrap doc">
  <nav class="toc" aria-label="On this page">
    <p class="label">On this page</p>
    <ol>
{toc_html}
    </ol>
    <p><a class="tool" href="{html.escape(args.source_link)}">Markdown source</a></p>
  </nav>
  <main id="main" class="prose">
{chr(10).join(blocks)}
  </main>
</div>

<footer class="foot">
  <div class="wrap foot-base"><span>X_ITE_R · MIT license · Reference: X_ITE 16.5.0</span><span><a href="{r}LICENSE">License</a> · <a href="{r}THIRD_PARTY_NOTICES.md">Third-party notices</a></span></div>
</footer>
</body>
</html>
'''
open(args.out, "w", encoding="utf-8").write(page)
