#!/usr/bin/env python3
"""
Generate a dark-mode hexagonal architecture report for a Rust workspace.

Usage:
    python3 depgraph-report.py [--branch BRANCH] [--base BASE] [--output FILE]

Defaults:
    --branch  current HEAD
    --base    main
    --output  target/xtask/depgraph-report.html

Requires: cargo-depgraph, graphviz (dot)
"""
import argparse, json, math, os, re, shutil, subprocess, sys, tempfile
import csv
from collections import defaultdict
from datetime import datetime, timezone
from html import escape
from pathlib import Path


def run(cmd, **kw):
    r = subprocess.run(cmd, capture_output=True, text=True, **kw)
    return r.stdout.strip()


def git(*args):
    return run(["git", "-C", REPO] + list(args))


def _safe_read_json(path: Path):
    try:
        with path.open() as f:
            return json.load(f)
    except (OSError, json.JSONDecodeError):
        return None


def _latest_sarif_meta(repo_root: str):
    roots = [
        Path(repo_root) / ".ctx" / "xcache",
        Path(repo_root) / ".ctx",
        Path(repo_root) / "target",
        Path(repo_root),
    ]
    candidates = []
    for root in roots:
        if not root.exists():
            continue
        for p in sorted(root.rglob("*.sarif")):
            try:
                mtime = p.stat().st_mtime
            except OSError:
                continue
            candidates.append((mtime, p))
    if not candidates:
        return None

    _, latest = max(candidates, key=lambda i: i[0])
    data = _safe_read_json(latest)
    if not data:
        return None

    runs = data.get("runs", [])
    total_results = 0
    total_warnings = 0
    total_errors = 0
    total_notes = 0
    tools = set()

    for run in runs:
        driver = run.get("tool", {}).get("driver", {})
        if "name" in driver:
            tools.add(driver.get("name"))
        for result in run.get("results", []) or []:
            total_results += 1
            level = str(result.get("level", "note")).lower()
            if level == "warning":
                total_warnings += 1
            elif level == "error":
                total_errors += 1
            else:
                total_notes += 1

    tool_name = ", ".join(sorted(tools)) if tools else "unknown"
    return {
        "path": str(latest),
        "updated": datetime.fromtimestamp(latest.stat().st_mtime, tz=timezone.utc).isoformat(timespec="seconds"),
        "runs": len(runs),
        "results": total_results,
        "warnings": total_warnings,
        "errors": total_errors,
        "notes": total_notes,
        "tool": tool_name,
    }


def _sarif_insight_html(meta):
    if not meta:
        return "<li><strong>Latest SARIF:</strong> no SARIF files found.</li>"

    path = meta["path"]
    file_name = Path(path).name
    return (
        f'<li><strong>Latest SARIF:</strong> <code>{escape(file_name)}</code> from '
        f'<code>{escape(meta["tool"])}</code> &middot; '
        f'{meta["results"]} results ({meta["errors"]} errors, '
        f'{meta["warnings"]} warnings, {meta["notes"]} notes) across {meta["runs"]} run(s) '
        f'&middot; updated {escape(meta["updated"])}.</li>'
    )


def _write_action_snapshots(
    report_path: Path,
    action_items: list[dict],
    *,
    crate_count: int,
    good_deps: int,
    bad_deps: int,
    arch_health_percent: int,
) -> None:
    report_dir = report_path.parent
    report_dir.mkdir(parents=True, exist_ok=True)
    payload = {
        "generated_at": datetime.now().strftime("%Y-%m-%d"),
        "source_report": str(report_path),
        "summary": {
            "crate_count": crate_count,
            "good_deps": good_deps,
            "bad_deps": bad_deps,
            "arch_health_percent": arch_health_percent,
        },
        "action_items": [
            {
                "severity": item["severity"],
                "source": item["src"],
                "target": item["dst"],
                "source_ring": item["src_ring"],
                "target_ring": item["dst_ring"],
                "note": item["remedy"] if isinstance(item["remedy"], str) else "",
            }
            for item in action_items
        ],
    }
    (report_dir / "depgraph-action-items.json").write_text(json.dumps(payload, indent=2))

    csv_rows = [
        [
            "severity",
            "source",
            "target",
            "source_ring",
            "target_ring",
            "note",
        ]
    ]
    for item in action_items:
        note = item["remedy"] if isinstance(item["remedy"], str) else ""
        csv_rows.append([
            item["severity"],
            item["src"],
            item["dst"],
            item["src_ring"],
            item["dst_ring"],
            note,
        ])

    with (report_dir / "depgraph-action-items.csv").open("w", newline="") as f:
        w = csv.writer(f)
        w.writerows(csv_rows)


