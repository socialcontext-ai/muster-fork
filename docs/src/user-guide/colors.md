# Colors

Muster sessions have a color applied to the tmux status bar. Colors can be set when creating a profile or changed live on a running session.

## Usage

```bash
# Set by name
muster color webapp orange

# Set by hex
muster color webapp '#f97316'

# Set a shade variant
muster color webapp red-dark

# List available named colors
muster color --list

# colour is accepted as an alias
muster colour webapp teal
```

`color` accepts a profile name, session ID, or full session name. If a session is running, the status bar updates instantly and the profile is also updated. If no session is running, the profile is updated directly.

## Named Colors

<table>
<thead><tr><th>Swatch</th><th>Name</th><th>Aliases</th><th>Hex</th></tr></thead>
<tbody>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#000000;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>black</td><td></td><td><code>#000000</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#cc0000;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>red</td><td></td><td><code>#cc0000</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#4e9a06;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>green</td><td></td><td><code>#4e9a06</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#c4a000;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>yellow</td><td></td><td><code>#c4a000</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#3465a4;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>blue</td><td></td><td><code>#3465a4</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#75507b;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>magenta</td><td></td><td><code>#75507b</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#06989a;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>cyan</td><td></td><td><code>#06989a</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#d3d7cf;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>white</td><td></td><td><code>#d3d7cf</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#f97316;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>orange</td><td></td><td><code>#f97316</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#ec4899;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>pink</td><td></td><td><code>#ec4899</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#a855f7;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>purple</td><td>violet</td><td><code>#a855f7</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#14b8a6;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>teal</td><td></td><td><code>#14b8a6</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#84cc16;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>lime</td><td></td><td><code>#84cc16</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#f59e0b;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>amber</td><td></td><td><code>#f59e0b</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#f43f5e;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>rose</td><td></td><td><code>#f43f5e</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#6366f1;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>indigo</td><td></td><td><code>#6366f1</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#0ea5e9;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>sky</td><td></td><td><code>#0ea5e9</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#10b981;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>emerald</td><td></td><td><code>#10b981</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#d946ef;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>fuchsia</td><td></td><td><code>#d946ef</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#ff7f50;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>coral</td><td></td><td><code>#ff7f50</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#ff6347;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>tomato</td><td></td><td><code>#ff6347</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#dc143c;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>crimson</td><td></td><td><code>#dc143c</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#ffd700;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>gold</td><td></td><td><code>#ffd700</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#000080;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>navy</td><td></td><td><code>#000080</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#8b4513;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>brown</td><td>chocolate</td><td><code>#8b4513</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#64748b;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>slate</td><td></td><td><code>#64748b</code></td></tr>
<tr><td><span style="display:inline-block;width:16px;height:16px;background:#808080;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td>gray</td><td>grey</td><td><code>#808080</code></td></tr>
</tbody>
</table>

## Shade Variants

Append `-light` or `-dark` to any color name for a lighter or darker variant:

```bash
muster color webapp orange-light
muster color webapp orange-dark
```

The following families have curated Tailwind CSS shade values. Other named colors compute light/dark by mixing toward white or scaling channels.

<table>
<thead><tr><th>Family</th><th>-light</th><th></th><th>-dark</th><th></th></tr></thead>
<tbody>
<tr><td>slate</td><td><code>#cbd5e1</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#cbd5e1;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td><code>#334155</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#334155;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td></tr>
<tr><td>gray</td><td><code>#d1d5db</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#d1d5db;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td><code>#374151</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#374151;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td></tr>
<tr><td>red</td><td><code>#fca5a5</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#fca5a5;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td><code>#b91c1c</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#b91c1c;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td></tr>
<tr><td>orange</td><td><code>#fdba74</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#fdba74;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td><code>#c2410c</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#c2410c;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td></tr>
<tr><td>amber</td><td><code>#fcd34d</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#fcd34d;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td><code>#b45309</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#b45309;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td></tr>
<tr><td>yellow</td><td><code>#fde047</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#fde047;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td><code>#a16207</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#a16207;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td></tr>
<tr><td>lime</td><td><code>#bef264</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#bef264;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td><code>#4d7c0f</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#4d7c0f;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td></tr>
<tr><td>green</td><td><code>#86efac</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#86efac;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td><code>#15803d</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#15803d;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td></tr>
<tr><td>emerald</td><td><code>#6ee7b7</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#6ee7b7;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td><code>#047857</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#047857;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td></tr>
<tr><td>teal</td><td><code>#5eead4</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#5eead4;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td><code>#0f766e</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#0f766e;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td></tr>
<tr><td>cyan</td><td><code>#67e8f9</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#67e8f9;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td><code>#0e7490</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#0e7490;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td></tr>
<tr><td>sky</td><td><code>#7dd3fc</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#7dd3fc;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td><code>#0369a1</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#0369a1;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td></tr>
<tr><td>blue</td><td><code>#93c5fd</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#93c5fd;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td><code>#1d4ed8</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#1d4ed8;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td></tr>
<tr><td>indigo</td><td><code>#a5b4fc</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#a5b4fc;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td><code>#4338ca</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#4338ca;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td></tr>
<tr><td>violet</td><td><code>#c4b5fd</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#c4b5fd;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td><code>#6d28d9</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#6d28d9;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td></tr>
<tr><td>purple</td><td><code>#d8b4fe</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#d8b4fe;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td><code>#7e22ce</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#7e22ce;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td></tr>
<tr><td>fuchsia</td><td><code>#f0abfc</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#f0abfc;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td><code>#a21caf</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#a21caf;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td></tr>
<tr><td>pink</td><td><code>#f9a8d4</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#f9a8d4;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td><code>#be185d</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#be185d;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td></tr>
<tr><td>rose</td><td><code>#fda4af</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#fda4af;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td><td><code>#be123c</code></td><td><span style="display:inline-block;width:16px;height:16px;background:#be123c;border:1px solid rgba(128,128,128,0.4);border-radius:3px;vertical-align:middle"></span></td></tr>
</tbody>
</table>

## Hex Colors

Any hex color is accepted directly:

```bash
muster color webapp '#a855f7'
muster color webapp '#10b981'
```
