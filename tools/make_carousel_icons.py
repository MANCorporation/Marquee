#!/usr/bin/env python3
"""Generates the five glossy carousel icons (assets/icons/*.svg) in the classic media-centre style.
Original artwork drawn for Marquee. Run, then render with: marquee --render-svg in.svg out.png 512"""
import math

HEAD = '<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 512 512">\n'
SHADOW = '''<filter id="sh" x="-20%" y="-20%" width="140%" height="150%"><feGaussianBlur stdDeviation="7"/></filter>
<filter id="soft" x="-30%" y="-30%" width="160%" height="160%"><feGaussianBlur stdDeviation="4"/></filter>'''

def blue_grad(id, a='#8fd2ff', b='#2a7bff', c='#0a2bb0'):
    return f'<linearGradient id="{id}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{a}"/><stop offset="0.45" stop-color="{b}"/><stop offset="1" stop-color="{c}"/></linearGradient>'

# ------------------------------------------------------------------ movies: clapperboard
def stripes(x0, y0, w, h, n=7, slant=26):
    out = ''
    sw = w / n
    for i in range(n):
        if i % 2 == 0:
            x = x0 + i * sw
            out += f'<polygon points="{x:.1f},{y0+h} {x+slant:.1f},{y0} {x+slant+sw:.1f},{y0} {x+sw:.1f},{y0+h}" fill="url(#white)"/>'
    return out

movies = HEAD + f'''<defs>{SHADOW}
{blue_grad('scr', '#5db8ff', '#1262ff', '#041a8c')}
<linearGradient id="bezel" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#3a3f4a"/><stop offset="0.4" stop-color="#14171d"/><stop offset="1" stop-color="#050608"/></linearGradient>
<linearGradient id="bar" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#4b505b"/><stop offset="0.5" stop-color="#16181e"/><stop offset="1" stop-color="#05060a"/></linearGradient>
<linearGradient id="white" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#ffffff"/><stop offset="1" stop-color="#c9ced8"/></linearGradient>
<linearGradient id="swoosh" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#fff" stop-opacity="0.55"/><stop offset="1" stop-color="#fff" stop-opacity="0"/></linearGradient>
<radialGradient id="glow" cx="0.35" cy="0.9" r="0.8"><stop offset="0" stop-color="#6fd1ff" stop-opacity="0.9"/><stop offset="1" stop-color="#6fd1ff" stop-opacity="0"/></radialGradient>
<clipPath id="scrclip"><rect x="112" y="226" width="290" height="166" rx="9"/></clipPath>
<clipPath id="lowclip"><rect x="92" y="170" width="326" height="52" rx="6"/></clipPath>
<clipPath id="upclip"><rect x="92" y="170" width="326" height="52" rx="6"/></clipPath>
</defs>
<g transform="translate(0 16) translate(256 270) rotate(-8) scale(0.9) translate(-256 -270)">
 <rect x="94" y="232" width="326" height="182" rx="16" fill="#000" opacity="0.45" filter="url(#sh)"/>
 <rect x="92" y="222" width="326" height="190" rx="16" fill="url(#bezel)"/>
 <rect x="93.5" y="223.5" width="323" height="187" rx="15" fill="none" stroke="#7d8594" stroke-opacity="0.55" stroke-width="2"/>
 <rect x="112" y="226" width="290" height="166" rx="9" fill="url(#scr)"/>
 <g clip-path="url(#scrclip)">
  <rect x="112" y="226" width="290" height="166" fill="url(#glow)"/>
  <path d="M112 226 H402 V270 C330 300 220 262 112 318 Z" fill="url(#swoosh)"/>
 </g>
 <rect x="112" y="226" width="290" height="166" rx="9" fill="none" stroke="#06103a" stroke-opacity="0.8" stroke-width="2.5"/>
 <!-- lower (fixed) bar -->
 <rect x="92" y="170" width="326" height="52" rx="6" fill="url(#bar)"/>
 <g clip-path="url(#lowclip)">{stripes(92, 170, 326, 52, 7, 22)}</g>
 <rect x="92" y="170" width="326" height="52" rx="6" fill="none" stroke="#9aa3b3" stroke-opacity="0.5" stroke-width="2"/>
 <!-- upper (open) bar, hinged on the left -->
 <g transform="rotate(-15 100 166)">
  <rect x="92" y="108" width="326" height="52" rx="6" fill="#000" opacity="0.3" filter="url(#soft)"/>
  <rect x="92" y="108" width="326" height="52" rx="6" fill="url(#bar)"/>
  <clipPath id="upc2"><rect x="92" y="108" width="326" height="52" rx="6"/></clipPath>
  <g clip-path="url(#upc2)">{stripes(92, 108, 326, 52, 7, 22).replace('<polygon','<polygon')}</g>
  <rect x="92" y="108" width="326" height="52" rx="6" fill="none" stroke="#9aa3b3" stroke-opacity="0.5" stroke-width="2"/>
 </g>
 <circle cx="100" cy="166" r="9" fill="#8a919e"/><circle cx="98" cy="164" r="4" fill="#e6eaf2"/>
</g>
</svg>'''