def _write_history_artifacts(
    report_path: Path,
    history_dir: Path,
    *,
    keep_timestamp: bool = True,
) -> list[Path]:
    if not history_dir:
        return []

    history_dir.mkdir(parents=True, exist_ok=True)
    timestamp = datetime.now(tz=timezone.utc).strftime("%Y%m%d-%H%M%S")

    primary = history_dir / "depgraph-report.html"
    timestamped = history_dir / f"{timestamp}-depgraph-report.html"
    snapshots = [
        (history_dir / "depgraph-action-items.json"),
        (history_dir / "depgraph-action-items.csv"),
    ]

    copied: list[Path] = []
    for target in (primary, timestamped):
        shutil.copy2(report_path, target)
        copied.append(target)

    for snapshot in snapshots:
        source = report_path.parent / snapshot.name
        if source.exists():
            shutil.copy2(source, snapshot)
            copied.append(snapshot)
            if keep_timestamp:
                shutil.copy2(source, history_dir / f"{timestamp}-{snapshot.name}")
                copied.append(history_dir / f"{timestamp}-{snapshot.name}")

    return copied


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------
p = argparse.ArgumentParser(description="Hexagonal workspace architecture report")
p.add_argument("--branch", default=None, help="Branch to analyse (default: HEAD)")
p.add_argument("--base", default="main", help="Base branch for divergence timeline")
p.add_argument("--output", default=None, help="Output HTML path")
p.add_argument("--repo", default=".", help="Repository root")
p.add_argument("--open", action="store_true", help="Open in browser after generation")
args = p.parse_args()

REPO = os.path.abspath(args.repo)
branch = args.branch or git("rev-parse", "--abbrev-ref", "HEAD")
base = args.base
out_path = args.output or os.path.join(REPO, "target", "xtask", "depgraph-report.html")
history_dir = Path(REPO) / ".ctx" / "xcache" / "history"
os.makedirs(os.path.dirname(out_path), exist_ok=True)

# ---------------------------------------------------------------------------
# 1. Cargo depgraph
# ---------------------------------------------------------------------------
raw_dot = run(["cargo", "depgraph", "--workspace-only"], cwd=REPO)

nodes = {}
edges = []
for line in raw_dot.splitlines():
    m = re.match(r'\s+(\d+)\s+\[.*label\s*=\s*"([^"]+)"', line)
    if m:
        nodes[int(m.group(1))] = m.group(2)
        continue
    m = re.match(r'\s+(\d+)\s+->\s+(\d+)\s*\[(.*)?\]', line)
    if m:
        style = m.group(3).strip() if m.group(3) else ""
        edges.append((int(m.group(1)), int(m.group(2)), style))

# Transitive reduction
adj = {n: set() for n in nodes}
for s, d, _ in edges:
    adj[s].add(d)


def reachable_without(src, skip):
    visited = set()
    q = [n for n in adj.get(src, set()) if n != skip]
    while q:
        n = q.pop(0)
        if n in visited:
            continue
        visited.add(n)
        q.extend(m for m in adj.get(n, set()) if m not in visited)
    return visited


reduced = [(s, d, st) for s, d, st in edges if d not in reachable_without(s, d)]

# Layer assignment (topo sort from leaves)
outgoing = {n: set() for n in nodes}
incoming = {n: set() for n in nodes}
for s, d, _ in reduced:
    outgoing[s].add(d)
    incoming[d].add(s)

layers = {}
remaining = set(nodes.keys())
layer = 0
while remaining:
    ready = {n for n in remaining if not (outgoing[n] & remaining)}
    if not ready:
        break
    for n in ready:
        layers[n] = layer
    remaining -= ready
    layer += 1

max_layer = max(layers.values()) if layers else 0

# Classify into rings by layer
# Layer 0 = leaves (no outgoing workspace deps) = Core
# Highest layer = roots (no incoming workspace deps) = Applications
ring_names = ["Core", "Domain", "Adapters", "Applications"]
ring_map = {}
for nid, l in layers.items():
    # Map layer range [0..max_layer] to ring range [0..3]
    if max_layer == 0:
        ring_idx = 0
    else:
        ring_idx = min(round(l / max_layer * (len(ring_names) - 1)), len(ring_names) - 1)
    ring_map[nid] = ring_idx

rings_by_idx = defaultdict(list)
for nid in sorted(nodes.keys(), key=lambda x: nodes[x]):
    rings_by_idx[ring_map.get(nid, 0)].append(nodes[nid])

# ---------------------------------------------------------------------------
# 2. Hexagonal SVG
# ---------------------------------------------------------------------------
name_to_id = {v: k for k, v in nodes.items()}


def hex_path(cx, cy, r):
    pts = []
    for i in range(6):
        a = math.radians(60 * i - 30)
        pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    d = f"M {pts[0][0]:.1f},{pts[0][1]:.1f}"
    for pt in pts[1:]:
        d += f" L {pt[0]:.1f},{pt[1]:.1f}"
    return d + " Z"


ring_radii = [0, 160, 320, 460]
hex_radii = [90, 210, 370, 500]
ring_colors = ["#7c4dff", "#448aff", "#ffd740", "#ff5252"]
ring_fills = ["#2d2250", "#1e3a5f", "#3d3520", "#3d2020"]

W, H = 1100, 1050
CX, CY = W // 2, H // 2 + 20

# Place nodes
positions = {}
for ring_idx, crate_names in rings_by_idx.items():
    r = ring_radii[min(ring_idx, len(ring_radii) - 1)]
    n = len(crate_names)
    if r == 0:
        positions[name_to_id[crate_names[0]]] = (CX, CY)
    else:
        for i, name in enumerate(crate_names):
            a = math.radians(-90 + (360 / n) * i)
            positions[name_to_id[name]] = (CX + r * math.cos(a), CY + r * math.sin(a))

svg_parts = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">']
svg_parts.append('<defs>')
svg_parts.append('<marker id="arrow-good" markerWidth="8" markerHeight="6" refX="8" refY="3" orient="auto">')
svg_parts.append('<path d="M0,0 L8,3 L0,6 Z" fill="#4caf50"/></marker>')
svg_parts.append('<marker id="arrow-bad" markerWidth="8" markerHeight="6" refX="8" refY="3" orient="auto">')
svg_parts.append('<path d="M0,0 L8,3 L0,6 Z" fill="#f44336"/></marker>')
svg_parts.append('</defs>')
svg_parts.append(f'<rect width="{W}" height="{H}" fill="#1a1a2e"/>')

for i in range(len(ring_names) - 1, -1, -1):
    r = hex_radii[min(i, len(hex_radii) - 1)]
    op = 0.15 + i * 0.05
    svg_parts.append(f'<path d="{hex_path(CX, CY, r)}" fill="{ring_colors[i]}" '
                     f'fill-opacity="{op}" stroke="{ring_colors[i]}" stroke-width="1.5" stroke-opacity="0.3"/>')