# ------------------------------------------------------------------ music: glossy double note
music = HEAD + f'''<defs>{SHADOW}
{blue_grad('n', '#9adcff', '#2b7dff', '#0a2eb8')}
<linearGradient id="nside" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#0b2fbf"/><stop offset="0.5" stop-color="#3c8cff"/><stop offset="1" stop-color="#0a2aa8"/></linearGradient>
<radialGradient id="hh" cx="0.35" cy="0.3" r="0.6"><stop offset="0" stop-color="#fff" stop-opacity="0.85"/><stop offset="1" stop-color="#fff" stop-opacity="0"/></radialGradient>
</defs>
<g transform="rotate(-4 256 230)">
 <g opacity="0.45" filter="url(#sh)" transform="translate(10 14)" fill="#000">
  <ellipse cx="150" cy="345" rx="68" ry="50" transform="rotate(-22 150 345)"/><ellipse cx="342" cy="312" rx="68" ry="50" transform="rotate(-22 342 312)"/>
  <polygon points="196,340 218,336 218,120 196,126"/><polygon points="388,308 410,304 410,80 388,86"/><polygon points="196,126 410,76 410,142 196,194"/>
 </g>
 <!-- stems -->
 <polygon points="192,342 224,337 224,128 192,134" fill="url(#nside)" stroke="#06208a" stroke-width="2.5" stroke-linejoin="round"/>
 <polygon points="384,309 416,304 416,86 384,92" fill="url(#nside)" stroke="#06208a" stroke-width="2.5" stroke-linejoin="round"/>
 <!-- beam -->
 <polygon points="192,134 416,82 416,152 192,206" fill="url(#n)" stroke="#06208a" stroke-width="3" stroke-linejoin="round"/>
 <polygon points="198,138 410,88 410,104 198,154" fill="#fff" opacity="0.35"/>
 <!-- heads -->
 <ellipse cx="150" cy="346" rx="68" ry="50" transform="rotate(-22 150 346)" fill="url(#n)" stroke="#06208a" stroke-width="3"/>
 <ellipse cx="342" cy="313" rx="68" ry="50" transform="rotate(-22 342 313)" fill="url(#n)" stroke="#06208a" stroke-width="3"/>
 <ellipse cx="132" cy="326" rx="36" ry="20" transform="rotate(-30 132 326)" fill="url(#hh)"/>
 <ellipse cx="324" cy="293" rx="36" ry="20" transform="rotate(-30 324 293)" fill="url(#hh)"/>
</g>
</svg>'''

# ------------------------------------------------------------------ photos: stack of prints
def print_(cx, cy, w, h, rot, photo, id):
    x, y = cx - w / 2, cy - h / 2
    pad = 18
    return f'''<g transform="rotate({rot} {cx} {cy})">
 <rect x="{x+4}" y="{y+10}" width="{w}" height="{h}" rx="5" fill="#000" opacity="0.4" filter="url(#sh)"/>
 <rect x="{x}" y="{y}" width="{w}" height="{h}" rx="5" fill="url(#paper)" stroke="#aab2c0" stroke-width="1.5"/>
 <clipPath id="c{id}"><rect x="{x+pad}" y="{y+pad}" width="{w-2*pad}" height="{h-2*pad-18}"/></clipPath>
 <g clip-path="url(#c{id})">{photo(x+pad, y+pad, w-2*pad, h-2*pad-18)}</g>
 <rect x="{x+pad}" y="{y+pad}" width="{w-2*pad}" height="{h-2*pad-18}" fill="none" stroke="#000" stroke-opacity="0.25" stroke-width="1.5"/>
</g>'''