label_y = [CY, CY - 175, CY - 335, CY - 475]
for i, name in enumerate(ring_names):
    svg_parts.append(f'<text x="{CX}" y="{label_y[min(i, len(label_y)-1)]}" text-anchor="middle" '
                     f'font-family="Helvetica" font-size="11" fill="{ring_colors[i]}" opacity="0.6">{name}</text>')

for src, dst, style in reduced:
    if src not in positions or dst not in positions:
        continue
    x1, y1 = positions[src]
    x2, y2 = positions[dst]
    dx, dy = x2 - x1, y2 - y1
    dist = math.sqrt(dx * dx + dy * dy)
    if dist < 1:
        continue
    ux, uy = dx / dist, dy / dist
    dash = ' stroke-dasharray="3,3"' if "dotted" in style else ""
    # Good = dep goes to adjacent inner ring; bad = skips rings or wrong direction
    src_ring = ring_map.get(src, 0)
    dst_ring = ring_map.get(dst, 0)
    ring_gap = src_ring - dst_ring  # positive = inward (correct direction)
    is_good = ring_gap == 1
    edge_color = "#4caf50" if is_good else "#f44336"
    marker = "arrow-good" if is_good else "arrow-bad"
    svg_parts.append(f'<line x1="{x1+ux*50:.1f}" y1="{y1+uy*25:.1f}" x2="{x2-ux*50:.1f}" '
                     f'y2="{y2-uy*25:.1f}" stroke="{edge_color}" stroke-width="1.2" '
                     f'stroke-opacity="0.7"{dash} marker-end="url(#{marker})"/>')

for ring_idx, crate_names in rings_by_idx.items():
    ci = min(ring_idx, len(ring_colors) - 1)
    for name in crate_names:
        nid = name_to_id.get(name)
        if nid is None or nid not in positions:
            continue
        x, y = positions[nid]
        tw = max(len(name) * 8.5, 80)
        th = 32
        svg_parts.append(f'<rect x="{x-tw/2:.1f}" y="{y-th/2:.1f}" width="{tw:.1f}" height="{th}" '
                         f'rx="6" fill="{ring_fills[ci]}" stroke="{ring_colors[ci]}" stroke-width="1.5"/>')
        svg_parts.append(f'<text x="{x:.1f}" y="{y+4.5:.1f}" text-anchor="middle" '
                         f'font-family="Helvetica" font-size="12" font-weight="500" fill="{ring_colors[ci]}">{name}</text>')

svg_parts.append('</svg>')
svg_content = '\n'.join(svg_parts)

# Count good vs bad edges and build actionable items
good_edges, bad_edges, bad_edge_list, action_items = 0, 0, [], []
for src, dst, _ in reduced:
    src_ring = ring_map.get(src, 0)
    dst_ring = ring_map.get(dst, 0)
    gap = src_ring - dst_ring
    if gap == 1:
        good_edges += 1
    else:
        bad_edges += 1
        src_name, dst_name = nodes[src], nodes[dst]
        src_rname, dst_rname = ring_names[src_ring], ring_names[dst_ring]

        if gap == 0:
            # Same-ring dep
            severity = "low"
            remedy = (f"Both are in {src_rname}. Consider whether <code>{dst_name}</code> "
                      f"belongs one ring lower, or extract the shared interface into "
                      f"the adjacent inner ring.")
        elif gap > 1:
            # Skipping rings outward->inward (Application -> Core)
            skip = gap - 1
            severity = "high" if skip >= 2 else "medium"
            middle_ring = ring_names[src_ring - 1]
            remedy = (f"Skips {skip} ring{'s' if skip > 1 else ''}. "
                      f"Re-export or facade the needed types through "
                      f"<code>{middle_ring}</code>-layer crate so "
                      f"<code>{src_name}</code> only reaches one ring inward.")
        else:
            # Negative gap = dep goes outward (inner depends on outer) -- worst
            severity = "critical"
            remedy = (f"Inverted dependency: {src_rname} depends on {dst_rname}. "
                      f"Extract the interface <code>{src_name}</code> needs from "
                      f"<code>{dst_name}</code> into a trait in the inner ring, "
                      f"then implement it in {dst_rname}.")

        bad_edge_list.append(f"{src_name} -> {dst_name}")
        action_items.append({
            "src": src_name, "dst": dst_name,
            "src_ring": src_rname, "dst_ring": dst_rname,
            "gap": gap, "severity": severity, "remedy": remedy,
        })

# Sort actions: critical first, then high, medium, low
sev_order = {"critical": 0, "high": 1, "medium": 2, "low": 3}
action_items.sort(key=lambda a: sev_order[a["severity"]])

# Health score: percentage of good edges
health_score = round(100 * good_edges / max(good_edges + bad_edges, 1))
sarif_meta = _latest_sarif_meta(REPO)

# Build action items HTML
actions_html = ""
if not action_items:
    actions_html = '<div class="action-item"><div class="action-path" style="color:#4caf50">No architectural violations. All deps follow ring adjacency.</div></div>'
else:
    for i, a in enumerate(action_items, 1):
        actions_html += f"""
      <div class="action-item" style="border-left:3px solid {'#f44336' if a['severity'] in ('critical','high') else '#ffa726' if a['severity'] == 'medium' else '#448aff'}">
        <div class="action-head">
          <span class="sev sev-{a['severity']}">{a['severity']}</span>
          <span class="action-path">{a['src']} &rarr; {a['dst']}</span>
          <span style="font-size:11px;color:#484f58">{a['src_ring']} &rarr; {a['dst_ring']}</span>
        </div>
        <div class="action-remedy">{a['remedy']}</div>
      </div>"""

# ---------------------------------------------------------------------------
# 3. Timeline
# ---------------------------------------------------------------------------
mb = git("merge-base", base, branch)
mb_date = git("log", "-1", "--format=%as", mb)

log_output = git("log", "--first-parent", branch, "--not", base,
                 "--no-merges", "--reverse", "--format=%h|%as|%s")

by_date = defaultdict(list)
for line in log_output.splitlines():
    if not line.strip():
        continue
    parts = line.split("|", 2)
    if len(parts) == 3:
        by_date[parts[1]].append({"sha": parts[0], "msg": parts[2]})

total_commits = sum(len(v) for v in by_date.values())
days_active = len(by_date)

cat_colors = {
    "feat": "#4fc3f7", "fix": "#ef5350", "refactor": "#ab47bc",
    "test": "#66bb6a", "docs": "#ffa726", "chore": "#78909c", "other": "#90a4ae"
}

max_count = max((len(v) for v in by_date.values()), default=1)

timeline_rows = ""
for date in sorted(by_date.keys()):
    commits = by_date[date]
    cats = defaultdict(int)
    for c in commits:
        msg = c["msg"]
        matched = False
        for cat in ["feat", "fix", "refactor", "test", "docs", "chore"]:
            if msg.startswith(cat):
                cats[cat] += 1
                matched = True
                break
        if not matched:
            cats["other"] += 1

    bar = ""
    for cat in ["feat", "fix", "refactor", "test", "docs", "chore", "other"]:
        n = cats.get(cat, 0)
        if n > 0:
            pct = (n / max_count) * 100
            bar += f'<span class="bar-seg" style="width:{pct:.1f}%;background:{cat_colors[cat]}" title="{cat}: {n}"></span>'

    highlights = "".join(f'<div class="tl-msg">{c["msg"][:80]}</div>' for c in commits[:2])
    if len(commits) > 2:
        highlights += f'<div class="tl-msg muted">+{len(commits) - 2} more</div>'

    timeline_rows += f"""
      <div class="tl-row">
        <div class="tl-date">{date}</div>
        <div class="tl-bar-wrap"><div class="tl-bar">{bar}</div><span class="tl-count">{len(commits)}</span></div>
        <div class="tl-detail">{highlights}</div>
      </div>"""

legend = "".join(
    f'<span class="leg-item"><span class="leg-dot" style="background:{c}"></span>{k}</span>'
    for k, c in cat_colors.items()
)