def sky(x, y, w, h):
    return f'''<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="url(#skyg)"/>
<circle cx="{x+w*0.74}" cy="{y+h*0.3}" r="{h*0.13}" fill="#ffe27a"/>
<path d="M{x} {y+h*0.78} C{x+w*0.3} {y+h*0.45} {x+w*0.55} {y+h*0.95} {x+w} {y+h*0.6} V{y+h} H{x} Z" fill="#3fa84a"/>
<path d="M{x} {y+h*0.9} C{x+w*0.4} {y+h*0.65} {x+w*0.7} {y+h*1.0} {x+w} {y+h*0.82} V{y+h} H{x} Z" fill="#2a8a3c"/>'''

def dusk(x, y, w, h):
    return f'''<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="url(#duskg)"/>
<path d="M{x} {y+h*0.8} L{x+w*0.3} {y+h*0.4} L{x+w*0.5} {y+h*0.7} L{x+w*0.72} {y+h*0.3} L{x+w} {y+h*0.75} V{y+h} H{x} Z" fill="#2b2a5e"/>'''

def meadow(x, y, w, h):
    return f'''<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="url(#meadowg)"/>
<circle cx="{x+w*0.3}" cy="{y+h*0.55}" r="{h*0.12}" fill="#ff5b6e"/><circle cx="{x+w*0.55}" cy="{y+h*0.65}" r="{h*0.1}" fill="#ffd23f"/><circle cx="{x+w*0.78}" cy="{y+h*0.5}" r="{h*0.11}" fill="#ff8c3a"/>'''

photos = HEAD + f'''<defs>{SHADOW}
<linearGradient id="paper" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#ffffff"/><stop offset="1" stop-color="#dfe4ec"/></linearGradient>
<linearGradient id="skyg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#2d86ff"/><stop offset="1" stop-color="#bfe6ff"/></linearGradient>
<linearGradient id="duskg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#3a1f7a"/><stop offset="0.6" stop-color="#e0577a"/><stop offset="1" stop-color="#ffb35a"/></linearGradient>
<linearGradient id="meadowg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#7fd6ff"/><stop offset="0.45" stop-color="#d7f2ff"/><stop offset="0.46" stop-color="#4cb050"/><stop offset="1" stop-color="#2c7d36"/></linearGradient>
</defs>
{print_(214, 214, 270, 220, -15, dusk, 1)}
{print_(300, 225, 270, 220, 9, meadow, 2)}
{print_(248, 262, 290, 240, -3, sky, 3)}
</svg>'''

# ------------------------------------------------------------------ settings: silver gear
def gear(cx, cy, r_out, r_in, teeth, hole):
    pts = []
    for i in range(teeth):
        a0 = 2 * math.pi * i / teeth
        step = 2 * math.pi / teeth
        for frac, r in [(0.0, r_in), (0.14, r_in), (0.24, r_out), (0.46, r_out), (0.56, r_in), (1.0, r_in)]:
            a = a0 + step * frac
            pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    d = 'M' + ' L'.join(f'{x:.1f},{y:.1f}' for x, y in pts) + ' Z'
    return d