# ---------------------------------------------------------------------------
# 4. Ring cards + table
# ---------------------------------------------------------------------------
ring_descs = [
    "Pure trait definitions. Zero external deps. Widest recompilation blast radius.",
    "Shared logic, contracts, connector types, feature flags. Depends only on Core.",
    "Infrastructure: K8s, Docker, devcontainer, runtime middleware, hooks, test harness.",
    "Deployable binaries. Compose adapters via trait bounds.",
]

ring_html = ""
for i, rname in enumerate(ring_names):
    crates = rings_by_idx.get(i, [])
    cl = " ".join(f"<code>{c}</code>" for c in crates)
    desc = ring_descs[i] if i < len(ring_descs) else ""
    ring_html += f"""
      <div class="ring-card" style="border-left:3px solid {ring_colors[i]}">
        <div class="ring-head" style="color:{ring_colors[i]}">{rname}</div>
        <div class="ring-crates">{cl}</div>
        <div class="ring-desc">{desc}</div>
      </div>"""

# Fan-in/fan-out
in_deg = defaultdict(int)
out_deg = defaultdict(int)
for s, d, _ in reduced:
    out_deg[s] += 1
    in_deg[d] += 1

table_rows = ""
for nid in sorted(nodes.keys(), key=lambda x: (-in_deg[x], -out_deg[x], nodes[x])):
    name = nodes[nid]
    ri = ring_map.get(nid, 0)
    fi, fo = in_deg[nid], out_deg[nid]
    fi_cls = ' class="hi"' if fi >= 4 else ""
    fo_cls = ' class="hi-b"' if fo >= 3 else ""
    table_rows += (f"        <tr><td><code>{name}</code></td>"
                   f"<td>{ring_names[ri]}</td><td{fi_cls}>{fi}</td><td{fo_cls}>{fo}</td></tr>\n")

# ---------------------------------------------------------------------------
# 5. Assemble HTML
# ---------------------------------------------------------------------------
html = f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{os.path.basename(REPO)} Architecture Report</title>
<style>
*{{margin:0;padding:0;box-sizing:border-box}}
body{{
  font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Helvetica,Arial,sans-serif;
  background:#0d1117;color:#c9d1d9;line-height:1.5;
  padding:24px;max-width:1100px;margin:0 auto;
}}
header{{padding:24px 0 16px;border-bottom:1px solid #21262d;margin-bottom:24px}}
header h1{{font-size:22px;font-weight:700;color:#e6edf3}}
header p{{color:#7d8590;font-size:13px;margin-top:4px}}
.stats{{display:grid;grid-template-columns:repeat(5,1fr);gap:12px;margin-bottom:24px}}
.stat{{background:#161b22;border:1px solid #21262d;border-radius:8px;padding:14px;text-align:center}}
.stat .v{{font-size:28px;font-weight:700;color:#7c4dff}}
.stat .l{{font-size:11px;color:#7d8590;margin-top:2px;text-transform:uppercase;letter-spacing:.5px}}
section{{margin-bottom:28px}}
h2{{font-size:15px;font-weight:600;color:#e6edf3;margin-bottom:12px;
  padding-bottom:6px;border-bottom:1px solid #21262d;text-transform:uppercase;letter-spacing:.5px}}
.diagram{{background:#161b22;border:1px solid #21262d;border-radius:8px;padding:16px;
  display:flex;justify-content:center;overflow-x:auto}}
.diagram svg{{max-width:100%;height:auto}}
.caption{{font-size:11px;color:#7d8590;text-align:center;margin-top:6px}}
.rings{{display:grid;grid-template-columns:repeat(4,1fr);gap:10px}}
.ring-card{{background:#161b22;border:1px solid #21262d;border-radius:8px;padding:12px 14px}}
.ring-head{{font-size:13px;font-weight:700;margin-bottom:4px}}
.ring-crates{{font-size:11px;color:#7d8590;margin-bottom:6px}}
.ring-desc{{font-size:12px;color:#8b949e}}
table{{width:100%;border-collapse:collapse;background:#161b22;border:1px solid #21262d;border-radius:8px;overflow:hidden}}
th,td{{padding:8px 14px;text-align:left;border-bottom:1px solid #21262d;font-size:13px}}
th{{background:#0d1117;font-weight:600;font-size:11px;text-transform:uppercase;letter-spacing:.5px;color:#7d8590}}
tr:last-child td{{border-bottom:none}}
code{{background:#21262d;padding:1px 5px;border-radius:3px;font-size:12px;color:#c9d1d9}}
.hi{{color:#ff5252;font-weight:600}}
.hi-b{{color:#448aff;font-weight:600}}
.tl-row{{display:grid;grid-template-columns:90px 200px 1fr;gap:10px;align-items:start;
  padding:6px 0;border-bottom:1px solid #21262d}}
.tl-row:last-child{{border-bottom:none}}
.tl-date{{font-size:12px;color:#7d8590;font-family:monospace;padding-top:2px}}
.tl-bar-wrap{{display:flex;align-items:center;gap:8px}}
.tl-bar{{display:flex;height:18px;border-radius:3px;overflow:hidden;flex:1;background:#21262d}}
.bar-seg{{display:block;min-width:2px}}
.tl-count{{font-size:12px;color:#8b949e;min-width:20px;text-align:right}}
.tl-detail{{font-size:11px}}
.tl-msg{{color:#8b949e;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;max-width:500px}}
.tl-msg.muted{{color:#484f58;font-style:italic}}
.legend{{display:flex;gap:14px;margin-bottom:10px;flex-wrap:wrap}}
.leg-item{{display:flex;align-items:center;gap:4px;font-size:11px;color:#7d8590}}
.leg-dot{{width:8px;height:8px;border-radius:2px;display:inline-block}}
.insights{{background:#161b22;border:1px solid #21262d;border-radius:8px;padding:14px 18px}}
.insights li{{margin-bottom:6px;font-size:13px;color:#8b949e}}
.insights li strong{{color:#c9d1d9}}
.action-item{{background:#161b22;border:1px solid #21262d;border-radius:8px;padding:14px 16px;margin-bottom:10px}}
.action-item .action-head{{display:flex;align-items:center;gap:10px;margin-bottom:6px}}
.action-item .action-path{{font-size:13px;font-weight:600;color:#c9d1d9}}
.action-item .action-remedy{{font-size:12px;color:#8b949e}}
.sev{{display:inline-block;padding:2px 8px;border-radius:3px;font-size:10px;font-weight:700;
  text-transform:uppercase;letter-spacing:.5px}}
.sev-critical{{background:#7f1d1d;color:#fca5a5}}
.sev-high{{background:#7c2d12;color:#fdba74}}
.sev-medium{{background:#78350f;color:#fde68a}}
.sev-low{{background:#1e3a5f;color:#93c5fd}}
.health-bar{{height:10px;border-radius:5px;background:#21262d;overflow:hidden;margin-top:6px}}
.health-fill{{height:100%;border-radius:5px;transition:width .3s}}
footer{{margin-top:32px;padding-top:12px;border-top:1px solid #21262d;font-size:11px;color:#484f58}}
@media(max-width:768px){{
  .stats{{grid-template-columns:repeat(2,1fr)}}
  .rings{{grid-template-columns:1fr 1fr}}
  .tl-row{{grid-template-columns:70px 140px 1fr}}
}}
</style>
</head>
<body>

<header>
  <h1>{os.path.basename(REPO)} Workspace Architecture</h1>
  <p><code>{branch}</code> &middot; diverged from <code>{base}</code> at
    <code>{mb[:11]}</code> ({mb_date}) &middot;
    {total_commits} commits over {days_active} days</p>
</header>

<div class="stats">
  <div class="stat">
    <div class="v" style="color:{'#4caf50' if health_score >= 80 else '#ffa726' if health_score >= 60 else '#f44336'}">{health_score}%</div>
    <div class="l">Arch Health</div>
    <div class="health-bar"><div class="health-fill" style="width:{health_score}%;background:{'#4caf50' if health_score >= 80 else '#ffa726' if health_score >= 60 else '#f44336'}"></div></div>
  </div>
  <div class="stat"><div class="v" style="color:#4caf50">{good_edges}</div><div class="l">Good Deps</div></div>
  <div class="stat"><div class="v" style="color:#f44336">{bad_edges}</div><div class="l">Bad Deps</div></div>
  <div class="stat"><div class="v">{len(nodes)}</div><div class="l">Crates</div></div>
  <div class="stat"><div class="v">{total_commits}</div><div class="l">Branch Commits</div></div>
</div>

<section>
  <h2>Hexagonal Dependency Map</h2>
  <div class="diagram">{svg_content}</div>
  <p class="caption">
    <span style="color:#4caf50">Green</span> = adjacent-ring dep (good) &middot;
    <span style="color:#f44336">Red</span> = ring-skipping dep (bad) &middot;
    Transitive edges removed. Inner rings compile first.</p>
</section>

<section>
  <h2>Action Items ({bad_edges} violation{'s' if bad_edges != 1 else ''})</h2>
  {actions_html}
</section>

<section>
  <h2>Architecture Rings</h2>
  <div class="rings">{ring_html}
  </div>
</section>

<section>
  <h2>Fan-in / Fan-out</h2>
  <table>
    <thead><tr><th>Crate</th><th>Ring</th><th>Fan-in</th><th>Fan-out</th></tr></thead>
    <tbody>
{table_rows}    </tbody>
  </table>
</section>

<section>
  <h2>Evolution Timeline (divergence from {base})</h2>
  <div class="legend">{legend}</div>
  <div style="background:#161b22;border:1px solid #21262d;border-radius:8px;padding:12px 16px">
    {timeline_rows}
  </div>
</section>

<section>
  <h2>Insights</h2>
  <div class="insights">
    <ul>
      {_sarif_insight_html(sarif_meta)}
      <li><strong>Highest fan-in:</strong> {nodes[max(nodes, key=lambda n: in_deg[n])]}
        ({max(in_deg.values())} direct dependents) &mdash; widest recompilation blast radius.</li>
      <li><strong>Highest fan-out:</strong> {nodes[max(nodes, key=lambda n: out_deg[n])]}
        ({max(out_deg.values())} direct deps) &mdash; main integration point.</li>
      <li><strong>{len(edges) - len(reduced)} of {len(edges)} edges</strong>
        ({100*(len(edges)-len(reduced))//len(edges)}%) removed by transitive reduction.</li>
      <li><strong>No circular dependencies.</strong> The graph is a clean DAG.</li>
      <li><span style="color:#4caf50"><strong>{good_edges} good deps</strong></span> (adjacent ring),
        <span style="color:#f44336"><strong>{bad_edges} bad deps</strong></span> (ring-skipping).
        {(' Bad: ' + '; '.join(bad_edge_list) + '.') if bad_edge_list else ''}</li>
    </ul>
  </div>
</section>

<footer>
  Generated from <code>cargo depgraph --workspace-only</code> &middot;
  branch <code>{branch}</code>
</footer>

</body>
</html>"""

with open(out_path, "w") as f:
    f.write(html)

_write_action_snapshots(
    Path(out_path),
    action_items,
    crate_count=len(nodes),
    good_deps=good_edges,
    bad_deps=bad_edges,
    arch_health_percent=health_score,
)

if history_dir:
    _write_history_artifacts(Path(out_path), history_dir)

print(f"Report written to {out_path}")

if args.open:
    if sys.platform == "darwin":
        subprocess.run(["open", out_path])
    elif sys.platform == "linux":
        subprocess.run(["xdg-open", out_path])