g = gear(256, 214, 192, 150, 10, 66)
settings = HEAD + f'''<defs>{SHADOW}
<linearGradient id="metal" x1="0.15" y1="0" x2="0.85" y2="1"><stop offset="0" stop-color="#ffffff"/><stop offset="0.3" stop-color="#d6dbe3"/><stop offset="0.6" stop-color="#8f96a3"/><stop offset="1" stop-color="#4a505b"/></linearGradient>
<linearGradient id="metal2" x1="0.85" y1="0" x2="0.15" y2="1"><stop offset="0" stop-color="#ffffff"/><stop offset="0.45" stop-color="#b9c0cb"/><stop offset="1" stop-color="#5b616d"/></linearGradient>
<radialGradient id="holeg" cx="0.5" cy="0.4" r="0.7"><stop offset="0" stop-color="#1b2230"/><stop offset="1" stop-color="#000"/></radialGradient>
<linearGradient id="spec" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#fff" stop-opacity="0.7"/><stop offset="1" stop-color="#fff" stop-opacity="0"/></linearGradient>
</defs>
<path d="{g}" transform="translate(8 14)" fill="#000" opacity="0.45" filter="url(#sh)"/>
<path d="{g}" fill="url(#metal)" stroke="#3b414c" stroke-width="3" stroke-linejoin="round"/>
<circle cx="256" cy="214" r="122" fill="none" stroke="#fff" stroke-opacity="0.55" stroke-width="3"/>
<circle cx="256" cy="214" r="116" fill="url(#metal2)" stroke="#5b616d" stroke-width="2"/>
<circle cx="256" cy="214" r="78" fill="url(#metal)" stroke="#3b414c" stroke-width="2.5"/>
<circle cx="256" cy="214" r="66" fill="url(#holeg)" stroke="#e8ecf3" stroke-width="3"/>
<path d="M100 150 C140 80 220 50 300 62 C220 70 150 110 120 190 Z" fill="url(#spec)" opacity="0.6"/>
</svg>'''

# ------------------------------------------------------------------ network: glossy globe
lat = ''.join(f'<ellipse cx="256" cy="214" rx="170" ry="{ry}" fill="none" stroke="#bfe8ff" stroke-opacity="0.55" stroke-width="3"/>' for ry in (50, 105))
lon = ''.join(f'<ellipse cx="256" cy="214" rx="{rx}" ry="170" fill="none" stroke="#bfe8ff" stroke-opacity="0.55" stroke-width="3"/>' for rx in (60, 120))
network = HEAD + f'''<defs>{SHADOW}
<radialGradient id="ball" cx="0.35" cy="0.3" r="0.85"><stop offset="0" stop-color="#9be0ff"/><stop offset="0.45" stop-color="#2a7bff"/><stop offset="1" stop-color="#06208a"/></radialGradient>
<radialGradient id="hl" cx="0.4" cy="0.25" r="0.6"><stop offset="0" stop-color="#fff" stop-opacity="0.85"/><stop offset="1" stop-color="#fff" stop-opacity="0"/></radialGradient>
<linearGradient id="ring" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#e8ecf3"/><stop offset="0.5" stop-color="#9aa3b3"/><stop offset="1" stop-color="#e8ecf3"/></linearGradient>
<clipPath id="gl"><circle cx="256" cy="214" r="170"/></clipPath>
</defs>
<circle cx="264" cy="228" r="170" fill="#000" opacity="0.45" filter="url(#sh)"/>
<circle cx="256" cy="214" r="170" fill="url(#ball)" stroke="#06208a" stroke-width="3"/>
<g clip-path="url(#gl)">{lat}{lon}
 <ellipse cx="256" cy="214" rx="170" ry="2" fill="none"/>
 <path d="M86 214 H426" stroke="#bfe8ff" stroke-opacity="0.55" stroke-width="3"/><path d="M256 44 V384" stroke="#bfe8ff" stroke-opacity="0.55" stroke-width="3"/>
 <ellipse cx="200" cy="120" rx="110" ry="62" fill="url(#hl)" transform="rotate(-25 200 120)"/>
</g>
<ellipse cx="256" cy="214" rx="214" ry="58" fill="none" stroke="url(#ring)" stroke-width="12" transform="rotate(-18 256 214)" opacity="0.95"/>
<path d="M62 258 C100 288 180 300 256 296" fill="none" stroke="#fff" stroke-opacity="0.0"/>
</svg>'''

for name, svg in [('movies', movies), ('music', music), ('photos', photos), ('settings', settings), ('sources', network)]:
    open(f'assets/icons/{name}.svg', 'w').write(svg)
print('ok')
