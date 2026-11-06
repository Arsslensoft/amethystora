// Generates the Amethystora artwork under system_files/ from the geometry and palette below.
//
//   node branding/generate.mjs
//
// SVGs and the fastfetch logo are written directly. PNGs are rendered with a
// headless Chromium browser (Edge or Chrome); set BROWSER to its path if it is
// not found automatically.

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { tmpdir } from "node:os";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const SHARED = join(ROOT, "system_files/shared");

// -------------------------------------------------------------- palette --

// Sampled from a deep amethyst, from the shadow in its pavilion to the glint on its crown
const SHADES = ["#16042c", "#260850", "#380d78", "#4a13a0", "#5d1ac4", "#7429e0", "#9148f0", "#b37df9", "#dcbcff"];

// The desktop accent (boot splash, fastfetch): a dark amethyst that still reads on dark surfaces.
// 06-branding.sh carries the same value.
const ACCENT = "#7c3aed";

// ---------------------------------------------------------------- the gem --

// The mark is a lowercase a cut like a crystal, in a 256x256 box: a hexagon bowl with upright sides, and
// for a stem its right side carried up to a point and down to a foot. cut() gives its corners from four
// lengths: from the bowl's centre to its top and bottom points (reach) and to its sides (half), the
// thickness of the ring all the way round (ring), and how far the stem's foot stands above the bowl's
// lowest point (lift), which is where the baseline runs when the mark is set as a letter. Upper-case
// names are corners of the outside, lower-case ones of the counter.
function cut(reach, half, ring, lift) {
  const length = Math.hypot(half, reach / 2);
  const [slope, cos, sin] = [reach / 2 / half, half / length, reach / 2 / length];
  // The ring is as thick measured across a slanted side as across an upright one
  const [peak, mitre, inner] = [ring / cos, (ring * (1 - sin)) / cos, half - ring];
  const notch = [inner, -reach + slope * inner];
  return {
    T: [0, -reach], UR: [half, -reach / 2], B: [0, reach], LL: [-half, reach / 2], UL: [-half, -reach / 2],
    t: [0, -reach + peak], ur: [inner, -reach / 2 + mitre], lr: [inner, reach / 2 - mitre],
    b: [0, reach - peak], ll: [-inner, reach / 2 - mitre], ul: [-inner, -reach / 2 + mitre],
    // Where the stem leaves the bowl, above and below, its point, and the two corners of its foot
    notch, join: [inner, reach - slope * inner], tip: [half, notch[1] - slope * ring],
    footR: [half, reach - lift], footL: [inner, reach - lift],
  };
}
const outlineOf = ({ UL, T, notch, tip, footR, footL, join, B, LL }) => [UL, T, notch, tip, footR, footL, join, B, LL];
const counterOf = ({ t, ur, lr, b, ll, ul }) => [t, ur, lr, b, ll, ul];

// The foot stands this far above the bowl's lowest point
const LIFT = 9;
const MARK = cut(90, 94, 58, LIFT);
// The bowl's centre in the box, placed so that the mark stands in the middle of it
const HUB = [128, 128 - (MARK.B[1] + MARK.tip[1]) / 2];
const placed = (pts) => pts.map(([x, y]) => [HUB[0] + x, HUB[1] + y]);

// Seven facets, each with its shade and the part it belongs to: five sides of the ring, clockwise from the
// upper left, then the point of the stem and its body. Light comes from the upper left, so the ring is
// lightest there and darkens round to the lower right.
const FACETS = (({ T, UR, B, LL, UL, t, ur, lr, b, ll, ul, notch, join, tip, footR, footL }) =>
  [
    [[UL, T, t, ul], 8, "ring"],
    [[T, notch, ur, t], 6, "ring"],
    [[lr, join, B, b], 4, "ring"],
    [[B, LL, ll, b], 5, "ring"],
    [[LL, UL, ul, ll], 7, "ring"],
    [[notch, tip, UR, ur], 7, "stem"],
    [[ur, UR, footR, footL], 6, "stem"],
  ].map(([pts, shade, part]) => ({ pts: placed(pts), shade, part })))(MARK);

// The outline of the whole mark, and the counter it leaves open in the bowl
const OUTLINE = placed(outlineOf(MARK));
const COUNTER = placed(counterOf(MARK));

// Tight bounds of the mark, for the bitmaps: [x, y, width, height]
const BOUNDS = (() => {
  const xs = OUTLINE.map(([x]) => x);
  const ys = OUTLINE.map(([, y]) => y);
  const [x0, y0] = [Math.min(...xs), Math.min(...ys)];
  return [x0, y0, Math.max(...xs) - x0, Math.max(...ys) - y0].map((v) => +v.toFixed(1));
})();

// The middle of the mark, where the boot splash centres its halo and the light inside the mark
const CENTER = [128, 128];

const toward = ([x0, y0], [x1, y1], s) => [x0 + (x1 - x0) * s, y0 + (y1 - y0) * s];
const centroid = (pts) => [0, 1].map((c) => pts.reduce((sum, p) => sum + p[c], 0) / pts.length);

// Light comes from the upper left, in front of the mark
const LIGHT = (() => {
  const l = [-0.45, -0.7, 0.55];
  const n = Math.hypot(...l);
  return l.map((c) => c / n);
})();
const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];

const clamp01 = (x) => Math.min(1, Math.max(0, x));
// Drawn in white, on a coloured ground, a facet keeps its place in the light by how much of the white shows
const opacityOf = (shade) => (0.4 + (0.6 * shade) / (SHADES.length - 1)).toFixed(2);

const pointList = (pts) => pts.map(([x, y]) => `${+x.toFixed(1)},${+y.toFixed(1)}`).join(" ");

// Each facet is stroked in its own colour, which hides the hairline seams antialiasing leaves between them
function gemPolygons(white = false, facets = FACETS) {
  return facets
    .map((f) =>
      white
        ? `<polygon points="${pointList(f.pts)}" fill="#ffffff" fill-opacity="${opacityOf(f.shade)}"/>`
        : `<polygon points="${pointList(f.pts)}" fill="${SHADES[f.shade]}" stroke="${SHADES[f.shade]}" stroke-width="0.6" stroke-linejoin="round"/>`,
    )
    .join("\n  ");
}

// The icon keeps the padded 256x256 box; bitmaps use the tight bounds of the mark. Given facets, only
// those are drawn: the bowl or the stem on its own, for the apps that move one against the other.
const gemSvg = (white = false, viewBox = "0 0 256 256", facets = FACETS) => `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${viewBox}">
  ${gemPolygons(white, facets)}
</svg>
`;

// ---------------------------------------------------------------- icons --

// Amethystora versions of the Universal Blue icons the inherited launchers used; the launchers in
// system_files name these instead.

// Symbolic (single colour, recoloured by GTK): the same cut at 16px, with the ring and the stem brought
// to 4px so that their upright edges fall on whole pixels, and the counter left open.
const symbolicSvg = (() => {
  const small = cut(6.6, 7, 4, 0.7);
  const hub = (16 - (small.B[1] - small.tip[1])) / 2 - small.tip[1];
  const path = (pts) => "M" + pts.map(([x, y]) => [8 + x, hub + y].map((v) => +v.toFixed(2)).join(",")).join("L") + "z";
  return `<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16">
  <path fill="#2e3436" fill-rule="evenodd" d="${path(outlineOf(small))}${path(counterOf(small))}"/>
</svg>
`;
})();

// Full-colour app icon: white glyph on a rounded amethyst tile.
const tileSvg = (glyph) => `<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256" viewBox="0 0 256 256">
  <defs>
    <linearGradient id="tile" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="${SHADES[6]}"/>
      <stop offset="1" stop-color="${SHADES[2]}"/>
    </linearGradient>
  </defs>
  <rect x="16" y="16" width="224" height="224" rx="52" fill="url(#tile)"/>
  ${glyph}
</svg>
`;

// The gem centred on the tile, in white facets
const docsGlyph = (() => {
  const s = 156 / BOUNDS[3];
  const [tx, ty] = [128 - s * (BOUNDS[0] + BOUNDS[2] / 2), 128 - s * (BOUNDS[1] + BOUNDS[3] / 2)];
  return `<g transform="translate(${tx.toFixed(1)} ${ty.toFixed(1)}) scale(${s.toFixed(3)})">
    ${gemPolygons(true)}
  </g>`;
})();

const communityGlyph = `<g fill="#ffffff">
    <rect x="108" y="118" width="88" height="64" rx="16" fill-opacity="0.7"/>
    <path d="M172 182l10 22 6-22z" fill-opacity="0.7"/>
    <rect x="60" y="68" width="108" height="76" rx="18"/>
    <path d="M80 144l-6 24 28-24z"/>
  </g>
  <g fill="${SHADES[4]}">
    <circle cx="92" cy="106" r="8"/>
    <circle cx="114" cy="106" r="8"/>
    <circle cx="136" cy="106" r="8"/>
  </g>`;

const updateGlyph = `<g fill="none" stroke="#ffffff" stroke-width="20">
    <path d="M79.1 110.2A52 52 0 0 1 172 100"/>
    <path d="M176.9 145.8A52 52 0 0 1 84 156"/>
  </g>
  <g fill="#ffffff">
    <path d="M154 94h40l-20 28z"/>
    <path d="M62 162h40l-20-28z"/>
  </g>`;

// Lines of a log, and a magnifying glass over them
const logsGlyph = `<g fill="none" stroke="#ffffff" stroke-width="16" stroke-linecap="round">
    <path d="M60 76h124M60 108h88M60 140h48"/>
    <circle cx="164" cy="164" r="30"/>
    <path d="M186 186l18 18"/>
  </g>`;

// A clock with an arrow running back round it: the hours the snapshots and the backup go back to
const backupsGlyph = `<g fill="none" stroke="#ffffff" stroke-width="16" stroke-linecap="round" stroke-linejoin="round">
    <path d="M72.6 100A64 64 0 1 1 65 143.1"/>
    <path d="M128 100v34l22 14"/>
  </g>
  <path d="M57.3 86.5L91.9 106.5L61.6 119z" fill="#ffffff"/>`;

// A vault door: the body in an outline on two feet, and its dial in four quarters, the tile showing between them
const vaultGlyph = `<g fill="none" stroke="#ffffff" stroke-width="14">
    <rect x="58" y="56" width="140" height="130" rx="34"/>
  </g>
  <g fill="#ffffff">
    <circle cx="128" cy="121" r="38"/>
    <rect x="74" y="188" width="26" height="16" rx="6"/>
    <rect x="156" y="188" width="26" height="16" rx="6"/>
  </g>
  <g fill="${SHADES[4]}">
    <rect x="122" y="83" width="12" height="76"/>
    <rect x="90" y="115" width="76" height="12"/>
  </g>`;

// Three sliders, each set somewhere else along its track
const controlGlyph = `<g fill="none" stroke="#ffffff" stroke-width="16" stroke-linecap="round" stroke-opacity="0.6">
    <path d="M64 84h128M64 128h128M64 172h128"/>
  </g>
  <g fill="#ffffff">
    <circle cx="150" cy="84" r="20"/>
    <circle cx="98" cy="128" r="20"/>
    <circle cx="170" cy="172" r="20"/>
  </g>`;

// ----------------------------------------------------------- wallpapers --

// A hexagonal crystal point: three visible column faces capped by three pyramid faces.
function crystal({ x, base, w, h, tip, rot, faces }) {
  const top = base - h;
  const xs = [x - w / 2, x - w / 4, x + w / 4, x + w / 2];
  const apex = `${x},${top - tip}`;
  const polys = [];
  for (let i = 0; i < 3; i++) {
    polys.push(`<polygon points="${xs[i]},${base} ${xs[i]},${top} ${xs[i + 1]},${top} ${xs[i + 1]},${base}" fill="${faces.column[i]}"/>`);
    polys.push(`<polygon points="${xs[i]},${top} ${xs[i + 1]},${top} ${apex}" fill="${faces.tip[i]}"/>`);
  }
  return `<g transform="rotate(${rot} ${x} ${base})">${polys.join("")}</g>`;
}

const CLUSTER = [
  { x: 2180, base: 2500, w: 200, h: 640, tip: 170, rot: -38 },
  { x: 3740, base: 2500, w: 220, h: 700, tip: 180, rot: 30 },
  { x: 2520, base: 2450, w: 300, h: 980, tip: 250, rot: -21 },
  { x: 3360, base: 2450, w: 340, h: 1060, tip: 270, rot: 15 },
  { x: 2920, base: 2450, w: 420, h: 1380, tip: 330, rot: -5 },
];

function wallpaperSvg(theme) {
  const t = theme === "dark"
    ? {
        bg: ["#0e0419", "#1f0840", "#35106a"],
        glowA: ["#7429e0", 0.45], glowB: ["#4a13a0", 0.5],
        faces: { column: ["#5d1ac4", "#4a13a0", "#2c0a5c"], tip: ["#9148f0", "#7429e0", "#4a13a0"] },
        shade: "#0e0419",
      }
    : {
        bg: ["#f6f0fd", "#e2d2f8", "#c6a8f0"],
        glowA: ["#ffffff", 0.7], glowB: ["#9148f0", 0.3],
        faces: { column: ["#9148f0", "#7429e0", "#5d1ac4"], tip: ["#dcbcff", "#b37df9", "#9148f0"] },
        shade: "#c6a8f0",
      };
  return `<svg xmlns="http://www.w3.org/2000/svg" width="3840" height="2160" viewBox="0 0 3840 2160">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="${t.bg[0]}"/>
      <stop offset="0.55" stop-color="${t.bg[1]}"/>
      <stop offset="1" stop-color="${t.bg[2]}"/>
    </linearGradient>
    <radialGradient id="glowA" cx="2900" cy="900" r="1700" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="${t.glowA[0]}" stop-opacity="${t.glowA[1]}"/>
      <stop offset="1" stop-color="${t.glowA[0]}" stop-opacity="0"/>
    </radialGradient>
    <radialGradient id="glowB" cx="600" cy="2000" r="1500" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="${t.glowB[0]}" stop-opacity="${t.glowB[1]}"/>
      <stop offset="1" stop-color="${t.glowB[0]}" stop-opacity="0"/>
    </radialGradient>
    <linearGradient id="fade" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0.55" stop-color="${t.shade}" stop-opacity="0"/>
      <stop offset="1" stop-color="${t.shade}" stop-opacity="0.85"/>
    </linearGradient>
  </defs>
  <rect width="3840" height="2160" fill="url(#bg)"/>
  <rect width="3840" height="2160" fill="url(#glowA)"/>
  <rect width="3840" height="2160" fill="url(#glowB)"/>
  ${CLUSTER.map((c) => crystal({ ...c, faces: t.faces })).join("\n  ")}
  <rect width="3840" height="2160" fill="url(#fade)"/>
</svg>
`;
}

// ------------------------------------------------------- boot splash --

// The splash has two parts, on plain black: a picture over the whole screen made every frame slow on a
// machine still drawing without its graphics driver. In the first the mark grows: the sides of its bowl
// appear one after another and settle, then the stem rises from its foot to its point. The flash that
// marks it whole is the first beat of the second part, which lasts until the login screen takes over:
// the glow breathes down from that height and up again, for as long as the boot takes. BOOT_TIMELINE drives the browser preview
// (node branding/generate.mjs --preview) and bootScript() ports it to Plymouth's script language, so
// both play the same animation. It sticks to the maths that language has: no exp, pow or floor.
//
// The mark is drawn in its 256-unit box; BOOT.unit converts that to pixels on a 1080p screen.
const BOOT = {
  unit: 280 / BOUNDS[3], // the mark stands 280px tall at 1080p
  centerY: 0.46, // of the screen height
  // s of plain sky before anything appears: when the graphics driver takes over, a monitor can take a second or
  // two to show a picture again, and the mark would grow before anyone could see it
  hold: 2,
  first: 0.2, // s before the first side of the bowl appears
  each: 0.13, // s from one side to the next
  fadeIn: 0.22, // s a side takes to appear
  settle: [14, 0.3], // how far out a side starts, in units, and the s it takes to come in
  stem: 1.0, // s at which the stem starts to grow
  whole: 1.7, // s at which it reaches its point: the flash, and the start of the second part
  cycle: 3.2, // s per glow cycle
  halo: 760, // px across at 1080p
  sky: "#000000",
};
// The violet night of the dark wallpapers and of the installer, under its crown
const CROWN_SKY = ["#0d0714", "#1a0a2e"];

// A seeded generator, so every run of this script draws the same dust
function random(seed) {
  return () => {
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// The bowl comes in one side at a time, clockwise from the left, each from a little way out along the
// line from the bowl's centre through its own. c is the middle of its picture, a square with room to spare.
const SIDES = [4, 0, 1, 2, 3].map((i) => {
  const facet = FACETS[i];
  const xs = facet.pts.map(([x]) => x);
  const ys = facet.pts.map(([, y]) => y);
  const c = [(Math.min(...xs) + Math.max(...xs)) / 2, (Math.min(...ys) + Math.max(...ys)) / 2];
  const side = Math.ceil(Math.max(Math.max(...xs) - Math.min(...xs), Math.max(...ys) - Math.min(...ys))) + 8;
  const [mx, my] = centroid(facet.pts);
  const out = Math.hypot(mx - HUB[0], my - HUB[1]);
  return { facet, c, side, dirx: (mx - HUB[0]) / out, diry: (my - HUB[1]) / out };
});

// The stem is one piece, both its facets, in a picture that ends at its foot, so that it can grow from there
const STEM = (() => {
  const pts = FACETS.filter((f) => f.part === "stem").flatMap((f) => f.pts);
  const xs = pts.map(([x]) => x);
  const ys = pts.map(([, y]) => y);
  const [x, y] = [Math.min(...xs) - 1, Math.min(...ys) - 1];
  return { x, y, width: Math.max(...xs) + 1 - x, height: Math.max(...ys) - y };
})();

// The installer's background: the crown of the gem this mark took the place of, seen from above and laid over the whole 1920x1080
// screen with the mark on its table: 16 points on a girdle wider than the screen, 8 star points part of
// the way in from every other one, and the 8 corners of the table, each with its height above the
// girdle. It is turned a sixteenth of a turn from that gem's, so that the table lies flat along the top.
const HEART = [960, 1080 * BOOT.centerY];
const CROWN = (() => {
  const girdle = (k) => {
    const a = (2 * Math.PI * (k + 1)) / 16 - Math.PI / 2;
    return [HEART[0] + 1500 * Math.cos(a), HEART[1] + 980 * Math.sin(a), 0];
  };
  const inward = (p, s, height) => [...toward(HEART, p, s), height];
  return { girdle, star: (i) => inward(girdle(2 * i + 1), 0.52, 340), table: (i) => inward(girdle(2 * i), 0.29, 510) };
})();

// How a facet of the crown faces the light, from about -1 (in shadow) to 1, from the plane through its
// first three points
function crownLevel([a, b, c]) {
  const u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
  const v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
  const n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
  const len = Math.hypot(...n) * (Math.sign(n[2]) || 1);
  return (Math.max(0, dot(n.map((x) => x / len), LIGHT)) - 0.55) * 1.6;
}

// Its shades are all dark, so that the mark stands out on it; the cuts between the facets are drawn faintly
const CROWN_TONES = ["#0c0515", "#120821", "#180a2e", "#1f0c3c", "#260e4a", "#2e1059"];
const crownSvg = (() => {
  const { girdle, star, table } = CROWN;
  const cuts = `stroke="${SHADES[6]}" stroke-opacity="0.13" stroke-width="2" stroke-linejoin="round"`;
  const facet = (pts, bias = 0) => {
    const tone = CROWN_TONES[Math.round(clamp01((crownLevel(pts) + bias + 1) / 2) * (CROWN_TONES.length - 1))];
    return `<polygon points="${pointList(pts)}" fill="${tone}" ${cuts}/>`;
  };
  // 8 bezel, 8 star and 16 upper girdle facets; neighbouring girdle facets alternate lighter and darker,
  // as a cut stone sparkles
  const facets = [];
  for (let i = 0; i < 8; i++) {
    const sparkle = i % 2 ? 0.22 : -0.08;
    facets.push(
      facet([table(i), star(i - 1), girdle(2 * i), star(i)]),
      facet([table(i), star(i), table(i + 1)], -0.1),
      facet([star(i), girdle(2 * i), girdle(2 * i + 1)], sparkle),
      facet([star(i), girdle(2 * i + 1), girdle(2 * i + 2)], 0.14 - sparkle),
    );
  }
  // The table, flat and the darkest of all, is where the mark stands
  facets.push(`<polygon points="${pointList(Array.from({ length: 8 }, (_, i) => table(i)))}" fill="#100720" ${cuts}/>`);
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1920 1080">
  ${facets.join("\n  ")}
</svg>
`;
})();

// One side of the bowl, drawn exactly as in the logo, in a square around it
const sideSvg = (s) => gemSvg(false, [s.c[0] - s.side / 2, s.c[1] - s.side / 2, s.side, s.side].join(" "), [s.facet]);
const stemSvg = gemSvg(false, [STEM.x, STEM.y, STEM.width, STEM.height].join(" "), FACETS.filter((f) => f.part === "stem"));

// The light that wells up inside the mark as it glows
const lightSvg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256">
  <defs>
    <radialGradient id="light" cx="${CENTER[0]}" cy="${CENTER[1]}" r="120" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#ffffff" stop-opacity="0.9"/>
      <stop offset="0.25" stop-color="#e3a2ff" stop-opacity="0.45"/>
      <stop offset="1" stop-color="${SHADES[7]}" stop-opacity="0"/>
    </radialGradient>
    <clipPath id="gem"><path clip-rule="evenodd" d="M${pointList(OUTLINE)}Z M${pointList(COUNTER)}Z"/></clipPath>
  </defs>
  <rect x="0" y="0" width="256" height="256" fill="url(#light)" clip-path="url(#gem)"/>
</svg>
`;

const mix = (a, b, s) =>
  "#" + [1, 3, 5].map((i) => {
    const [x, y] = [a, b].map((hex) => parseInt(hex.slice(i, i + 2), 16));
    return Math.round(x + (y - x) * s).toString(16).padStart(2, "0");
  }).join("");

// 41 gradient stops of a(o), o running from the centre (0) to the rim (1), the colour blending from inner to
// outer. Enough stops that the steps between them never show as rings.
const stops = (inner, outer, a) =>
  Array.from({ length: 41 }, (_, i) => {
    const o = i / 40;
    return `<stop offset="${o}" stop-color="${mix(inner, outer, o)}" stop-opacity="${a(o).toFixed(3)}"/>`;
  }).join("");

// Violet smoke over a whole screen, in a 480x270 box stretched to it, for the Nebula wallpaper: layers of
// fractal noise thin and thicken a glow that is densest around the centre given and thins out towards the
// corners without ever ending. Each layer is [seed, frequency, colour, peak, floor]; the colour blends to
// rim at the edge.
const nebula = ({ center: [cx, cy], rim, layers }) => {
  const layer = ([seed, frequency, color, peak, floor]) => {
    const falloff = (o) => peak * (floor + (1 - floor) * Math.exp(-3.5 * o * o));
    return `<filter id="smoke${seed}" x="-10%" y="-10%" width="120%" height="120%">
      <feTurbulence type="fractalNoise" baseFrequency="${frequency}" numOctaves="4" seed="${seed}"/>
      <feColorMatrix values="2 0 0 0 -0.5  2 0 0 0 -0.5  2 0 0 0 -0.5  0 0 0 0 1"/>
      <feGaussianBlur stdDeviation="2.5"/>
    </filter>
    <mask id="clouds${seed}"><rect width="480" height="270" filter="url(#smoke${seed})"/></mask>
    <radialGradient id="fall${seed}" cx="${cx}" cy="${cy}" r="300" gradientUnits="userSpaceOnUse">${stops(color, rim, falloff)}</radialGradient>`;
  };
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 480 270" preserveAspectRatio="none">
  <defs>
    ${layers.map(layer).join("\n    ")}
  </defs>
  ${layers.map(([seed]) => `<rect width="480" height="270" fill="url(#fall${seed})" mask="url(#clouds${seed})"/>`).join("\n  ")}
</svg>
`;
};

// The soft glow right around the mark that brightens and dims with it: a bell curve that flattens out to
// nothing before its rim, so it has no edge.
const haloSvg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 200">
  <defs><radialGradient id="halo">${stops(SHADES[7], SHADES[6], (o) => 0.8 * Math.exp(-4 * o * o) * (1 - o * o) ** 2)}</radialGradient></defs>
  <ellipse cx="100" cy="100" rx="86" ry="100" fill="url(#halo)"/>
</svg>
`;

// ----------------------------------------------------- boot animation --

// Where everything is t seconds into the splash: the sides in 1080p pixels from the heart, the stem as a
// share of its height. The Plymouth script is a line-for-line port of this function.
const BOOT_TIMELINE = String.raw`
function clamp(u) {
  return Math.min(1, Math.max(0, u));
}
function frame(t, B, SIDES) {
  t = Math.max(0, t - B.hold);
  const whole = t >= B.whole ? 1 : 0;
  // Part 1: the sides of the bowl, each coming in from a little way out as it appears
  const sides = SIDES.map((s, i) => {
    const start = B.first + B.each * i;
    const v = 1 - clamp((t - start) / B.settle[1]);
    const out = B.settle[0] * v * v * v;
    return {
      x: (s.c[0] - 128 + s.dirx * out) * B.unit,
      y: (s.c[1] - 128 + s.diry * out) * B.unit,
      opacity: whole ? 0 : clamp((t - start) / B.fadeIn),
    };
  });
  // ...then the stem from its foot, a little past its height and back
  const v = clamp((t - B.stem) / (B.whole - B.stem)) - 1;
  const stem = {
    height: 0.12 + 0.88 * (1 + 1.9 * v * v * v + 0.9 * v * v),
    opacity: whole ? 0 : clamp((t - B.stem) / 0.12),
  };
  // The flash, and the glow of part 2, which starts at its height, where the flash leaves it
  const rise = clamp((t - 0.3) / B.whole);
  const since = (t - B.whole) / 0.75;
  const flash = since >= 0 && since < 1 ? (1 - since) * (1 - since) * (1 - since) : 0;
  const g = whole ? 0.5 + 0.5 * Math.cos((2 * Math.PI * (t - B.whole)) / B.cycle) : 0;
  return {
    sides, stem, gem: whole,
    halo: Math.min(1, rise * (0.3 + 0.7 * g) + 0.5 * flash),
    light: whole * Math.min(1, 0.45 * g + 0.9 * flash),
  };
}
`;

const svgData = (svg) => `data:image/svg+xml;base64,${Buffer.from(svg).toString("base64")}`;

// An animated preview of the splash on a 1920x1080 stage, scaled to the window
function bootPreviewHtml() {
  const k = BOOT.unit;
  const img = (id, svg, w, h, extra = "") =>
    `<img id="${id}" src="${svgData(svg)}" style="width:${w}px;height:${h}px;margin:${-h / 2}px 0 0 ${-w / 2}px${extra}">`;
  return `<!doctype html><html><head><meta charset="utf-8"><title>Amethystora boot splash</title><style>
    ${wordmarkFace}
    html,body{margin:0;height:100%;background:#000;overflow:hidden;font-family:"${WORDMARK_FONT}",sans-serif}
    #stage{position:absolute;left:50%;top:50%;width:1920px;height:1080px;transform-origin:0 0;overflow:hidden;
      background:${BOOT.sky}}
    #heart{position:absolute;left:960px;top:${1080 * BOOT.centerY}px}
    #heart img{position:absolute;left:0;top:0;will-change:transform,opacity}
    /* The stem keeps its place in the mark and grows from its foot */
    #heart #stem{left:${(STEM.x - 128) * k}px;top:${(STEM.y - 128) * k}px;width:${STEM.width * k}px;height:${STEM.height * k}px;
      transform-origin:50% 100%}
    #mark{position:absolute;left:0;right:0;top:${Math.round(1080 * 0.94) - 18}px;text-align:center;color:#fff;
      font-weight:${WORDMARK_WEIGHT};letter-spacing:-0.015em;font-size:34px;line-height:36px}
    #mark img.a{${GEM_CSS};${glowCss(WORDMARK.glow)}}
    /* The author credit under the name, faint, so that it only shows when looked for */
    #credit{position:absolute;left:0;right:0;top:${Math.round(1080 * 0.94) + 26}px;text-align:center;color:${SHADES[7]};
      opacity:0.22;font-weight:500;letter-spacing:0.04em;font-size:13px;line-height:16px}
    #bar{position:fixed;left:12px;bottom:12px;display:flex;gap:8px;align-items:center;color:#cdb8ec;font:13px system-ui}
    button{background:#2a1740;color:#eadcff;border:1px solid #4a2d70;border-radius:6px;padding:6px 12px;font:inherit;cursor:pointer}
  </style></head><body>
  <div id="stage"><div id="heart">
    ${img("halo", haloSvg, BOOT.halo, BOOT.halo)}
    ${SIDES.map((s, i) => img(`side${i}`, sideSvg(s), s.side * k, s.side * k)).join("")}
    <img id="stem" src="${svgData(stemSvg)}">
    ${img("gem", gemSvg(), 256 * k, 256 * k)}
    ${img("light", lightSvg, 256 * k, 256 * k)}
  </div><div id="mark">${wordmark(true)}</div><div id="credit">by Arsslensoft</div></div>
  <div id="bar"><button id="replay">Replay</button><button id="slow">Slow motion: off</button><span id="clock"></span></div>
  <script>
    ${BOOT_TIMELINE}
    const B = ${JSON.stringify(BOOT)}, SIDES = ${JSON.stringify(SIDES.map(({ c, dirx, diry }) => ({ c, dirx, diry })))};
    const $ = (id) => document.getElementById(id);
    const stage = $("stage");
    const fit = () => {
      const s = Math.min(innerWidth / 1920, innerHeight / 1080);
      stage.style.transform = "scale(" + s + ") translate(-960px,-540px)";
    };
    addEventListener("resize", fit); fit();
    let t0 = performance.now(), t = 0, last = t0, speed = 1;
    $("replay").onclick = () => { t = 0; };
    $("slow").onclick = () => { speed = speed === 1 ? 0.25 : 1; $("slow").textContent = "Slow motion: " + (speed === 1 ? "off" : "on"); };
    const put = (el, x, y, opacity) => {
      el.style.transform = "translate(" + x + "px," + y + "px)";
      el.style.opacity = opacity;
    };
    function tick(now) {
      t += ((now - last) / 1000) * speed; last = now;
      // #t=1.5 in the address holds the animation at that moment
      const hold = /t=([\\d.]+)/.exec(location.hash);
      if (hold) t = +hold[1];
      const f = frame(t, B, SIDES);
      f.sides.forEach((s, i) => put($("side" + i), s.x, s.y, s.opacity));
      $("stem").style.transform = "scaleY(" + f.stem.height + ")";
      $("stem").style.opacity = f.stem.opacity;
      put($("gem"), 0, 0, f.gem); put($("light"), 0, 0, f.light);
      put($("halo"), 0, 0, f.halo);
      $("clock").textContent = t.toFixed(2) + " s";
      requestAnimationFrame(tick);
    }
    document.fonts.load('${WORDMARK_WEIGHT} 16px "${WORDMARK_FONT}"').finally(() => requestAnimationFrame(tick));
  </script></body></html>`;
}

// ------------------------------------------------------ fastfetch logo --

function inPolygon([x, y], poly) {
  let inside = false;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const [xi, yi] = poly[i];
    const [xj, yj] = poly[j];
    if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) inside = !inside;
  }
  return inside;
}

function colorAt(x, y) {
  const facet = FACETS.find((f) => inPolygon([x, y], f.pts));
  return facet ? SHADES[facet.shade] : null;
}

const rgb = (hex) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)).join(";");

// The logo in a terminal is 26 pixels tall, 13 lines, and as wide in columns, which leaves the summary
// fastfetch prints beside it 50 columns of an 80-column terminal
const LOGO_ROWS = 26;

// Truecolour half-block art: every character cell shows two vertically stacked pixels. light, when given,
// recolours each pixel of the gem from its colour and position (the glint frames below).
function ansiLogo(rows = LOGO_ROWS, light = null) {
  const cols = Math.round((rows * BOUNDS[2]) / BOUNDS[3]);
  const [x0, y0, w, h] = BOUNDS;
  const px = (i, j) => {
    const [x, y] = [x0 + ((i + 0.5) * w) / cols, y0 + ((j + 0.5) * h) / rows];
    const color = colorAt(x, y);
    return color && light ? light(color, x, y) : color;
  };
  const lines = [];
  for (let j = 0; j < rows; j += 2) {
    let line = "";
    for (let i = 0; i < cols; i++) {
      const top = px(i, j);
      const bottom = px(i, j + 1);
      if (!top && !bottom) line += "\x1b[0m ";
      else if (top && !bottom) line += `\x1b[0;38;2;${rgb(top)}m▀`;
      else if (!top && bottom) line += `\x1b[0;38;2;${rgb(bottom)}m▄`;
      else line += `\x1b[38;2;${rgb(top)};48;2;${rgb(bottom)}m▀`;
    }
    lines.push(line.replace(/(\x1b\[0m )+$/, "") + "\x1b[0m");
  }
  return lines.join("\n") + "\n";
}

// The Linux text console has 16 colours and maps truecolour to the nearest, which turns the dark facets blue.
// Its variant uses the console's own colours: magenta, bright magenta, and white for the side the light falls
// on. Bright colours only exist in the foreground, so the brighter pixel of each cell is drawn with the glyph.
const CONSOLE_TONES = { [SHADES[8]]: 2, [SHADES[7]]: 1, [SHADES[6]]: 1 };
const CONSOLE_FG = ["35", "95", "97"];
const CONSOLE_BG = ["45", "45", "47"];

function consoleLogo(rows = LOGO_ROWS) {
  const cols = Math.round((rows * BOUNDS[2]) / BOUNDS[3]);
  const [x0, y0, w, h] = BOUNDS;
  const tone = (i, j) => {
    const color = colorAt(x0 + ((i + 0.5) * w) / cols, y0 + ((j + 0.5) * h) / rows);
    return color && (CONSOLE_TONES[color] ?? 0);
  };
  const lines = [];
  for (let j = 0; j < rows; j += 2) {
    let line = "";
    for (let i = 0; i < cols; i++) {
      const top = tone(i, j);
      const bottom = tone(i, j + 1);
      if (top === null && bottom === null) line += "\x1b[0m ";
      else if (bottom === null) line += `\x1b[0;${CONSOLE_FG[top]}m▀`;
      else if (top === null) line += `\x1b[0;${CONSOLE_FG[bottom]}m▄`;
      else if (top === bottom) line += `\x1b[0;${CONSOLE_FG[top]}m█`;
      else if (top > bottom) line += `\x1b[0;${CONSOLE_FG[top]};${CONSOLE_BG[bottom]}m▀`;
      else line += `\x1b[0;${CONSOLE_FG[bottom]};${CONSOLE_BG[top]}m▄`;
    }
    lines.push(line.replace(/(\x1b\[0m )+$/, "") + "\x1b[0m");
  }
  return lines.join("\n") + "\n";
}

// ----------------------------------------------------- fastfetch glint --

// amethystora-fetch plays these frames before it prints the system summary: a band of light crosses the
// gem the way the light falls on it, from the upper left, and each facet flashes whole as the band
// passes, as a cut stone does when it turns. Only colours change, never the silhouette, so every frame
// covers the one before it exactly. The last frame is the still logo, which fastfetch prints over it.
const GLINT_FRAMES = 18;
const GLINT_COLOR = "#f6efff";
// Half-width of the band, in the gem's 256-unit box
const GLINT_WIDTH = 20;
// How many shades of the palette the band lifts a facet at its centre, where it also turns towards white.
// Climbing the gem's own shades keeps a lit facet amethyst; mixing straight to white would grey it.
const GLINT_LIFT = 5;

// How far a point lies along the band's path, which runs the way the light falls
const glintAlong = (x, y) => -(x * LIGHT[0] + y * LIGHT[1]) / Math.hypot(LIGHT[0], LIGHT[1]);

const FACET_CENTRES = FACETS.map((f) => [0, 1].map((a) => f.pts.reduce((s, p) => s + p[a], 0) / f.pts.length));
// The band runs from just before the first facet to just past the last, so no frame is spent off the gem
const GLINT_SPAN = (() => {
  const along = FACET_CENTRES.map(([x, y]) => glintAlong(x, y));
  return [Math.min(...along) - GLINT_WIDTH, Math.max(...along) + GLINT_WIDTH];
})();

const mixHex = (a, b, t) =>
  "#" +
  [1, 3, 5]
    .map((i) => Math.round(parseInt(a.slice(i, i + 2), 16) * (1 - t) + parseInt(b.slice(i, i + 2), 16) * t))
    .map((c) => c.toString(16).padStart(2, "0"))
    .join("");

function glintFrames() {
  const frames = Array.from({ length: GLINT_FRAMES }, (_, k) => {
    const centre = GLINT_SPAN[0] + ((GLINT_SPAN[1] - GLINT_SPAN[0]) * k) / (GLINT_FRAMES - 1);
    return ansiLogo(LOGO_ROWS, (color, x, y) => {
      const facet = FACETS.findIndex((f) => inPolygon([x, y], f.pts));
      const [cx, cy] = facet < 0 ? [x, y] : FACET_CENTRES[facet];
      const d = (glintAlong(cx, cy) - centre) / GLINT_WIDTH;
      const lit = Math.exp(-d * d);
      const shade = SHADES[Math.min(SHADES.length - 1, SHADES.indexOf(color) + Math.round(lit * GLINT_LIFT))];
      return mixHex(shade, GLINT_COLOR, Math.max(0, lit - 0.6) * 1.2);
    });
  });
  return [...frames, ansiLogo()];
}

// ------------------------------------------------------------ PNG logos --

// The wordmark is set in Quicksand Bold (branding/fonts, SIL Open Font License), whose own a is a bowl and a
// stem, as the mark is, so the mark reads as the "a" of the name. The font is embedded so the rendering does
// not depend on the fonts of this machine.
const WORDMARK_FONT = "Quicksand";
const WORDMARK_WEIGHT = 700;
const wordmarkFace = `@font-face{font-family:"${WORDMARK_FONT}";font-weight:300 700;src:url(data:font/ttf;base64,${readFileSync(
  join(ROOT, "branding/fonts/QuicksandVariable.ttf"),
).toString("base64")}) format("truetype")}`;

const gemImg = (white, cls = "") =>
  `<img${cls ? ` class="${cls}"` : ""} src="data:image/svg+xml;base64,${Buffer.from(
    gemSvg(white, BOUNDS.join(" ")),
  ).toString("base64")}">`;

// The wordmark: the gem is the "a" of amethystora, set as a letter against "methystora", a little larger and
// heavier than its neighbours so that it holds its own as the logo. One unit of its box is 0.0036em: its ring
// comes to 209 per 1000 em beside Quicksand Bold's stems of 120, and its bowl is as wide as it is tall, as
// that face's round letters are. The foot of its stem stands on the baseline, and the bowl's lowest point
// just under it, the way a round letter overshoots. Without the gem the name is set in full.
// The apps' stylesheets set their own wordmarks with these same four lengths.
const GEM_UNIT = 0.0036;
const GEM_CSS = `height:${(BOUNDS[3] * GEM_UNIT).toFixed(3)}em;width:${(BOUNDS[2] * GEM_UNIT).toFixed(3)}em;margin-right:0.055em;vertical-align:${(-LIFT * GEM_UNIT).toFixed(3)}em`;
const wordmark = (gem) => (gem ? `<span>${gemImg(false, "a")}methystora</span>` : "<span>amethystora</span>");

// A palette colour at an opacity, for the glow below
const alpha = (hex, a) => `rgba(${[1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)).join(",")},${a})`;

const glowCss = (glow) =>
  glow
    ? `filter:drop-shadow(0 0 ${glow}px ${alpha(SHADES[8], 0.9)})
       drop-shadow(0 0 ${(glow * 2.4).toFixed(1)}px ${alpha(SHADES[7], 0.65)})
       drop-shadow(0 0 ${(glow * 4.6).toFixed(1)}px ${alpha(SHADES[6], 0.5)});`
    : "";

// Gem or wordmark centred on a transparent canvas, scaled down to fit with a margin.
//
// glow lights the gem the way the boot splash does, in the same three shades working outwards, so
// the logo under the login dialog is the gem still glowing after the splash has faded. It is a stack
// of drop-shadows rather than a halo image because a shadow follows the gem's own silhouette, and
// because it costs no layout: the row measures the same with it as without.
//
// fit is how much of the canvas the wordmark is allowed to fill. A glow reaches well past the gem
// and the canvas clips, so a glowing logo is given a wider margin to spread into rather than being
// drawn smaller.
function logoHtml({ width, height, text, white = false, gem = true, glow = 0, fit = 0.92 }) {
  const textColor = text === "white" ? "#ffffff" : "#241f31";
  const body = `<div id="row">${text ? wordmark(gem) : gemImg(white)}</div>`;
  const glowFilter = glowCss(glow);
  return `<!doctype html><html><head><style>
    ${wordmarkFace}
    html,body{margin:0;width:${width}px;height:${height}px;background:transparent;overflow:hidden}
    body{display:flex;align-items:center;justify-content:center}
    #row{display:flex;align-items:center;white-space:nowrap}
    #row>img{height:${Math.round(height * 0.84)}px;${glowFilter}}
    span{font-family:"${WORDMARK_FONT}";font-weight:${WORDMARK_WEIGHT};letter-spacing:-0.015em;font-size:${Math.round(height * 0.62)}px;
      color:${textColor};line-height:1}
    img.a{${GEM_CSS};${glowFilter}}
  </style></head><body>${body}<script>
    // Measure once the wordmark font is in use, not the fallback font
    document.fonts.load('${WORDMARK_WEIGHT} 16px "${WORDMARK_FONT}"').then(() => {
      const row = document.getElementById("row");
      const scale = Math.min(1, (${width} * ${fit}) / row.offsetWidth, (${height} * ${fit}) / row.offsetHeight);
      row.style.transform = "scale(" + scale + ")";
    });
  </script></body></html>`;
}

// ------------------------------------------------------ Plymouth theme --

// The splash runs in Plymouth's script plugin. Its images are drawn at twice their 1080p size so they stay
// sharp up to 4K, and the script scales each one once at start-up to fit the screen. The halo is soft
// enough to be drawn smaller and stretched.
const THEME = "usr/share/plymouth/themes/amethystora";
const GEM_PX = 256 * BOOT.unit; // the gem's 256-unit box, in 1080p pixels
// The name at the foot of the splash is the login screen's logo, the gem glowing as the "a", so the splash hands
// over to GDM with the same mark in both. Its box has the proportions of amethystora-wordmark-glow.png and the
// glow is scaled with it, so the lettering comes out at the 34px the credit below is spaced for.
const WORDMARK = { box: [276, 90], glow: 6, fit: 0.66 };
const TEXT = {
  credit: { text: "by Arsslensoft", size: 13, weight: 500, spacing: "0.04em", color: SHADES[7], box: [130, 18], opacity: 0.22 },
};
// Centre of the name, as a share of the screen height, and of the credit below it, in 1080p pixels
const WORDMARK_Y = 0.94;
const CREDIT_BELOW = 34;

const svgPage = (svg, width, height) => `<!doctype html><html><head><style>
    html,body{margin:0;background:transparent;overflow:hidden}
    svg{display:block;width:${width}px;height:${height}px}
  </style></head><body>${svg}</body></html>`;

// A line of text in the wordmark font, centred in its box, at twice its 1080p size
const textPage = ({ text, size, weight, spacing, color, box }) => `<!doctype html><html><head><style>
    ${wordmarkFace}
    html,body{margin:0;width:${box[0] * 2}px;height:${box[1] * 2}px;background:transparent;overflow:hidden}
    body{display:flex;align-items:center;justify-content:center;font-family:"${WORDMARK_FONT}";font-weight:${weight};
      letter-spacing:${spacing};font-size:${size * 2}px;line-height:1;color:${color};white-space:nowrap}
  </style></head><body>${text}<script>document.fonts.load('${weight} 16px "${WORDMARK_FONT}"');</script></body></html>`;

// The password field: a pill in the colours of the splash with the lock inside it, on the left; the typed
// characters show as dots after the lock. In 1080p pixels.
const FIELD = { width: 360, height: 44, textX: 48, dot: 10, gap: 5 };
const fieldSvg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${FIELD.width} ${FIELD.height}">
  <rect x="0.75" y="0.75" width="${FIELD.width - 1.5}" height="${FIELD.height - 1.5}" rx="${(FIELD.height - 1.5) / 2}"
    fill="#140824" fill-opacity="0.6" stroke="${SHADES[7]}" stroke-opacity="0.55" stroke-width="1.5"/>
  <path d="M21.5 20v-3.5a4.5 4.5 0 0 1 9 0V20" fill="none" stroke="#e9dcff" stroke-width="2" stroke-linecap="round"/>
  <rect x="18" y="20" width="16" height="12" rx="2.5" fill="#e9dcff"/>
</svg>
`;
const dotSvg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><circle cx="5" cy="5" r="4" fill="#efe3ff"/></svg>
`;

const BOOT_PNGS = [
  { out: `${THEME}/field.png`, width: FIELD.width * 2, height: FIELD.height * 2, html: () => svgPage(fieldSvg, FIELD.width * 2, FIELD.height * 2) },
  { out: `${THEME}/dot.png`, width: FIELD.dot * 2, height: FIELD.dot * 2, html: () => svgPage(dotSvg, FIELD.dot * 2, FIELD.dot * 2) },
  { out: `${THEME}/halo.png`, width: BOOT.halo / 2, height: BOOT.halo / 2, html: () => svgPage(haloSvg, BOOT.halo / 2, BOOT.halo / 2) },
  ...[["gem", gemSvg()], ["light", lightSvg]].map(([name, svg]) => {
    const px = Math.round(GEM_PX * 2);
    return { out: `${THEME}/${name}.png`, width: px, height: px, html: () => svgPage(svg, px, px) };
  }),
  ...SIDES.map((s, i) => {
    const px = Math.round(s.side * BOOT.unit * 2);
    return { out: `${THEME}/side-${i}.png`, width: px, height: px, html: () => svgPage(sideSvg(s), px, px) };
  }),
  (() => {
    const [width, height] = [STEM.width, STEM.height].map((v) => Math.round(v * BOOT.unit * 2));
    return { out: `${THEME}/stem.png`, width, height, html: () => svgPage(stemSvg, width, height) };
  })(),
  {
    out: `${THEME}/wordmark.png`, width: WORDMARK.box[0] * 2, height: WORDMARK.box[1] * 2,
    html: () => logoHtml({ width: WORDMARK.box[0] * 2, height: WORDMARK.box[1] * 2, text: "white", glow: WORDMARK.glow * 2, fit: WORDMARK.fit }),
  },
  ...Object.entries(TEXT).map(([name, t]) => ({
    out: `${THEME}/${name}.png`, width: t.box[0] * 2, height: t.box[1] * 2, html: () => textPage(t),
  })),
];

// Numbers for the script: plain decimals, never exponents
const num = (v) => {
  const s = v.toFixed(4).replace(/\.?0+$/, "");
  return s === "-0" || s === "" ? "0" : s;
};
const rgb01 = (hex) => [1, 3, 5].map((i) => num(parseInt(hex.slice(i, i + 2), 16) / 255)).join(", ");

// The splash as a Plymouth script: the same timeline as BOOT_TIMELINE, plus the password prompt, messages and
// update progress, which the script plugin leaves to the theme.
function bootScript() {
  const B = BOOT;
  const K = num(B.unit);
  // Set a sprite's opacity only when it changes, so sprites that hold still are not redrawn
  const fade = (name, expr) => `  o = ${expr};
  if (Math.Abs(o - global.${name}.opacity) > 0.004) {
    global.${name}.sprite.SetOpacity(o);
    global.${name}.opacity = o;
  }`;
  const sides = SIDES.map((s, i) => `side[${i}].image = sized(Image("side-${i}.png"), ${num(s.side * B.unit)} * scale, ${num(s.side * B.unit)} * scale);
side[${i}].sprite = Sprite(side[${i}].image);
side[${i}].sprite.SetZ(3);
side[${i}].sprite.SetOpacity(0);
side[${i}].x = ${num(s.c[0] - 128)};
side[${i}].y = ${num(s.c[1] - 128)};
side[${i}].dirx = ${num(s.dirx)};
side[${i}].diry = ${num(s.diry)};`).join("\n");

  return `# The Amethystora boot splash, for Plymouth's script plugin.
# Generated by branding/generate.mjs from the same timeline as its browser preview
# (node branding/generate.mjs --preview): change it there, not here.

Window.SetBackgroundTopColor(${rgb01(B.sky)});
Window.SetBackgroundBottomColor(${rgb01(B.sky)});

# The mark and everything round it are laid out for a 1920x1080 screen and scaled to fit this one.
screen_x = Window.GetX();
screen_y = Window.GetY();
screen_w = Window.GetWidth();
screen_h = Window.GetHeight();
scale = Math.Min(screen_w / 1920, screen_h / 1080);
origin_x = screen_x + screen_w / 2;
origin_y = screen_y + screen_h * ${num(B.centerY)};

fun sized(image, width, height) {
  if (width < 1) { width = 1; }
  if (height < 1) { height = 1; }
  return image.Scale(Math.Int(width + 0.5), Math.Int(height + 0.5));
}

fun clamp01(v) {
  if (v < 0) { return 0; }
  if (v > 1) { return 1; }
  return v;
}

# ------------------------------------------------------------------- scene --

halo.image = sized(Image("halo.png"), ${B.halo} * scale, ${B.halo} * scale);
halo.sprite = Sprite(halo.image);
halo.sprite.SetPosition(origin_x - halo.image.GetWidth() / 2, origin_y - halo.image.GetHeight() / 2, 1);
halo.sprite.SetOpacity(0);
halo.opacity = 0;

side_count = ${SIDES.length};
${sides}

# The stem keeps its place in the mark and grows from its foot
stem.image = sized(Image("stem.png"), ${num(STEM.width * B.unit)} * scale, ${num(STEM.height * B.unit)} * scale);
stem.grown = stem.image;
stem.height = 0;
stem.foot = origin_y + ${num((STEM.y + STEM.height - 128) * B.unit)} * scale;
stem.sprite = Sprite(stem.image);
stem.sprite.SetX(origin_x + ${num((STEM.x - 128) * B.unit)} * scale);
stem.sprite.SetZ(3);
stem.sprite.SetOpacity(0);

gem.image = sized(Image("gem.png"), ${num(GEM_PX)} * scale, ${num(GEM_PX)} * scale);
gem.sprite = Sprite(gem.image);
gem.sprite.SetPosition(origin_x - gem.image.GetWidth() / 2, origin_y - gem.image.GetHeight() / 2, 4);
gem.sprite.SetOpacity(0);

light.image = sized(Image("light.png"), ${num(GEM_PX)} * scale, ${num(GEM_PX)} * scale);
light.sprite = Sprite(light.image);
light.sprite.SetPosition(origin_x - light.image.GetWidth() / 2, origin_y - light.image.GetHeight() / 2, 5);
light.sprite.SetOpacity(0);
light.opacity = 0;

wordmark.image = sized(Image("wordmark.png"), ${WORDMARK.box[0]} * scale, ${WORDMARK.box[1]} * scale);
wordmark.sprite = Sprite(wordmark.image);
wordmark_y = screen_y + screen_h * ${WORDMARK_Y};
wordmark.sprite.SetPosition(origin_x - wordmark.image.GetWidth() / 2, wordmark_y - wordmark.image.GetHeight() / 2, 6);

credit.image = sized(Image("credit.png"), ${TEXT.credit.box[0]} * scale, ${TEXT.credit.box[1]} * scale);
credit.sprite = Sprite(credit.image);
credit.sprite.SetPosition(origin_x - credit.image.GetWidth() / 2, wordmark_y + ${CREDIT_BELOW} * scale - credit.image.GetHeight() / 2, 6);
credit.sprite.SetOpacity(${TEXT.credit.opacity});

# ---------------------------------------------------------------- timeline --

# Shutdown and reboot skip the first part and start in the second, with the mark whole and the glow at
# its height
tick = 0;
mode = Plymouth.GetMode();
if (mode == "shutdown" || mode == "reboot") {
  tick = Math.Int(${num(B.hold + B.whole + B.cycle)} * 50);
}
grown = 0;

# Called 50 times a second
fun refresh_callback() {
  t = global.tick / 50 - ${num(B.hold)};
  if (t < 0) { t = 0; }
  global.tick = global.tick + 1;

  if (t < ${num(B.whole)}) {
    # Part 1: the sides of the bowl, each coming in from a little way out as it appears
    for (i = 0; i < global.side_count; i++) {
      start = ${num(B.first)} + ${num(B.each)} * i;
      v = 1 - clamp01((t - start) / ${num(B.settle[1])});
      out = ${num(B.settle[0])} * v * v * v;
      x = global.origin_x + (global.side[i].x + global.side[i].dirx * out) * ${K} * global.scale;
      y = global.origin_y + (global.side[i].y + global.side[i].diry * out) * ${K} * global.scale;
      global.side[i].sprite.SetX(x - global.side[i].image.GetWidth() / 2);
      global.side[i].sprite.SetY(y - global.side[i].image.GetHeight() / 2);
      global.side[i].sprite.SetOpacity(clamp01((t - start) / ${num(B.fadeIn)}));
    }
    # ...then the stem from its foot, a little past its height and back
    v = clamp01((t - ${num(B.stem)}) / ${num(B.whole - B.stem)}) - 1;
    height = Math.Int(global.stem.image.GetHeight() * (0.12 + 0.88 * (1 + 1.9 * v * v * v + 0.9 * v * v)) + 0.5);
    if (height < 1) { height = 1; }
    if (height != global.stem.height) {
      global.stem.grown = global.stem.image.Scale(global.stem.image.GetWidth(), height);
      global.stem.sprite.SetImage(global.stem.grown);
      global.stem.sprite.SetY(global.stem.foot - height);
      global.stem.height = height;
    }
    global.stem.sprite.SetOpacity(clamp01((t - ${num(B.stem)}) / 0.12));
  } else if (!global.grown) {
    # The mark is whole: one picture takes the place of its pieces
    for (i = 0; i < global.side_count; i++) {
      global.side[i].sprite.SetOpacity(0);
    }
    global.stem.sprite.SetOpacity(0);
    global.gem.sprite.SetOpacity(1);
    global.grown = 1;
  }

  # The flash, and the glow of part 2, which starts at its height, where the flash leaves it
  rise = clamp01((t - 0.3) / ${num(B.whole)});
  flash = 0;
  since = (t - ${num(B.whole)}) / 0.75;
  if (since >= 0 && since < 1) { flash = (1 - since) * (1 - since) * (1 - since); }
  glow = 0;
  whole = 0;
  if (t >= ${num(B.whole)}) {
    glow = 0.5 + 0.5 * Math.Cos(6.2831853 * (t - ${num(B.whole)}) / ${num(B.cycle)});
    whole = 1;
  }

${fade("halo", "Math.Min(1, rise * (0.3 + 0.7 * glow) + 0.5 * flash)")}
${fade("light", "whole * Math.Min(1, 0.45 * glow + 0.9 * flash)")}

}
Plymouth.SetRefreshFunction(refresh_callback);

# ------------------------------------------------ password and questions --

# Text scales with the screen like everything else; size is in points at 1080p
fun font(size) {
  points = Math.Int(size * global.scale + 0.5);
  if (points < 9) { points = 9; }
  return "Inter " + points;
}

# The field under the gem, the lock drawn inside it, and the prompt above
field_image = sized(Image("field.png"), ${FIELD.width} * scale, ${FIELD.height} * scale);
dot_image = sized(Image("dot.png"), ${FIELD.dot} * scale, ${FIELD.dot} * scale);
field_x = origin_x - field_image.GetWidth() / 2;
field_y = origin_y + 210 * scale - field_image.GetHeight() / 2;
text_x = field_x + ${FIELD.textX} * scale;
dot_step = (${FIELD.dot} + ${FIELD.gap}) * scale;
dot_room = Math.Int((field_image.GetWidth() - ${FIELD.textX + FIELD.height / 2} * scale) / dot_step);

dialog.field = Sprite(field_image);
dialog.field.SetPosition(field_x, field_y, 10);
dialog.prompt = Sprite();
dialog.prompt.SetZ(10);
dialog.text = Sprite();
dialog.text.SetZ(11);

fun hide_dots() {
  for (i = 0; global.dialog.dot[i]; i++) {
    global.dialog.dot[i].SetOpacity(0);
  }
}

fun hide_dialog() {
  global.dialog.field.SetOpacity(0);
  global.dialog.prompt.SetOpacity(0);
  global.dialog.text.SetOpacity(0);
  hide_dots();
}
hide_dialog();

fun show_dialog(prompt) {
  global.dialog.field.SetOpacity(1);
  image = Image.Text(prompt, 0.93, 0.89, 1, 1, font(12));
  global.dialog.prompt.SetImage(image);
  global.dialog.prompt.SetX(global.origin_x - image.GetWidth() / 2);
  global.dialog.prompt.SetY(global.field_y - image.GetHeight() - 14 * global.scale);
  global.dialog.prompt.SetOpacity(1);
}

fun display_password_callback(prompt, bullets) {
  show_dialog(prompt);
  global.dialog.text.SetOpacity(0);
  for (i = 0; global.dialog.dot[i] || i < bullets; i++) {
    if (!global.dialog.dot[i]) {
      global.dialog.dot[i] = Sprite(global.dot_image);
      global.dialog.dot[i].SetX(global.text_x + i * global.dot_step);
      global.dialog.dot[i].SetY(global.field_y + global.field_image.GetHeight() / 2 - global.dot_image.GetHeight() / 2);
      global.dialog.dot[i].SetZ(11);
    }
    if (i < bullets && i < global.dot_room) {
      global.dialog.dot[i].SetOpacity(1);
    } else {
      global.dialog.dot[i].SetOpacity(0);
    }
  }
}
Plymouth.SetDisplayPasswordFunction(display_password_callback);

fun display_question_callback(prompt, entry) {
  show_dialog(prompt);
  hide_dots();
  image = Image.Text(entry, 1, 1, 1, 1, font(12));
  global.dialog.text.SetImage(image);
  global.dialog.text.SetX(global.text_x);
  global.dialog.text.SetY(global.field_y + global.field_image.GetHeight() / 2 - image.GetHeight() / 2);
  global.dialog.text.SetOpacity(1);
}
Plymouth.SetDisplayQuestionFunction(display_question_callback);

fun display_normal_callback() {
  hide_dialog();
}
Plymouth.SetDisplayNormalFunction(display_normal_callback);

# ------------------------------------------------------ messages, updates --

message = Sprite();
message.SetZ(10);
fun message_callback(text) {
  image = Image.Text(text, 0.86, 0.8, 0.96, 1, font(11));
  global.message.SetImage(image);
  global.message.SetX(global.origin_x - image.GetWidth() / 2);
  global.message.SetY(global.screen_y + global.screen_h * 0.84);
}
Plymouth.SetMessageFunction(message_callback);

# Offline updates and upgrades: what is happening and how far along, under the gem
if (mode == "updates" || mode == "system-upgrade" || mode == "firmware-upgrade") {
  title = "Installing updates";
  if (mode == "system-upgrade") { title = "Upgrading the system"; }
  if (mode == "firmware-upgrade") { title = "Upgrading firmware"; }
  title_image = Image.Text(title + " - do not turn off your computer", 0.93, 0.89, 1, 1, font(14));
  status.title = Sprite(title_image);
  status.title.SetPosition(origin_x - title_image.GetWidth() / 2, origin_y + 190 * scale, 10);
  status.progress = Sprite();
  status.progress.SetZ(10);
}

fun system_update_callback(progress) {
  image = Image.Text(Math.Int(progress) + "%", 0.86, 0.8, 0.96, 1, font(12));
  global.status.progress.SetImage(image);
  global.status.progress.SetX(global.origin_x - image.GetWidth() / 2);
  global.status.progress.SetY(global.origin_y + 230 * global.scale);
}
Plymouth.SetSystemUpdateFunction(system_update_callback);
`;
}

// Wallpapers are rendered to PNG
const wallpaperHtml = (theme) => `<!doctype html><html><body style="margin:0">${wallpaperSvg(theme)}</body></html>`;

// ------------------------------------------------------- more wallpapers --

// Super+Ctrl+Space cycles through the current theme's wallpapers. The Amethystora themes get three more
// of their own, each in a light and a dark version like the crystal field: the stone broken open, its cut
// face, and a night sky of violet smoke. Every theme, Amethystora's included, also gets three drawn
// in its own palette. Those are templates in /usr/share/amethystora/themed, written here with {{ key }}
// wherever a colour goes, which amethystora-theme fills in from the theme's colors.toml. They keep to
// flat shapes and gradients, because GNOME draws an SVG wallpaper itself, with librsvg, at every login.

const [WALL_W, WALL_H] = [3840, 2160];
const n1 = (v) => +v.toFixed(1);
const polyPath = (pts, close = true) => "M" + pts.map(([x, y]) => `${n1(x)},${n1(y)}`).join("L") + (close ? "Z" : "");
const wallSvg = (defs, body) => `<svg xmlns="http://www.w3.org/2000/svg" width="${WALL_W}" height="${WALL_H}" viewBox="0 0 ${WALL_W} ${WALL_H}">
  <defs>
    ${defs}
  </defs>
  ${body}
</svg>
`;
const fillAll = (fill, extra = "") => `<rect width="${WALL_W}" height="${WALL_H}" fill="${fill}"${extra}/>`;

// A colour a fraction t of the way along a list of colours
const ramp = (colors, t) => {
  const u = clamp01(t) * (colors.length - 1);
  const i = Math.min(colors.length - 2, Math.floor(u));
  return mix(colors[i], colors[i + 1], u - i);
};

// The facets' shades, darkest first: amethyst in the dark, lilac to white in the light
const FACET_TONES = {
  dark: ["#0e0419", "#1f0840", "#35106a", SHADES[3], SHADES[5], SHADES[6]],
  light: [SHADES[7], "#c6a8f0", "#e2d2f8", "#f6f0fd", "#ffffff"],
};

// The cut face of a stone, filling the screen: a jittered grid of triangles, each a flat facet tilted by a
// gentle swell under it and shaded by how it faces the light, brightest around the crystal field's glow.
function facetsSvg(theme, tones = theme === "dark" ? FACET_TONES.dark : FACET_TONES.light) {
  const dark = theme !== "light";
  const rnd = random(59);
  const [cols, rows] = [24, 14];
  const [cw, ch] = [WALL_W / (cols - 3), WALL_H / (rows - 3)];
  const swell = (x, y) =>
    240 * (Math.sin(x / 820 + 0.6) * Math.cos(y / 640 - 0.3) + 0.6 * Math.sin((x + 1.7 * y) / 1150 + 2.1) + 0.3 * Math.cos((x - y) / 520));
  // The grid runs a cell past every edge, so no facet ends short of the screen's
  const grid = Array.from({ length: rows }, (_, j) =>
    Array.from({ length: cols }, (_, i) => {
      const [x, y] = [(i - 1 + (rnd() - 0.5) * 0.64) * cw, (j - 1 + (rnd() - 0.5) * 0.64) * ch];
      return [x, y, swell(x, y)];
    }));
  const facets = [];
  for (let j = 0; j < rows - 1; j++) {
    for (let i = 0; i < cols - 1; i++) {
      const [a, b, c, d] = [grid[j][i], grid[j][i + 1], grid[j + 1][i + 1], grid[j + 1][i]];
      for (const [p, q, r] of rnd() < 0.5 ? [[a, b, c], [a, c, d]] : [[a, b, d], [b, c, d]]) {
        const u = [q[0] - p[0], q[1] - p[1], q[2] - p[2]];
        const v = [r[0] - p[0], r[1] - p[1], r[2] - p[2]];
        const n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        const len = Math.hypot(...n) * Math.sign(n[2]);
        const [mx, my] = centroid([p, q, r]);
        const glow = Math.exp(-((mx - 2900) ** 2 + (my - 850) ** 2) / (2 * 1500 ** 2));
        const lit = dot(n.map((x) => x / len), LIGHT) - 0.55;
        const color = ramp(tones, (dark ? 0.12 : 0.3) + 0.6 * glow + 1.5 * lit - (dark ? 0.12 : 0.05) * (my / WALL_H));
        facets.push(`<path d="${polyPath([p, q, r])}" fill="${color}" stroke="${color}" stroke-width="1" stroke-linejoin="round"/>`);
      }
    }
  }
  // Under the facets, so that their antialiased seams come out opaque: Mutter blends two pictures with alpha without the first
  return wallSvg("", `${fillAll("#2a0f4a")}\n  ${facets.join("\n  ")}`);
}

// A night sky in the boot splash's two colours, with violet smoke drawn off to the right, where the
// crystal field stands, and dust across it. The smoke is turbulence, which only the browser renders.
function nebulaWallpaperHtml(theme) {
  const dark = theme === "dark";
  const rnd = random(83);
  const sky = dark ? CROWN_SKY : ["#f6f0fd", "#dcc8f6"];
  const smoke = dark
    ? nebula({ center: [330, 110], rim: SHADES[3], layers: [[3, 0.014, SHADES[6], 0.8, 0.12], [11, 0.024, SHADES[5], 0.45, 0.1]] })
    : nebula({ center: [330, 110], rim: SHADES[7], layers: [[3, 0.014, SHADES[6], 0.5, 0.1], [11, 0.024, SHADES[7], 0.4, 0.1]] });
  const speck = dark ? "#f0d4ff" : SHADES[5];
  const dust = Array.from({ length: 280 }, () =>
    `<circle cx="${n1(rnd() * WALL_W)}" cy="${n1(rnd() * WALL_H)}" r="${n1(1 + 2.6 * rnd() ** 3)}" fill="${speck}" fill-opacity="${(0.25 + 0.6 * rnd()).toFixed(2)}"/>`);
  // and a few motes that glow
  const motes = Array.from({ length: 9 }, () =>
    `<circle cx="${n1(1700 + rnd() * 2000)}" cy="${n1(200 + rnd() * 1500)}" r="${n1(10 + rnd() * 14)}" fill="url(#mote)"/>`);
  return `<!doctype html><html><head><style>
    html,body{margin:0}
    #frame{position:relative;width:${WALL_W}px;height:${WALL_H}px;overflow:hidden;background:linear-gradient(180deg,${sky[0]},${sky[1]})}
    #frame>*{position:absolute;inset:0;width:100%;height:100%}
  </style></head><body><div id="frame"><img src="${svgData(smoke)}">
    <svg viewBox="0 0 ${WALL_W} ${WALL_H}"><defs><radialGradient id="mote"><stop offset="0" stop-color="#ffffff"/>
      <stop offset="0.35" stop-color="${dark ? "#f0d4ff" : SHADES[7]}" stop-opacity="0.8"/><stop offset="1" stop-color="${SHADES[7]}" stop-opacity="0"/></radialGradient></defs>
      ${dust.join("")}${motes.join("")}</svg></div></body></html>`;
}

const tpl = (key) => `{{ ${key} }}`;
const templateHeader = (what) => `<?xml version="1.0" encoding="UTF-8"?>
<!--
  ${what}
  Drawn by branding/generate.mjs, which is where to change it; amethystora-theme fills in the colours.
-->
`;

// Hills fading back into a haze under a sun in the accent colour. Every ridge is the background colour
// with the accent laid over it, less of it the nearer the ridge, so the nearest are the palest in a light
// theme and the darkest in a dark one.
function ridgesTemplate() {
  const rnd = random(101);
  const ridges = [[1180, 150, 0.5, 0.16], [1370, 165, 0.36, 0.11], [1560, 175, 0.24, 0.07], [1760, 165, 0.14, 0.04], [1960, 140, 0.06, 0]]
    .map(([base, amp, accent, tint]) => {
      const waves = [[1, 1.2, 1], [2.2, 3.8, 0.5], [5, 8, 0.22], [11, 17, 0.08]].map(([lo, hi, a]) => [lo + (hi - lo) * rnd(), a, rnd() * 2 * Math.PI]);
      const line = Array.from({ length: 161 }, (_, k) => {
        const x = (k / 160) * WALL_W;
        return [x, base - amp * waves.reduce((sum, [f, a, phase]) => sum + a * Math.sin((2 * Math.PI * f * x) / WALL_W + phase), 0)];
      });
      const d = polyPath([[0, WALL_H], ...line, [WALL_W, WALL_H]]);
      return [
        `<path d="${d}" fill="${tpl("background")}"/>`,
        `<path d="${d}" fill="${tpl("accent")}" fill-opacity="${accent}"/>`,
        ...(tint ? [`<path d="${d}" fill="${tpl("color5")}" fill-opacity="${tint}"/>`] : []),
      ].join("\n  ");
    });
  return templateHeader("Hills fading into a haze under a sun in the theme's accent colour.") + wallSvg(
    `<linearGradient id="haze" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="${tpl("accent")}" stop-opacity="0"/><stop offset="0.55" stop-color="${tpl("accent")}" stop-opacity="0.32"/>
    </linearGradient>
    <radialGradient id="halo" cx="2760" cy="880" r="900" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="${tpl("color5")}" stop-opacity="0.45"/><stop offset="1" stop-color="${tpl("color5")}" stop-opacity="0"/>
    </radialGradient>`,
    [
      fillAll(tpl("background")),
      fillAll("url(#haze)"),
      fillAll("url(#halo)"),
      `<circle cx="2760" cy="880" r="220" fill="${tpl("accent")}" fill-opacity="0.9"/>`,
      ...ridges,
    ].join("\n  "),
  );
}

// The lines where a height field sampled on a grid crosses a level (marching squares), joined up into
// polylines: open where they run off the grid, closed where they ring a hill
function isolines(field, nx, ny, cell, level) {
  const points = new Map();
  const ends = new Map();
  const segments = [];
  // An edge of the grid, by its first corner and whether it runs down or across, and where on it the level is
  const edge = (i, j, down) => {
    const key = (j * nx + i) * 2 + (down ? 1 : 0);
    if (!points.has(key)) {
      const [a, b] = [field[j * nx + i], field[(j + (down ? 1 : 0)) * nx + i + (down ? 0 : 1)]];
      const s = (level - a) / (b - a);
      points.set(key, down ? [i * cell, (j + s) * cell] : [(i + s) * cell, j * cell]);
    }
    return key;
  };
  const link = (p, q) => {
    const k = segments.push([p, q]) - 1;
    for (const e of [p, q]) ends.set(e, [...(ends.get(e) || []), k]);
  };
  for (let j = 0; j < ny - 1; j++) {
    for (let i = 0; i < nx - 1; i++) {
      const v = [field[j * nx + i], field[j * nx + i + 1], field[(j + 1) * nx + i + 1], field[(j + 1) * nx + i]];
      const code = (v[0] > level ? 8 : 0) | (v[1] > level ? 4 : 0) | (v[2] > level ? 2 : 0) | (v[3] > level ? 1 : 0);
      const [top, right, bottom, left] = [() => edge(i, j, false), () => edge(i + 1, j, true), () => edge(i, j + 1, false), () => edge(i, j, true)];
      // Two opposite corners above the level: the middle of the cell decides which way the lines turn
      const middle = (v[0] + v[1] + v[2] + v[3]) / 4 > level;
      switch (code) {
        case 1: case 14: link(left(), bottom()); break;
        case 2: case 13: link(bottom(), right()); break;
        case 3: case 12: link(left(), right()); break;
        case 4: case 11: link(top(), right()); break;
        case 6: case 9: link(top(), bottom()); break;
        case 7: case 8: link(left(), top()); break;
        case 5: middle ? (link(left(), top()), link(bottom(), right())) : (link(top(), right()), link(left(), bottom())); break;
        case 10: middle ? (link(top(), right()), link(left(), bottom())) : (link(left(), top()), link(bottom(), right())); break;
      }
    }
  }
  const used = new Uint8Array(segments.length);
  const lines = [];
  for (let s = 0; s < segments.length; s++) {
    if (used[s]) continue;
    used[s] = 1;
    const chain = [...segments[s]];
    for (const forward of [true, false]) {
      for (;;) {
        const end = forward ? chain[chain.length - 1] : chain[0];
        const k = (ends.get(end) || []).find((n) => !used[n]);
        if (k === undefined) break;
        used[k] = 1;
        const other = segments[k][0] === end ? segments[k][1] : segments[k][0];
        forward ? chain.push(other) : chain.unshift(other);
      }
    }
    const closed = chain.length > 3 && chain[0] === chain[chain.length - 1];
    lines.push({ pts: (closed ? chain.slice(1) : chain).map((e) => points.get(e)), closed });
  }
  return lines;
}

// Drops the points of a polyline that it passes within tolerance of anyway (Ramer-Douglas-Peucker)
function simplify(pts, tolerance = 0.8) {
  if (pts.length < 3) return pts;
  const keep = new Uint8Array(pts.length);
  keep[0] = keep[pts.length - 1] = 1;
  const stack = [[0, pts.length - 1]];
  while (stack.length) {
    const [a, b] = stack.pop();
    const [[ax, ay], [bx, by]] = [pts[a], pts[b]];
    const len = Math.hypot(bx - ax, by - ay) || 1;
    let [far, index] = [0, -1];
    for (let i = a + 1; i < b; i++) {
      const d = Math.abs((bx - ax) * (ay - pts[i][1]) - (ax - pts[i][0]) * (by - ay)) / len;
      if (d > far) [far, index] = [d, i];
    }
    if (far > tolerance) {
      keep[index] = 1;
      stack.push([a, index], [index, b]);
    }
  }
  return pts.filter((_, i) => keep[i]);
}

// A topographic map: the contour lines of a few hills and hollows, every fifth one heavier, as survey maps
// draw them, over two soft glows.
function contoursTemplate() {
  const rnd = random(131);
  const cell = 16;
  const [nx, ny] = [WALL_W / cell + 1, WALL_H / cell + 1];
  const hills = Array.from({ length: 7 }, () => [rnd() * WALL_W, rnd() * WALL_H, 380 + rnd() * 520, (rnd() < 0.3 ? -0.7 : 1) * (0.6 + rnd() * 0.6)]);
  const height = (x, y) =>
    hills.reduce((h, [hx, hy, s, a]) => h + a * Math.exp(-((x - hx) ** 2 + (y - hy) ** 2) / (2 * s * s)), 0) + 0.25 * Math.sin(x / 700 + y / 1100);
  const field = new Float64Array(nx * ny);
  for (let j = 0; j < ny; j++) for (let i = 0; i < nx; i++) field[j * nx + i] = height(i * cell, j * cell);
  // Levels at equal shares of the screen rather than equal heights, so the lines spread across all of it
  // instead of crowding on the few steep hills and leaving the low ground bare
  const sorted = Float64Array.from(field).sort();
  const LEVELS = 40;
  const [minor, major] = [[], []];
  for (let k = 1; k < LEVELS; k++) {
    const level = sorted[Math.floor((sorted.length * k) / LEVELS)];
    const d = isolines(field, nx, ny, cell, level).map(({ pts, closed }) => polyPath(simplify(pts), closed)).join("");
    (k % 5 ? minor : major).push(d);
  }
  const lines = (paths, opacity, width) =>
    `<path d="${paths.join("")}" fill="none" stroke="${tpl("accent")}" stroke-opacity="${opacity}" stroke-width="${width}" stroke-linejoin="round" stroke-linecap="round"/>`;
  return templateHeader("Contour lines of a few hills and hollows, every fifth one heavier, in the theme's accent colour.") + wallSvg(
    `<radialGradient id="glowA" cx="2700" cy="800" r="1800" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="${tpl("accent")}" stop-opacity="0.18"/><stop offset="1" stop-color="${tpl("accent")}" stop-opacity="0"/>
    </radialGradient>
    <radialGradient id="glowB" cx="500" cy="1800" r="1400" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="${tpl("color6")}" stop-opacity="0.12"/><stop offset="1" stop-color="${tpl("color6")}" stop-opacity="0"/>
    </radialGradient>`,
    [fillAll(tpl("background")), fillAll("url(#glowA)"), fillAll("url(#glowB)"), lines(minor, 0.34, 2.5), lines(major, 0.62, 4)].join("\n  "),
  );
}

// Translucent ribbons rising across the screen from the lower left, blending where they cross, each in a
// gradient between two of the theme's colours
function wavesTemplate() {
  const rnd = random(151);
  const ribbons = Array.from({ length: 7 }, (_, k) => {
    const y0 = 700 + k * 190 + (rnd() - 0.5) * 120;
    const slope = -0.28 + (rnd() - 0.5) * 0.1;
    const waves = [[0.6 + rnd() * 0.4, 180 + rnd() * 120, rnd() * 2 * Math.PI], [1.4 + rnd(), 60 + rnd() * 60, rnd() * 2 * Math.PI]];
    const [thick, beat, phase] = [90 + rnd() * 160, 0.5 + rnd(), rnd() * 2 * Math.PI];
    const centre = (x) => y0 + slope * (x - WALL_W / 2) + waves.reduce((sum, [f, a, p]) => sum + a * Math.sin((2 * Math.PI * f * x) / WALL_W + p), 0);
    const half = (x) => thick * (0.55 + 0.45 * Math.sin((2 * Math.PI * beat * x) / WALL_W + phase));
    const xs = Array.from({ length: 121 }, (_, i) => -100 + (i / 120) * (WALL_W + 200));
    const top = xs.map((x) => [x, centre(x) - half(x)]);
    return [
      `<path d="${polyPath([...top, ...xs.map((x) => [x, centre(x) + half(x)]).reverse()])}" fill="url(#ribbon${k % 3})" fill-opacity="${(0.14 + 0.2 * rnd()).toFixed(2)}"/>`,
      `<path d="${polyPath(top, false)}" fill="none" stroke="${tpl("foreground")}" stroke-opacity="0.12" stroke-width="2"/>`,
    ].join("\n  ");
  });
  const ribbon = (id, from, to) => `<linearGradient id="${id}" x1="0" y1="0" x2="${WALL_W}" y2="0" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="${tpl(from)}"/><stop offset="1" stop-color="${tpl(to)}"/>
    </linearGradient>`;
  return templateHeader("Translucent ribbons in the theme's accent and its magenta and cyan, rising from the lower left.") + wallSvg(
    [
      ribbon("ribbon0", "accent", "color5"),
      ribbon("ribbon1", "color5", "color6"),
      ribbon("ribbon2", "color4", "accent"),
      `<radialGradient id="glow" cx="2900" cy="700" r="1900" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="${tpl("accent")}" stop-opacity="0.16"/><stop offset="1" stop-color="${tpl("accent")}" stop-opacity="0"/>
    </radialGradient>`,
    ].join("\n    "),
    [fillAll(tpl("background")), fillAll("url(#glow)"), ...ribbons].join("\n  "),
  );
}

// ------------------------------------------------------ theme wallpapers --

// Every theme has three pictures of places that do not exist, drawn here as flat shapes and glows, every
// colour taken from the theme's own colors.toml, so a changed palette only needs this script run again.
// They are rendered to PNG, like the crystal field, and each theme's backgrounds.list names them; the
// Amethystora themes open on the cut stone's facets instead.

const palette = (theme) => Object.fromEntries(
  [...readFileSync(join(SHARED, "usr/share/amethystora/themes", theme, "colors.toml"), "utf8").matchAll(/^(\w+) = "(#[0-9a-fA-F]{6})"/gm)]
    .map(([, key, hex]) => [key, hex.toLowerCase()]));

// A gradient through colours given as "#rrggbb" or ["#rrggbb", opacity], evenly spaced across what it fills
const linear = (id, colors, [x1, y1, x2, y2] = [0, 0, 0, 1]) =>
  `<linearGradient id="${id}" x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}">${colors.map((c, i) => {
    const [color, opacity = 1] = [].concat(c);
    return `<stop offset="${(i / (colors.length - 1)).toFixed(3)}" stop-color="${color}" stop-opacity="${opacity}"/>`;
  }).join("")}</linearGradient>`;
// A glow around a point, fading to nothing at radius r
const glow = (id, [x, y], r, color, a) =>
  `<radialGradient id="${id}" cx="${n1(x)}" cy="${n1(y)}" r="${n1(r)}" gradientUnits="userSpaceOnUse">${stops(color, color, (o) => a * (1 - o) ** 2)}</radialGradient>`;
// In screen units, so that a straight line, whose box has no height, is blurred too
const blur = (id, sd) =>
  `<filter id="${id}" filterUnits="userSpaceOnUse" x="-400" y="-400" width="${WALL_W + 800}" height="${WALL_H + 800}"><feGaussianBlur stdDeviation="${sd}"/></filter>`;
// Daylight keeps a share of them (keep), each as much fainter, and draws every one of them all the same so that the rest
// of the scene takes the same random numbers and stands where it stands at night
const starfield = (rnd, count, color, bottom, keep = 1) => `<g fill="${color}">${Array.from({ length: count }, (_, k) => {
  const star = `<circle cx="${n1(rnd() * WALL_W)}" cy="${n1(rnd() ** 1.3 * bottom)}" r="${n1(1.2 + 3.4 * rnd() ** 4)}" fill-opacity="${((0.25 + 0.65 * rnd()) * keep).toFixed(2)}"/>`;
  return k < Math.round(count * keep) ? star : "";
}).join("")}</g>`;
// Everything under a line that crosses the screen
const belowPath = (pts) => polyPath([[pts[0][0], WALL_H + 10], ...pts, [pts[pts.length - 1][0], WALL_H + 10]]);
const below = (pts, fill, extra = "") => `<path d="${belowPath(pts)}" fill="${fill}"${extra}/>`;
// A sun or moon with its glow
const orb = (id, [x, y, r, color], reach = 6, a = 0.55) => ({
  def: glow(id, [x, y], r * reach, color, a),
  body: `${fillAll(`url(#${id})`)}<circle cx="${x}" cy="${y}" r="${r}" fill="${color}"/>`,
});
// Haze thickening downward over a band of the screen, drawn with the scene's "mist" gradient
const haze = (top, bottom, a) => `<rect y="${n1(top)}" width="${WALL_W}" height="${n1(bottom - top)}" fill="url(#mist)" fill-opacity="${a}"/>`;

// A line of peaks across the screen by midpoint displacement from eight random heights, so that the peaks
// spread across it, jagged when rough is near 1 and rolling when it is low, standing on base and reaching
// up to base - height
function ridge(rnd, base, height, rough = 0.55, levels = 8) {
  const n = 2 ** levels;
  const ys = new Float64Array(n + 1);
  for (let i = 0; i <= n; i += n / 8) ys[i] = rnd();
  for (let step = n / 8, amp = 0.6; step > 1; step /= 2, amp *= rough)
    for (let i = step / 2; i < n; i += step) ys[i] = (ys[i - step / 2] + ys[i + step / 2]) / 2 + (rnd() - 0.5) * amp;
  const [lo, hi] = [Math.min(...ys), Math.max(...ys)];
  return Array.from(ys, (v, i) => [-20 + (i / n) * (WALL_W + 40), base - (height * (v - lo)) / (hi - lo)]);
}

// A smooth line across the screen, a few sine waves around base
function swell(rnd, base, amp, waves = [[0.5, 1.2, 1], [1.5, 3, 0.4], [3.5, 6, 0.12]]) {
  const w = waves.map(([lo, hi, a]) => [lo + (hi - lo) * rnd(), a, rnd() * 2 * Math.PI]);
  return Array.from({ length: 161 }, (_, k) => {
    const x = -20 + (k / 160) * (WALL_W + 40);
    return [x, base - amp * w.reduce((sum, [f, a, p]) => sum + a * Math.sin((2 * Math.PI * f * x) / WALL_W + p), 0)];
  });
}

// Mirrors a scene in still water below the horizon, tinted and broken by a few ripples
function mirror(rnd, scene, horizon, water, ripple, defs) {
  defs.push(`<clipPath id="water"><rect y="${horizon}" width="${WALL_W}" height="${WALL_H - horizon}"/></clipPath>`,
    linear("water", [[water, 0.4], [water, 0.92]]));
  const ripples = Array.from({ length: 80 }, () => {
    const y = horizon + 8 + rnd() ** 1.6 * (WALL_H - horizon);
    return `<rect x="${n1(rnd() * WALL_W)}" y="${n1(y)}" width="${n1(60 + rnd() * 420)}" height="${n1(2 + (y - horizon) / 110)}" rx="2"/>`;
  });
  return `<g clip-path="url(#water)"><g transform="matrix(1 0 0 -1 0 ${2 * horizon})">${scene}</g>
  <rect y="${horizon}" width="${WALL_W}" height="${WALL_H - horizon}" fill="url(#water)"/>
  <g fill="${ripple}" fill-opacity="0.16">${ripples.join("")}</g></g>`;
}

// Curtains of light hanging from wavy lines, bright at the foot and fading upward, streaked with rays
function aurora(rnd, colors, defs) {
  const rays = Array.from({ length: 320 }, () =>
    `<rect x="${n1(rnd() * WALL_W)}" width="${n1(8 + rnd() * 40)}" height="${WALL_H}" fill="#ffffff" fill-opacity="${(0.3 + 0.7 * rnd()).toFixed(2)}"/>`);
  defs.push(blur("aurora", 22), `<mask id="rays"><rect width="${WALL_W}" height="${WALL_H}" fill="#ffffff" fill-opacity="0.35"/>${rays.join("")}</mask>`);
  return colors.map((color, k) => {
    defs.push(linear(`curtain${k}`, [[color, 0], [color, 0.12], [color, 0.8]]));
    const foot = swell(rnd, 760 + k * 130, 240 + rnd() * 120);
    const top = foot.map(([x, y]) => [x, y - 380 - 260 * Math.sin((2 * Math.PI * x * (1 + 0.4 * k)) / WALL_W + k)]);
    return `<g filter="url(#aurora)"><path d="${polyPath([...top, ...foot.slice().reverse()])}" fill="url(#curtain${k})" mask="url(#rays)"/>
    <path d="${polyPath(foot, false)}" fill="none" stroke="${color}" stroke-opacity="0.8" stroke-width="12"/></g>`;
  }).join("\n  ");
}

// Ranges of peaks one behind another, each in its own colour with haze between them, under a sky with a
// sun or moon; if asked, stars, an aurora, snow on the peaks, a lake mirroring it all or petals on the wind.
// A range is [base, height, roughness, colour, snow colour].
function peaksSvg(o) {
  const rnd = random(o.seed);
  const horizon = o.lake || WALL_H;
  const defs = [linear("sky", o.sky), linear("mist", [[o.mist, 0], [o.mist, 1]]), blur("soft", 14)];
  const sky = [fillAll("url(#sky)")];
  const light = { ...DAYLIGHT_SCENE.night, ...o.daylight };
  if (o.stars) sky.push(starfield(rnd, o.stars, o.starColor, horizon * 0.8, light.stars));
  if (o.aurora) {
    const curtains = aurora(rnd, o.aurora, defs);
    if (light.aurora > 0) sky.push(light.aurora === 1 ? curtains : `<g opacity="${light.aurora}">${curtains}</g>`);
  }
  if (o.sun) {
    const sun = orb("sunglow", o.sun);
    defs.push(sun.def);
    sky.push(sun.body);
  }
  const land = o.ranges.map(([base, height, rough, color, snow], k) => {
    const pts = ridge(rnd, base, height, rough);
    const shape = [below(pts, color)];
    if (snow) {
      // What of the range stands above a ragged snowline
      defs.push(`<clipPath id="range${k}"><path d="${belowPath(pts)}"/></clipPath>`);
      const line = Array.from({ length: 97 }, (_, i) => [-20 + (i / 96) * (WALL_W + 40), base - height * (0.64 + 0.12 * (rnd() - 0.5))]);
      shape.push(`<path d="${polyPath([[-20, -10], [WALL_W + 20, -10], ...line.reverse()])}" fill="${snow}" clip-path="url(#range${k})"/>`);
    }
    if (k < o.ranges.length - 1) shape.push(haze(base - height * 0.7, base + 600, o.haze));
    return shape.join("");
  });
  const scene = [...sky, ...land].join("\n  ");
  const body = [scene];
  if (o.lake) body.push(mirror(rnd, scene, horizon, o.water, o.ripple, defs));
  if (o.petals) {
    body.push(Array.from({ length: 110 }, (_, k) => {
      const near = k % 5 === 0;
      const [x, y, s] = [rnd() * WALL_W, rnd() * WALL_H, (near ? 30 : 9) + (near ? 24 : 14) * rnd()];
      return `<ellipse cx="${n1(x)}" cy="${n1(y)}" rx="${n1(s)}" ry="${n1(s * 0.55)}" transform="rotate(${Math.round(rnd() * 180)} ${n1(x)} ${n1(y)})"`
        + ` fill="${o.petals[Math.floor(rnd() * o.petals.length)]}" fill-opacity="${(0.5 + 0.4 * rnd()).toFixed(2)}"${near ? ' filter="url(#soft)"' : ""}/>`;
    }).join(""));
  }
  return wallSvg(defs.join("\n    "), body.join("\n  "));
}

// A pine, as a stack of tiers widening toward its foot at (x, y)
function pine(x, y, h) {
  const [tiers, w] = [5, h * 0.21];
  const right = [];
  for (let k = 1; k <= tiers; k++) {
    const [ty, tw] = [y - h + (h * 0.88 * k) / tiers, w * (0.35 + (0.65 * k) / tiers)];
    right.push([x + tw, ty], [x + tw * 0.4, ty - h * 0.025]);
  }
  right.pop();
  right.push([x + w * 0.08, y - h * 0.12], [x + w * 0.08, y]);
  return polyPath([[x, y - h], ...right, ...right.map(([px, py]) => [2 * x - px, py]).reverse()]);
}

// Rows of pines on rolling ground, each farther into the mist than the one in front of it, below far hills
// and a sun or moon. A row is [base, tree height, colour]; a hill is [base, height, roughness, colour].
function forestSvg(o) {
  const rnd = random(o.seed);
  const defs = [linear("sky", o.sky), linear("mist", [[o.mist, 0], [o.mist, 1]])];
  const body = [fillAll("url(#sky)")];
  if (o.stars) body.push(starfield(rnd, o.stars, o.starColor, 1000, { ...DAYLIGHT_SCENE.night, ...o.daylight }.stars));
  if (o.sun) {
    const sun = orb("sunglow", o.sun);
    defs.push(sun.def);
    body.push(sun.body);
  }
  for (const [base, height, rough, color] of o.hills) body.push(below(ridge(rnd, base, height, rough), color), haze(base - height, base + 400, o.haze));
  o.rows.forEach(([base, tall, color], k) => {
    const ground = swell(rnd, base, 30 + tall * 0.12);
    const at = (x) => ground[Math.max(0, Math.min(160, Math.round(((x + 20) / (WALL_W + 40)) * 160)))][1];
    const trees = [];
    for (let x = -60 + rnd() * 80; x < WALL_W + 60; x += tall * (0.14 + 0.26 * rnd())) trees.push(pine(x, at(x) + 30, tall * (0.5 + 0.55 * rnd())));
    body.push(`<g fill="${color}">${below(ground, color)}<path d="${trees.join("")}"/></g>`);
    if (k < o.rows.length - 1) body.push(haze(base - tall * 1.1, base + 500, o.haze));
  });
  return wallSvg(defs.join("\n    "), body.join("\n  "));
}

// Sheets of paper cut into flowing curves and stacked, each casting a soft shadow on the one behind it,
// the colours running down the list of bands, each [from, to] across the screen
function dunesSvg(o) {
  const rnd = random(o.seed);
  const defs = [`<filter id="shade" filterUnits="userSpaceOnUse" x="-100" y="-400" width="${WALL_W + 200}" height="${WALL_H + 800}">`
    + `<feDropShadow dx="0" dy="-16" stdDeviation="30" flood-color="${o.shadow}" flood-opacity="${o.shadowOpacity}"/></filter>`];
  const body = [fillAll(o.bg)];
  if (o.sun) {
    const sun = orb("sunglow", o.sun, 5, 0.4);
    defs.push(sun.def);
    body.push(sun.body);
  }
  o.bands.forEach(([from, to], k) => {
    defs.push(linear(`band${k}`, [from, to], [0, 0, 1, 0.3]));
    const base = o.top + ((WALL_H + 120 - o.top) * k) / o.bands.length;
    const pts = swell(rnd, base, o.amp * (0.7 + 0.6 * rnd())).map(([x, y]) => [x, y + o.tilt * (x - WALL_W / 2)]);
    body.push(below(pts, `url(#band${k})`, ' filter="url(#shade)"'));
  });
  return wallSvg(defs.join("\n    "), body.join("\n  "));
}

// A striped sun going down behind low mountains or mesas, over a glowing grid or dunes of sand.
// A range is [height, roughness, colour, rim colour]; the sun is [x, y, radius] with its colours top down.
function retroSvg(o) {
  const rnd = random(o.seed);
  const h = o.horizon;
  const [sx, sy, r] = o.sun;
  const defs = [linear("sky", o.sky), linear("sun", o.sunColors), glow("sunglow", [sx, sy], r * 3, o.sunColors[1], 0.5), blur("neon", 10),
    linear("ground", o.ground)];
  // Bars cut across the lower half of the sun, thickening toward the horizon
  const bars = Array.from({ length: 9 }, (_, k) =>
    `<rect x="${sx - r - 10}" y="${n1(sy + r * (0.02 + 0.105 * k))}" width="${2 * r + 20}" height="${n1(4 + 4.2 * k)}" fill="#000000"/>`);
  defs.push(`<mask id="bars"><rect width="${WALL_W}" height="${WALL_H}" fill="#ffffff"/>${bars.join("")}</mask>`);
  const body = [fillAll("url(#sky)")];
  if (o.stars) body.push(starfield(rnd, o.stars, o.starColor, h * 0.7, { ...DAYLIGHT_SCENE.night, ...o.daylight }.stars));
  body.push(fillAll("url(#sunglow)"), `<circle cx="${sx}" cy="${sy}" r="${r}" fill="url(#sun)" mask="url(#bars)"/>`);
  for (const [height, rough, color, rim] of o.ranges) {
    let pts;
    if (o.mesas) {
      // Flat-topped buttes with steep sides, in a broken line
      pts = [[-20, h]];
      for (let x = -20 - rnd() * 300; x < WALL_W + 20; ) {
        const [w, top, gap] = [260 + rnd() * 700, h - height * (0.35 + 0.65 * rnd()), rnd() < 0.4 ? 60 + rnd() * 400 : 0];
        pts.push([x, h], [x + 50 + rnd() * 60, top], [x + w - 50 - rnd() * 60, top + (rnd() - 0.5) * 30], [x + w, h]);
        x += w + gap;
      }
      pts.push([WALL_W + 20, h]);
    } else {
      pts = ridge(rnd, h, height, rough, 5);
    }
    body.push(below(pts, color));
    if (rim) body.push(`<path d="${polyPath(pts, false)}" fill="none" stroke="${rim}" stroke-width="10" filter="url(#neon)"/>`,
      `<path d="${polyPath(pts, false)}" fill="none" stroke="${rim}" stroke-width="3" stroke-linejoin="round"/>`);
  }
  body.push(`<rect y="${h}" width="${WALL_W}" height="${WALL_H - h}" fill="url(#ground)"/>`);
  if (o.grid) {
    const lines = [];
    for (let i = -40; i <= 40; i++) lines.push(`M${WALL_W / 2 + i * 60},${h}L${WALL_W / 2 + i * 520},${WALL_H + 10}`);
    for (let k = 1; k <= 16; k++) lines.push(`M0,${n1(h + (WALL_H - h) * (k / 16) ** 2.2)}H${WALL_W}`);
    const d = lines.join("");
    body.push(`<path d="${d}" stroke="${o.grid}" stroke-width="10" fill="none" filter="url(#neon)"/>`, `<path d="${d}" stroke="${o.grid}" stroke-width="3" fill="none"/>`,
      // the grid fades into the haze at the horizon
      `<rect y="${h}" width="${WALL_W}" height="260" fill="url(#fade)"/>`);
    defs.push(linear("fade", [o.ground[0], [o.ground[0], 0]]));
  }
  if (o.sand) o.sand.forEach((color, k) => body.push(below(swell(rnd, h + 120 + k * 190, 50 + k * 30), color)));
  return wallSvg(defs.join("\n    "), body.join("\n  "));
}

// A city at night: rows of towers, the nearer with lit windows and the nearest with neon signs, under a big
// moon, above a canal that mirrors it. A row is [tallest, colour, share of windows lit].
function skylineSvg(o) {
  const rnd = random(o.seed);
  const ground = o.water;
  const defs = [linear("sky", o.sky), linear("mist", [[o.mist, 0], [o.mist, 1]]), blur("neon", 14)];
  const moon = orb("moonglow", o.moon, 5, 0.45);
  defs.push(moon.def);
  const body = [fillAll("url(#sky)"), starfield(rnd, 260, o.starColor, 1100, { ...DAYLIGHT_SCENE.night, ...o.daylight }.stars), moon.body];
  o.rows.forEach(([tallest, color, lit], row) => {
    const [towers, windows, neon] = [[], o.windows.map(() => []), []];
    for (let x = -60 - rnd() * 100; x < WALL_W + 60; ) {
      const w = 90 + rnd() * 190;
      const height = tallest * (rnd() < 0.12 ? 0.9 + 0.1 * rnd() : 0.3 + 0.55 * rnd() ** 1.3);
      const top = ground - height;
      towers.push(`M${n1(x)},${ground + 10}V${n1(top)}H${n1(x + w)}V${ground + 10}Z`);
      if (rnd() < 0.3) towers.push(`M${n1(x + w * 0.2)},${n1(top + 2)}V${n1(top - 40 - rnd() * 60)}H${n1(x + w * 0.8)}V${n1(top + 2)}Z`);
      if (rnd() < 0.25) towers.push(`M${n1(x + w / 2 - 3)},${n1(top)}v-${n1(80 + rnd() * 160)}h6V${n1(top)}Z`);
      if (lit) {
        for (let wy = top + 30; wy < ground - 40; wy += 38)
          for (let wx = x + 16; wx < x + w - 28; wx += 28)
            if (rnd() < lit) windows[Math.floor(rnd() * o.windows.length)].push(`M${n1(wx)},${n1(wy)}h12v18h-12Z`);
      }
      if (row === o.rows.length - 1 && rnd() < 0.22) {
        const [nx, ny, nh] = [x + w * (0.15 + 0.5 * rnd()), top + 60 + rnd() * height * 0.4, 160 + rnd() * 220];
        neon.push(`<rect x="${n1(nx)}" y="${n1(ny)}" width="34" height="${n1(nh)}" rx="10" fill="none" stroke="${o.neon[Math.floor(rnd() * o.neon.length)]}" stroke-width="6"/>`);
      }
      x += w + (rnd() < 0.3 ? rnd() * 50 : -rnd() * 30);
    }
    body.push(`<path d="${towers.join("")}" fill="${color}"/>`,
      ...windows.map((d, k) => (d.length ? `<path d="${d.join("")}" fill="${o.windows[k]}" fill-opacity="${row === o.rows.length - 1 ? 0.7 : 0.45}"/>` : "")),
      ...(neon.length ? [`<g filter="url(#neon)">${neon.join("")}</g>`, neon.join("")] : []));
    if (row < o.rows.length - 1) body.push(haze(ground - tallest, ground, o.haze));
  });
  if (o.rain) {
    body.push(`<g stroke="${o.rain}" stroke-width="2" stroke-opacity="0.18">${Array.from({ length: 420 }, () => {
      const [x, y, l] = [rnd() * (WALL_W + 400), rnd() * WALL_H, 40 + rnd() * 90];
      return `<path d="M${n1(x)},${n1(y)}l${n1(-l * 0.25)},${n1(l)}"/>`;
    }).join("")}</g>`);
  }
  const scene = body.join("\n  ");
  const water = mirror(rnd, scene, ground, o.waterColor, o.ripple, defs);
  return wallSvg(defs.join("\n    "), scene + "\n  " + water);
}

// Swells rolling in under a low moon or sun whose light lies across them, a far coast on the horizon and,
// if asked, icebergs adrift. A berg is [x, waterline, width, height].
function seaSvg(o) {
  const rnd = random(o.seed);
  const h = o.horizon;
  const [mx, , mr, light] = o.moon;
  const moon = orb("moonglow", o.moon, 6, 0.5);
  const defs = [linear("sky", o.sky), moon.def];
  const body = [fillAll("url(#sky)")];
  if (o.stars) body.push(starfield(rnd, o.stars, o.starColor, h * 0.8));
  body.push(moon.body);
  if (o.coast) body.push(`<path d="${belowPath(ridge(rnd, h + 2, 120, 0.5))}" fill="${o.coast}"/>`);
  body.push(`<rect y="${h}" width="${WALL_W}" height="${WALL_H - h}" fill="${o.sea[0]}"/>`);
  const bergs = [...(o.bergs || [])].sort((a, b) => a[1] - b[1]);
  const berg = ([x, y, w, tall]) => {
    const peaks = Array.from({ length: 5 }, (_, i) => [x - w / 2 + (w * (i + 0.5 + (rnd() - 0.5) * 0.6)) / 5, y - tall * (0.35 + 0.65 * rnd())]);
    const top = peaks.reduce((a, b) => (b[1] < a[1] ? b : a));
    const outline = [[x - w / 2, y], ...peaks, [x + w / 2, y]];
    // the face toward the light, left of the highest peak
    const lit = [[x - w / 2, y], ...peaks.filter((p) => p[0] < top[0]), top, [top[0] - w * 0.08, y]];
    return `<path d="${polyPath(outline)}" fill="${o.ice[1]}"/><path d="${polyPath(lit)}" fill="${o.ice[0]}"/>`
      + `<path d="${polyPath(outline.map(([px, py]) => [px, y + (y - py) * 0.45]))}" fill="${o.ice[0]}" fill-opacity="0.12"/>`;
  };
  const n = o.swells;
  for (let k = 0; k < n; k++) {
    const t = (k + 1) / n;
    const base = h + (WALL_H + 40 - h) * t ** 1.5;
    while (bergs.length && bergs[0][1] < base) body.push(berg(bergs.shift()));
    const [amp, crests, phase] = [6 + 70 * t ** 1.6, 26 - 20 * t, rnd() * Math.PI];
    const pts = Array.from({ length: 321 }, (_, i) => {
      const x = -20 + (i / 320) * (WALL_W + 40);
      return [x, base - amp * (1 - Math.abs(Math.sin((Math.PI * crests * x) / WALL_W + phase))) ** 2];
    });
    body.push(below(pts, mix(o.sea[0], o.sea[1], t)), `<path d="${polyPath(pts, false)}" fill="none" stroke="${o.foam}" stroke-opacity="${(0.12 + 0.25 * t).toFixed(2)}" stroke-width="${n1(1.5 + 3 * t)}"/>`);
  }
  // the light on the water, in broken strokes widening toward the shore
  body.push(`<g fill="${light}">${Array.from({ length: 90 }, (_, k) => {
    const t = k / 90;
    const [y, w] = [h + 6 + (WALL_H - h) * t ** 1.5, mr * (0.4 + 2.6 * t) * (0.4 + rnd())];
    return `<rect x="${n1(mx - w / 2 + (rnd() - 0.5) * mr * (0.5 + 2.5 * t))}" y="${n1(y)}" width="${n1(w)}" height="${n1(3 + 9 * t)}" rx="3" fill-opacity="${(0.55 * (1 - t) + 0.08).toFixed(2)}"/>`;
  }).join("")}</g>`);
  return wallSvg(defs.join("\n    "), body.join("\n  "));
}

// Soft clouds of colour with ribbons of fine lines twisting through them like silk. A ribbon is
// [from, to, centre line, width, twists].
function silkSvg(o) {
  const rnd = random(o.seed);
  const defs = [blur("cloud", 260)];
  const clouds = o.clouds.map((color) =>
    `<ellipse cx="${n1(rnd() * WALL_W)}" cy="${n1(rnd() * WALL_H)}" rx="${n1(700 + rnd() * 900)}" ry="${n1(400 + rnd() * 500)}" fill="${color}" fill-opacity="${o.cloudOpacity}"/>`);
  const body = [fillAll(o.bg), `<g filter="url(#cloud)">${clouds.join("")}</g>`];
  o.ribbons.forEach(([from, to, y0, width, twists], r) => {
    defs.push(linear(`silk${r}`, [from, to], [0, 0, 1, 0]));
    const [p1, p2, amp, slope] = [rnd() * 2 * Math.PI, rnd() * 2 * Math.PI, 220 + rnd() * 200, -0.18 + 0.1 * rnd()];
    const N = 64;
    for (let k = 0; k < N; k++) {
      const t = k / (N - 1) - 0.5;
      const pts = Array.from({ length: 161 }, (_, i) => {
        const x = -40 + (i / 160) * (WALL_W + 80);
        const u = (2 * Math.PI * x) / WALL_W;
        return [x, y0 + slope * (x - WALL_W / 2) + amp * Math.sin(0.8 * u + p1) + t * width * Math.sin(twists * u + p2) + t * t * 120];
      });
      body.push(`<path d="${polyPath(pts, false)}" fill="none" stroke="url(#silk${r})" stroke-width="2.4" stroke-opacity="${(0.1 + 0.6 * (1 - 2 * Math.abs(t))).toFixed(2)}"/>`);
    }
  });
  return wallSvg(defs.join("\n    "), body.join("\n  "));
}

// A world in space among stars and the glow of a nebula: banded, ringed and lit from the upper left, with a
// small moon; or, as an eclipse, dark against the corona of the star behind it. The planet is [x, y, radius].
function planetSvg(o) {
  const rnd = random(o.seed);
  const [px, py, pr] = o.planet;
  const defs = [linear("sky", o.sky), blur("nebula", 240)];
  const nebula = o.nebula.map((color) =>
    `<ellipse cx="${n1(rnd() * WALL_W)}" cy="${n1(rnd() * WALL_H)}" rx="${n1(600 + rnd() * 900)}" ry="${n1(300 + rnd() * 400)}" fill="${color}" fill-opacity="${o.nebulaOpacity}"/>`);
  const body = [fillAll("url(#sky)"), `<g filter="url(#nebula)">${nebula.join("")}</g>`,
    starfield(rnd, 420, o.starColor, WALL_H, { ...DAYLIGHT_SCENE.night, ...o.daylight }.stars)];
  if (o.corona) {
    defs.push(glow("corona", [px, py], pr * 3.4, o.corona[0], 0.95), glow("inner", [px, py], pr * 1.3, o.corona[1], 1), blur("flare", 12));
    const a = -0.9;
    const [bx, by] = [px + pr * Math.cos(a), py + pr * Math.sin(a)];
    defs.push(glow("bead", [bx, by], 160, o.corona[1], 1));
    body.push(fillAll("url(#corona)"), fillAll("url(#inner)"), `<circle cx="${px}" cy="${py}" r="${pr}" fill="${o.disc}"/>`,
      `<circle cx="${px}" cy="${py}" r="${pr}" fill="none" stroke="${o.corona[1]}" stroke-width="10" filter="url(#flare)"/>`,
      `<circle cx="${px}" cy="${py}" r="${pr}" fill="none" stroke="${o.corona[1]}" stroke-width="2.5"/>`,
      fillAll("url(#bead)"), `<circle cx="${n1(bx)}" cy="${n1(by)}" r="10" fill="#ffffff"/>`);
    return wallSvg(defs.join("\n    "), body.join("\n  "));
  }
  const ring = (half) => `<g transform="translate(${px} ${py}) rotate(-16)" clip-path="url(#${half})" fill="none" stroke="${o.ring}">${[[1.55, 22, 0.35], [1.7, 40, 0.55], [1.85, 10, 0.3], [2.0, 50, 0.4], [2.22, 18, 0.25]]
    .map(([s, w, a]) => `<ellipse rx="${n1(pr * s)}" ry="${n1(pr * s * 0.2)}" stroke-width="${w}" stroke-opacity="${a}"/>`).join("")}</g>`;
  defs.push(`<clipPath id="back"><rect x="-5000" y="-5000" width="10000" height="5000"/></clipPath>`,
    `<clipPath id="front"><rect x="-5000" y="0" width="10000" height="5000"/></clipPath>`,
    `<clipPath id="globe"><circle cx="${px}" cy="${py}" r="${pr}"/></clipPath>`,
    `<radialGradient id="body" cx="0.35" cy="0.3" r="0.8"><stop offset="0" stop-color="${o.colors[0]}"/><stop offset="1" stop-color="${o.colors[1]}"/></radialGradient>`,
    linear("night", [[o.sky[0], 0], [o.sky[0], 0], [o.sky[0], 0.9]], [0, 0, 1, 1]));
  const bands = Array.from({ length: 11 }, (_, k) =>
    `<rect x="${px - pr}" y="${n1(py - pr + (2 * pr * (k + rnd() * 0.5)) / 11)}" width="${2 * pr}" height="${n1(20 + rnd() * 70)}" fill="${o.bands[k % o.bands.length]}" fill-opacity="${(0.15 + 0.2 * rnd()).toFixed(2)}"/>`);
  const [mx, my, mr] = o.moon;
  defs.push(`<radialGradient id="moon" cx="0.35" cy="0.3" r="0.8"><stop offset="0" stop-color="${o.moonColors[0]}"/><stop offset="1" stop-color="${o.moonColors[1]}"/></radialGradient>`);
  body.push(ring("back"), `<circle cx="${px}" cy="${py}" r="${pr}" fill="url(#body)"/>`,
    `<g clip-path="url(#globe)" transform="rotate(-16 ${px} ${py})">${bands.join("")}</g>`,
    `<circle cx="${px}" cy="${py}" r="${pr}" fill="url(#night)"/>`, ring("front"),
    `<circle cx="${mx}" cy="${my}" r="${mr}" fill="url(#moon)"/>`);
  return wallSvg(defs.join("\n    "), body.join("\n  "));
}

// Each theme's three, the first the one it opens on. Colours not in a palette are mixed from those that are.
const THEME_WALLPAPERS = {
  amethystora: (c) => ({
    "starry-peaks": () => peaksSvg({
      seed: 1009, sky: [mix(c.background, "#000000", 0.3), c.background, mix(c.background, c.accent, 0.35), mix(c.color5, c.color1, 0.35)],
      stars: 380, starColor: c.color15, sun: [1080, 560, 110, c.cursor], mist: mix(c.background, c.color5, 0.5), haze: 0.3,
      ranges: [[1500, 820, 0.55, mix(c.background, c.accent, 0.45), mix(c.cursor, c.accent, 0.2)], [1500, 580, 0.55, mix(c.background, c.accent, 0.28)],
        [1500, 380, 0.5, mix(c.background, c.selection_background, 0.4)], [1500, 210, 0.45, mix(c.background, "#000000", 0.3)]],
      lake: 1500, water: c.background, ripple: c.color13,
    }),
    "violet-waves": () => dunesSvg({
      seed: 1013, bg: c.background, top: 380, amp: 180, tilt: 0.09, shadow: "#000000", shadowOpacity: 0.6, sun: [900, 430, 180, c.cursor],
      bands: [[c.color13, c.color5], [c.color5, c.accent], [c.color1, c.color5], [c.accent, c.selection_background], [c.color5, c.color6], [c.selection_background, c.accent]]
        .map(([a, b], k) => [mix(a, c.background, 0.12 + 0.12 * k), mix(b, c.background, 0.12 + 0.12 * k)]),
    }),
    "dusk-sea": () => seaSvg({
      seed: 1019, horizon: 1220, sky: [mix(c.background, "#000000", 0.3), mix(c.background, c.accent, 0.4), mix(c.color1, c.color3, 0.4)], stars: 160, starColor: c.color15,
      moon: [2500, 1000, 160, mix(c.color3, "#ffffff", 0.2)], coast: mix(c.background, c.accent, 0.3),
      sea: [mix(c.background, c.accent, 0.35), mix(c.background, "#000000", 0.4)], foam: c.color13, swells: 26,
    }),
  }),
  "amethystora-light": (c) => {
    const lavender = (color, s) => mix(color, c.background, s);
    const sun = mix(c.color9, "#ffffff", 0.55);
    return {
      "lavender-hills": () => peaksSvg({
        seed: 1103, sky: [lavender(c.accent, 0.72), lavender(c.color13, 0.8), c.background], sun: [2560, 720, 180, sun],
        mist: c.background, haze: 0.45,
        ranges: [[1300, 520, 0.4, lavender(c.color13, 0.7)], [1520, 460, 0.38, lavender(c.color13, 0.55)],
          [1780, 420, 0.36, lavender(c.accent, 0.45)], [2080, 380, 0.34, lavender(c.accent, 0.25)]],
        petals: [c.color13, c.color5, lavender(c.color13, 0.5)],
      }),
      "lavender-waves": () => dunesSvg({
        seed: 1109, bg: c.background, top: 380, amp: 180, tilt: -0.09, shadow: c.accent, shadowOpacity: 0.14, sun: [2900, 440, 180, sun],
        bands: [[c.color13, c.color5], [c.color5, c.accent], [c.color14, c.color13], [c.color9, c.color13], [c.accent, c.color5], [c.color5, c.color14]]
          .map(([a, b], k) => [lavender(a, 0.78 - 0.08 * k), lavender(b, 0.78 - 0.08 * k)]),
      }),
      "lavender-pines": () => forestSvg({
        seed: 1117, sky: [lavender(c.accent, 0.62), lavender(c.color9, 0.78), lavender(c.color11, 0.82)], sun: [1200, 900, 160, sun],
        mist: c.background, haze: 0.42,
        hills: [[1250, 440, 0.5, lavender(c.color13, 0.62)]],
        rows: [[1330, 360, lavender(c.color13, 0.5)], [1560, 520, lavender(c.accent, 0.5)], [1820, 700, lavender(c.accent, 0.3)], [2100, 900, mix(c.accent, c.foreground, 0.35)]],
      }),
    };
  },
  "tokyo-night": (c, daylight) => ({
    "neon-city": () => skylineSvg({ daylight,
      seed: 211, sky: [mix(c.background, "#000000", 0.4), c.background, mix(c.background, c.color5, 0.4)], moon: [2860, 560, 210, mix(c.color13, "#ffffff", 0.35)],
      starColor: c.cursor, mist: mix(c.background, c.color5, 0.5), haze: 0.35, water: 1760, waterColor: mix(c.background, "#000000", 0.3), ripple: c.color13,
      rows: [[980, mix(c.background, c.color5, 0.16), 0], [760, mix(c.background, c.color8, 0.45), 0.1], [560, mix(c.background, "#000000", 0.35), 0.2]],
      windows: [c.color3, c.color13, c.color14, c.color4], neon: [c.color1, c.color14, c.color13], rain: c.color4,
    }),
    synthwave: () => retroSvg({
      seed: 223, horizon: 1360, sun: [1920, 1010, 520], sunColors: [c.color3, c.color11, c.color1, c.color13], starColor: c.cursor, stars: 220,
      sky: [mix(c.background, "#000000", 0.4), c.background, mix(c.background, c.color5, 0.45), mix(c.color1, c.background, 0.35)],
      ranges: [[400, 0.7, mix(c.background, c.color5, 0.28), c.color13], [250, 0.65, mix(c.background, "#000000", 0.3), c.color14]],
      ground: [mix(c.background, c.color5, 0.2), mix(c.background, "#000000", 0.4)], grid: c.color13,
    }),
    "night-silk": () => silkSvg({
      seed: 227, bg: mix(c.background, "#000000", 0.2), clouds: [c.color5, c.color4, c.color14, c.color1, c.color13], cloudOpacity: 0.3,
      ribbons: [[c.color4, c.color13, 1080, 420, 1.3], [c.color14, c.color5, 1260, 260, 1.8]],
    }),
  }),
  catppuccin: (c, daylight) => {
    const mauve = mix(c.color1, c.color4, 0.5);
    return {
      "lakeside-peaks": () => peaksSvg({ daylight,
        seed: 307, sky: [mix(c.background, "#000000", 0.3), c.background, mix(c.background, mauve, 0.45), mix(c.color1, c.color3, 0.45)],
        stars: 260, starColor: c.cursor, sun: [2760, 600, 120, c.cursor], mist: mix(c.background, c.color1, 0.55), haze: 0.3,
        ranges: [[1480, 820, 0.55, mix(c.background, mauve, 0.5), mix(c.cursor, mauve, 0.25)], [1480, 580, 0.55, mix(c.background, mauve, 0.3)],
          [1480, 380, 0.5, mix(c.background, c.color4, 0.18)], [1480, 210, 0.45, mix(c.background, "#000000", 0.25)]],
        lake: 1480, water: c.background, ripple: c.color5,
      }),
      "pastel-waves": () => dunesSvg({
        seed: 311, bg: c.background, top: 380, amp: 180, tilt: -0.1, shadow: "#000000", shadowOpacity: 0.55, sun: [2900, 420, 190, mix(c.color5, c.background, 0.1)],
        bands: [[mauve, c.color4], [c.color4, c.color6], [c.color1, mauve], [c.color6, c.color4], [c.color5, c.color1], [mauve, c.color4]]
          .map(([a, b], k) => [mix(a, c.background, 0.1 + 0.12 * k), mix(b, c.background, 0.1 + 0.12 * k)]),
      }),
      "ringed-planet": () => planetSvg({
        seed: 313, sky: [mix(c.background, "#000000", 0.35), c.background], nebula: [mauve, c.color4, c.color1, c.color6], nebulaOpacity: 0.35, starColor: c.cursor,
        planet: [1560, 1180, 500], colors: [c.color5, mix(mauve, c.background, 0.35)], bands: [c.color1, c.color3, c.color6, c.color4], ring: c.cursor,
        moon: [3060, 560, 90], moonColors: [c.color6, mix(c.color6, c.background, 0.6)],
      }),
    };
  },
  "catppuccin-latte": (c, daylight) => {
    const mauve = mix(c.color1, c.color4, 0.5);
    const pastel = (color, s = 0.6) => mix(color, c.background, s);
    return {
      "morning-peaks": () => peaksSvg({ daylight,
        seed: 401, sky: [pastel(c.accent, 0.72), pastel(c.color5, 0.78), c.background], sun: [1180, 680, 160, mix(c.color3, "#ffffff", 0.45)],
        mist: c.background, haze: 0.45,
        ranges: [[1500, 820, 0.55, pastel(c.accent, 0.68), "#ffffff"], [1500, 560, 0.55, pastel(mauve, 0.58)], [1500, 360, 0.5, pastel(c.color5, 0.5)],
          [1500, 200, 0.45, pastel(mix(mauve, c.foreground, 0.4), 0.3)]],
        lake: 1500, water: pastel(c.accent, 0.7), ripple: "#ffffff",
      }),
      "pastel-waves": () => dunesSvg({
        seed: 409, bg: c.background, top: 380, amp: 180, tilt: 0.1, shadow: c.foreground, shadowOpacity: 0.16, sun: [900, 440, 180, pastel(c.color3, 0.35)],
        bands: [[c.color5, mauve], [mauve, c.accent], [c.color6, c.accent], [c.color3, c.color5], [c.accent, c.color6], [mauve, c.color5]]
          .map(([a, b], k) => [pastel(a, 0.72 - 0.08 * k), pastel(b, 0.72 - 0.08 * k)]),
      }),
      silk: () => silkSvg({
        seed: 419, bg: c.background, clouds: [c.color5, c.accent, c.color6, c.color3, mauve], cloudOpacity: 0.3,
        ribbons: [[c.color5, c.accent, 1080, 420, 1.3], [c.color6, mauve, 1280, 260, 1.8]],
      }),
    };
  },
  everforest: (c, daylight) => ({
    "misty-pines": () => forestSvg({ daylight,
      seed: 503, sky: [mix(c.background, "#000000", 0.2), mix(c.background, c.accent, 0.3), mix(c.foreground, c.color3, 0.35)],
      sun: [2600, 880, 150, mix(c.color3, "#ffffff", 0.3)], mist: mix(c.foreground, c.accent, 0.35), haze: 0.35,
      hills: [[1250, 440, 0.5, mix(c.background, c.accent, 0.5)]],
      rows: [[1330, 360, mix(c.background, c.color6, 0.4)], [1560, 520, mix(c.background, c.color2, 0.24)], [1820, 700, mix(c.background, "#000000", 0.05)],
        [2100, 900, mix(c.background, "#000000", 0.4)]],
    }),
    "mountain-lake": () => peaksSvg({
      seed: 509, sky: [mix(c.background, c.accent, 0.35), mix(c.accent, c.foreground, 0.5), mix(c.color3, c.foreground, 0.45)],
      sun: [1300, 1180, 170, mix(c.color3, "#ffffff", 0.35)], mist: mix(c.foreground, c.color3, 0.3), haze: 0.35,
      ranges: [[1420, 760, 0.55, mix(c.background, c.accent, 0.55), mix(c.foreground, "#ffffff", 0.3)], [1420, 520, 0.55, mix(c.background, c.color6, 0.4)],
        [1420, 320, 0.5, mix(c.background, c.color2, 0.25)], [1420, 170, 0.45, mix(c.background, "#000000", 0.2)]],
      lake: 1420, water: mix(c.background, c.accent, 0.2), ripple: c.foreground,
    }),
    "moss-silk": () => silkSvg({
      seed: 521, bg: mix(c.background, "#000000", 0.15), clouds: [c.color2, c.accent, c.color6, c.color3, c.color2], cloudOpacity: 0.28,
      ribbons: [[c.color2, c.accent, 1100, 420, 1.3], [c.color3, c.color6, 1260, 260, 1.8]],
    }),
  }),
  gruvbox: (c, daylight) => {
    const orange = c.selection_background;
    return {
      "desert-sun": () => retroSvg({ daylight,
        seed: 601, horizon: 1380, sun: [1920, 1060, 470], sunColors: [c.color3, orange, c.color1], stars: 120, starColor: c.foreground,
        sky: [mix(c.background, "#000000", 0.2), c.background, mix(c.background, orange, 0.35), mix(c.color3, orange, 0.45)],
        ranges: [[380, 0, mix(c.background, orange, 0.42)], [260, 0, mix(c.background, c.color1, 0.25)]], mesas: true,
        ground: [mix(c.background, orange, 0.3), mix(c.background, "#000000", 0.2)],
        sand: [mix(c.background, orange, 0.28), mix(c.background, c.color3, 0.18), mix(c.background, orange, 0.1), mix(c.background, "#000000", 0.15)],
      }),
      "autumn-pines": () => forestSvg({
        seed: 607, sky: [mix(c.background, "#000000", 0.15), mix(c.background, c.color1, 0.3), mix(c.color3, orange, 0.35)],
        sun: [1100, 920, 170, mix(c.color3, "#ffffff", 0.2)], mist: mix(c.color3, orange, 0.4), haze: 0.3,
        hills: [[1260, 420, 0.5, mix(c.background, orange, 0.45)]],
        rows: [[1340, 360, mix(c.background, orange, 0.35)], [1570, 520, mix(c.background, c.color1, 0.24)], [1830, 700, mix(c.background, c.color3, 0.1)],
          [2100, 900, mix(c.background, "#000000", 0.35)]],
      }),
      "ember-dunes": () => dunesSvg({
        seed: 613, bg: c.background, top: 380, amp: 190, tilt: -0.08, shadow: "#000000", shadowOpacity: 0.6, sun: [2900, 460, 200, c.color3],
        bands: [[c.color3, orange], [orange, c.color1], [c.color1, c.color5], [c.color2, c.color3], [orange, c.color3], [c.color6, c.color2]]
          .map(([a, b], k) => [mix(a, c.background, 0.12 + 0.12 * k), mix(b, c.background, 0.12 + 0.12 * k)]),
      }),
    };
  },
  "matte-black": (c, daylight) => ({
    eclipse: () => planetSvg({ daylight,
      seed: 701, sky: [mix(c.background, "#000000", 0.6), c.background], nebula: [c.accent, c.color3, c.accent], nebulaOpacity: 0.07, starColor: c.foreground,
      planet: [2360, 980, 400], corona: [c.accent, mix(c.color2, "#ffffff", 0.4)], disc: mix(c.background, "#000000", 0.5),
    }),
    "ember-peaks": () => peaksSvg({
      seed: 709, sky: [mix(c.background, "#000000", 0.5), c.background, mix(c.background, c.accent, 0.14)], stars: 160, starColor: c.foreground,
      sun: [2820, 640, 80, c.accent], mist: mix(c.background, c.accent, 0.3), haze: 0.18,
      ranges: [[1300, 760, 0.5, mix(c.background, "#ffffff", 0.1)], [1520, 600, 0.48, mix(c.background, "#ffffff", 0.07)],
        [1780, 480, 0.45, mix(c.background, "#ffffff", 0.04)], [2060, 380, 0.42, mix(c.background, "#000000", 0.3)]],
    }),
    "night-sea": () => seaSvg({
      seed: 719, horizon: 1240, sky: [mix(c.background, "#000000", 0.5), c.background, mix(c.background, c.accent, 0.1)], stars: 200, starColor: c.foreground,
      moon: [2300, 880, 130, c.accent], coast: mix(c.background, "#ffffff", 0.06), sea: [mix(c.background, "#ffffff", 0.05), mix(c.background, "#000000", 0.5)],
      foam: mix(c.background, "#ffffff", 0.45), swells: 26,
    }),
  }),
  nord: (c, daylight) => ({
    aurora: () => peaksSvg({ daylight,
      seed: 809, sky: [mix(c.background, "#000000", 0.4), c.background, mix(c.background, c.color8, 0.7)], stars: 300, starColor: c.color15,
      aurora: [c.color2, c.color14, c.color5], mist: c.color8, haze: 0.3,
      ranges: [[1540, 720, 0.58, mix(c.background, c.accent, 0.35), c.color7], [1540, 480, 0.55, mix(c.background, c.accent, 0.18), c.color15],
        [1540, 280, 0.5, mix(c.color0, "#000000", 0.15)]],
      lake: 1540, water: mix(c.background, "#000000", 0.15), ripple: c.color6,
    }),
    "ice-sea": () => seaSvg({
      seed: 811, horizon: 1160, sky: [c.background, mix(c.background, c.accent, 0.45), mix(c.color6, c.color15, 0.55)],
      moon: [1100, 900, 120, mix(c.color15, "#ffffff", 0.4)], coast: mix(c.background, c.accent, 0.35),
      sea: [mix(c.background, c.accent, 0.4), mix(c.background, "#000000", 0.35)], foam: c.color15, swells: 24,
      ice: [c.color15, mix(c.color6, c.background, 0.25)], bergs: [[2700, 1290, 620, 300], [600, 1220, 280, 120], [1700, 1480, 900, 420], [3500, 1200, 200, 90]],
    }),
    "snow-drifts": () => dunesSvg({
      seed: 821, bg: mix(c.background, c.accent, 0.25), top: 560, amp: 170, tilt: 0.07, shadow: "#000000", shadowOpacity: 0.45, sun: [1000, 400, 150, c.color15],
      bands: [[c.color15, c.color7], [c.color6, c.color14], [c.color7, c.color6], [c.accent, c.color6], [c.color8, c.accent], [c.color0, c.color8]],
    }),
  }),
  "rose-pine-dawn": (c, daylight) => ({
    "dawn-hills": () => peaksSvg({ daylight,
      seed: 907, sky: [mix(c.color5, c.background, 0.62), mix(c.color6, c.background, 0.55), c.background], sun: [2480, 740, 190, mix(c.color3, c.background, 0.3)],
      mist: c.background, haze: 0.45,
      ranges: [[1300, 520, 0.4, mix(c.color5, c.background, 0.6)], [1520, 460, 0.38, mix(c.color6, c.background, 0.45)],
        [1780, 420, 0.36, mix(c.color1, c.background, 0.4)], [2080, 380, 0.34, mix(c.color5, c.foreground, 0.35)]],
      petals: [c.color6, c.color1, mix(c.color6, "#ffffff", 0.45)],
    }),
    "dawn-sea": () => seaSvg({
      seed: 911, horizon: 1200, sky: [mix(c.color5, c.background, 0.6), mix(c.color6, c.background, 0.5), c.background],
      moon: [1400, 980, 170, mix(c.color3, c.background, 0.25)], coast: mix(c.color5, c.background, 0.45),
      sea: [mix(c.accent, c.background, 0.45), mix(c.color2, c.foreground, 0.2)], foam: c.background, swells: 24,
    }),
    "rose-silk": () => silkSvg({
      seed: 919, bg: c.background, clouds: [c.color6, c.color5, c.accent, c.color3, c.color1], cloudOpacity: 0.28,
      ribbons: [[c.color1, c.color5, 1080, 420, 1.3], [c.color6, c.accent, 1280, 260, 1.8]],
    }),
  }),
};

// ------------------------------------------------------------- Daylight --

// Daylight shows the picture each theme opens on at four times of day, and GNOME fades from one to the next by itself
// (amethystora-theme writes the slideshow). The other three times are the same scene drawn from the theme's palette
// graded to the time in OKLab: every colour's lightness raised toward day or lowered toward night, and its hue pulled warm
// at dawn (toward peach) and dusk (toward rose), a little cooler by day and toward blue by night. The picture the theme
// was drawn as is reused for its own time: night for a dark theme, day for a light one.
const DAYLIGHT_GRADE = {
  dark: {
    dawn: [(L) => L + 0.14 * (1 - L), (a) => 0.85 * a + 0.035, (b) => 0.85 * b + 0.05],
    day: [(L) => L + 0.62 * (1 - L), (a) => 0.85 * a - 0.01, (b) => 0.85 * b - 0.03],
    dusk: [(L) => L + 0.08 * (1 - L), (a) => 0.85 * a + 0.06, (b) => 0.85 * b + 0.015],
  },
  light: {
    dawn: [(L) => 0.8 * L, (a) => 0.85 * a + 0.035, (b) => 0.85 * b + 0.05],
    dusk: [(L) => 0.62 * L, (a) => 0.85 * a + 0.06, (b) => 0.85 * b + 0.015],
    night: [(L) => 0.28 * L + 0.02, (a) => 0.85 * a, (b) => 0.85 * b - 0.03],
  },
};
// What only a night sky has, by the share of it each time keeps: the stars (how many, and how bright) and an aurora
const DAYLIGHT_SCENE = {
  night: { stars: 1, aurora: 1 },
  dawn: { stars: 0.25, aurora: 0.3 },
  day: { stars: 0, aurora: 0 },
  dusk: { stars: 0.35, aurora: 0.4 },
};

const srgbToLinear = (v) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
const linearToSrgb = (v) => (v <= 0.0031308 ? 12.92 * v : 1.055 * v ** (1 / 2.4) - 0.055);
// OKLab, the colour and back (Bjorn Ottosson's matrices, public domain), as palette.awk has them
function toOklab(hex) {
  const [r, g, b] = [1, 3, 5].map((i) => srgbToLinear(parseInt(hex.slice(i, i + 2), 16) / 255));
  const [l, m, s] = [
    0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b,
    0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b,
    0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b,
  ].map(Math.cbrt);
  return [0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s];
}
function oklabLinear(L, a, b) {
  const [l, m, s] = [L + 0.3963377774 * a + 0.2158037573 * b, L - 0.1055613458 * a - 0.0638541728 * b,
    L - 0.0894841775 * a - 1.291485548 * b].map((x) => x ** 3);
  return [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s, -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s];
}
// A colour graded to a time of day; one out of the screen's colours keeps its lightness and hue and loses chroma until in
function graded(hex, [fL, fa, fb]) {
  const [L0, a0, b0] = toOklab(hex);
  const L = Math.min(1, Math.max(0, fL(L0)));
  let [a, b] = [fa(a0), fb(b0)];
  let rgb = oklabLinear(L, a, b);
  for (let k = 0; k < 200 && rgb.some((v) => v < 0 || v > 1); k++) {
    [a, b] = [a * 0.97, b * 0.97];
    rgb = oklabLinear(L, a, b);
  }
  return "#" + rgb.map((v) => Math.round(255 * Math.min(1, Math.max(0, linearToSrgb(v)))).toString(16).padStart(2, "0")).join("");
}
// Every colour of a palette graded to the time, from the theme's own mode
const daylightPalette = (c, time, mode) =>
  Object.fromEntries(Object.entries(c).map(([key, hex]) => [key, graded(hex, DAYLIGHT_GRADE[mode][time])]));

const isLight = (theme) => existsSync(join(SHARED, "usr/share/amethystora/themes", theme, "light.mode"));
const DAYLIGHT_DIR = "usr/share/backgrounds/amethystora";
// Each theme's opening picture, as backgrounds.list names it, and its four times: the picture it is, and the three drawn
const DAYLIGHT = Object.keys(THEME_WALLPAPERS).filter((theme) => !theme.startsWith("amethystora")).map((theme) => {
  const opening = Object.keys(THEME_WALLPAPERS[theme](palette(theme)))[0];
  const own = isLight(theme) ? "day" : "night";
  const files = Object.fromEntries(["dawn", "day", "dusk", "night"].map((time) =>
    [time, `${DAYLIGHT_DIR}/${theme}/${opening}${time === own ? "" : `-${time}`}.png`]));
  return { theme, opening, own, files };
});
// The two Amethystora themes open on the cut stone's facets, dark and light, which are their night and their day, and each
// has its own other three. The dark theme's are its facets' shades graded, and its day stays a dark amethyst. The light
// theme's are shades of their own: its near-white graded turns to grey, and a palette's grade makes a night as dark as the
// dark theme's. They stay lilac stone, under a peach dawn, a rose dusk and a pale moon, lit as the dark facets are (more of
// the ramp shows than in the white facets, whose glow is all white)
const FACETS_DAYLIGHT = {
  dawn: `${DAYLIGHT_DIR}/amethystora-facets-l-dawn.png`, day: `${DAYLIGHT_DIR}/amethystora-facets-l.png`,
  dusk: `${DAYLIGHT_DIR}/amethystora-facets-l-dusk.png`, night: `${DAYLIGHT_DIR}/amethystora-facets-l-night.png`,
};
const FACETS_DARK_DAYLIGHT = {
  dawn: `${DAYLIGHT_DIR}/amethystora-facets-dawn.png`, day: `${DAYLIGHT_DIR}/amethystora-facets-day.png`,
  dusk: `${DAYLIGHT_DIR}/amethystora-facets-dusk.png`, night: `${DAYLIGHT_DIR}/amethystora-facets-d.png`,
};
const FACETS_DARK_DAY_GRADE = [(L) => L + 0.2 * (1 - L), (a) => a + 0.02, (b) => b - 0.05];
const FACETS_LIGHT_TONES = {
  dawn: ["#8660c4", "#9e78ce", "#bc94d6", "#dab4d8", "#efcfce", "#fce4ca"],
  dusk: ["#5f42a6", "#7050b0", "#8a60b8", "#aa72b8", "#d08cb0", "#efaa9e"],
  night: ["#3f3598", "#4a40a6", "#5650b4", "#6a68c4", "#8c8cd6", "#b4b6ea"],
};
const DAYLIGHT_PNGS = [
  ...[
    [FACETS_DARK_DAYLIGHT.dawn, DAYLIGHT_GRADE.dark.dawn],
    [FACETS_DARK_DAYLIGHT.dusk, DAYLIGHT_GRADE.dark.dusk],
    [FACETS_DARK_DAYLIGHT.day, FACETS_DARK_DAY_GRADE],
  ].map(([out, grade]) => ({
    out, width: WALL_W, height: WALL_H, daylight: true,
    html: () => svgPage(facetsSvg("dark", FACET_TONES.dark.map((hex) => graded(hex, grade))), WALL_W, WALL_H),
  })),
  ...["dawn", "dusk", "night"].map((time) => ({
    out: FACETS_DAYLIGHT[time], width: WALL_W, height: WALL_H, daylight: true,
    html: () => svgPage(facetsSvg("dark", FACETS_LIGHT_TONES[time]), WALL_W, WALL_H),
  })),
  ...DAYLIGHT.flatMap(({ theme, opening, own, files }) => ["dawn", "day", "dusk", "night"].filter((time) => time !== own)
    .map((time) => ({
      out: files[time], width: WALL_W, height: WALL_H, daylight: true,
      html: () => svgPage(THEME_WALLPAPERS[theme](daylightPalette(palette(theme), time, own === "day" ? "light" : "dark"),
        DAYLIGHT_SCENE[time])[opening](), WALL_W, WALL_H),
    }))),
];
const daylightList = (files) => ["dawn", "day", "dusk", "night"].map((time) => `${time} /${files[time]}
`).join("");

// ---------------------------------------------------------- login screen --

// The login screen stands on the default dark theme's wallpaper, Facets, so that the desktop a person
// logs in to starts from the picture they logged in on. 12-login-screen.sh puts it inside GNOME Shell's
// theme, which is the only place the login screen takes a background from.
const LOGIN = { width: 2560, height: 1440 };
const SOFT = { blur: 44, overscan: 1.12 };

// The installer stands on the crown of a cut stone, in a square, so that the crown covers it instead of
// being stretched, and soft: its sidebar and top bar are narrow strips of it with labels all over them
const installerSkyHtml = ({ width, height, soft = false }) => `<!doctype html><html><head><style>
    html,body{margin:0;background:${CROWN_SKY[0]}}
    #frame{position:relative;width:${width}px;height:${height}px;overflow:hidden}
    #sky{position:absolute;inset:0;background:linear-gradient(180deg,${CROWN_SKY[0]},${CROWN_SKY[1]})}
    /* A blur this wide pulls in whatever is past the edge, which is nothing, and would leave the
       picture fading out around its border. The layer is drawn oversized and the frame clips it,
       so the fade happens outside the picture instead of inside it. */
    #sky.soft{transform:scale(${SOFT.overscan});filter:blur(${SOFT.blur}px)}
    #crown{display:block;width:100%;height:100%;object-fit:cover}
    /* This takes the soft picture down to a violet that white text has the contrast to be read
       on. The same thing is done to the boot menu, for the same reason. */
    #scrim{position:absolute;inset:0;background:radial-gradient(60% 60% at 50% 46%,
      rgba(11,4,20,0.42) 0%, rgba(11,4,20,0.60) 60%, rgba(11,4,20,0.74) 100%)}
  </style></head><body><div id="frame"><div id="sky"${soft ? ' class="soft"' : ""}>
    <img id="crown" src="${svgData(crownSvg)}"></div>${soft ? '<div id="scrim"></div>' : ""}</div></body></html>`;

const logo = (spec) => ({ ...spec, html: () => logoHtml(spec) });

// ------------------------------------------------------------ GNOME Tour --

// The picture on the first page of GNOME Tour. Fedora builds the Tour with its own in place of GNOME's,
// a backpack with Fedora's logo on it, and 12-tour.sh puts this one in its place: the boot splash at the
// top of its first glow, cut to the Tour's 600x300 frame around the
// mark, as a card with the window's rounded corners. Plain shapes and gradients only, and nothing ahead
// of the <svg> tag: GTK draws it, not a browser.
const TOUR = { width: 600, height: 300, crop: 1280, radius: 12 };
const tourSvg = (() => {
  const f = new Function(`${BOOT_TIMELINE}\nreturn frame;`)()(BOOT.hold + BOOT.whole + BOOT.cycle, BOOT, SIDES);
  const [x0, y0] = [HEART[0] - TOUR.crop / 2, HEART[1] - TOUR.crop / 4];
  const inner = (svg) => svg.slice(svg.indexOf(">") + 1, svg.lastIndexOf("</svg>")).trim();
  // A picture's box of the given units, px across and centred on the heart, as the splash places it
  const around = (units, px) => `translate(${num(HEART[0] - px / 2)} ${num(HEART[1] - px / 2)}) scale(${num(px / units)})`;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${TOUR.width}" height="${TOUR.height}" viewBox="0 0 ${TOUR.width} ${TOUR.height}">
  <defs>
    <clipPath id="card"><rect width="${TOUR.width}" height="${TOUR.height}" rx="${TOUR.radius}"/></clipPath>
  </defs>
  <g clip-path="url(#card)"><g transform="scale(${num(TOUR.width / TOUR.crop)}) translate(${num(-x0)} ${num(-y0)})">
  <rect x="${num(x0)}" y="${num(y0)}" width="${TOUR.crop}" height="${TOUR.crop / 2}" fill="${BOOT.sky}"/>
  <g opacity="${num(f.halo)}" transform="${around(200, BOOT.halo)}">${inner(haloSvg)}</g>
  <g transform="${around(256, 256 * BOOT.unit)}">${inner(gemSvg())}</g>
  <g opacity="${num(f.light)}" transform="${around(256, 256 * BOOT.unit)}">${inner(lightSvg)}</g>
  </g></g>
</svg>
`;
})();

// ------------------------------------------------------------- GRUB menu --

// The boot menu is the first Amethystora screen anyone sees, so it stands on the crystal field of
// the desktop's wallpapers, dimmed until the entries read over it, and hands over to the Plymouth
// splash on the same dark ground. The gfxmenu layout follows the theme format of
// vinceliuice/grub2-themes (GPL-3.0); the artwork is Amethystora's own.
//
// The font is not here: theme.txt asks for "Inter Regular 16", which 06-branding.sh builds into
// font16.pf2 with grub2-mkfont, because GRUB reads its own bitmap font format and not TTF.
const GRUB = "usr/share/grub/themes/amethystora";
const GRUB_W = 1920;
const GRUB_H = 1080;
const GRUB_ROW = 48; // height of a menu row, and so of the selection pill
const GRUB_CAP = 24; // width of the pill's rounded end
const GRUB_FONT = "Inter Regular 16";
const GRUB_PILL = SHADES[5];

// The wallpaper, scaled into a 16:9 frame and darkened, so white menu text holds its contrast
// wherever the crystals fall
const grubBackgroundHtml = () => `<!doctype html><html><head><style>
    html,body{margin:0;background:${SHADES[0]}}
    #frame{position:relative;width:${GRUB_W}px;height:${GRUB_H}px;overflow:hidden}
    #frame svg{display:block;width:100%;height:100%}
    #scrim{position:absolute;inset:0;background:linear-gradient(180deg,
      rgba(14,4,25,0.45) 0%, rgba(14,4,25,0.80) 46%, rgba(14,4,25,0.62) 100%)}
  </style></head><body><div id="frame">${wallpaperSvg("dark")}<div id="scrim"></div></div></body></html>`;

// The selection pill is sliced into three: a rounded cap at each end and a one-column middle that
// GRUB repeats to whatever width the entry needs. Each cap is the same rounded rectangle, shown
// through a viewBox over its left or right half, so the two ends always match.
const grubCap = (side) => `<svg xmlns="http://www.w3.org/2000/svg" width="${GRUB_CAP}" height="${GRUB_ROW}"
  viewBox="${side === "e" ? GRUB_CAP : 0} 0 ${GRUB_CAP} ${GRUB_ROW}">
  <rect width="${GRUB_CAP * 2}" height="${GRUB_ROW}" rx="${GRUB_ROW / 2}" fill="${GRUB_PILL}"/>
</svg>`;
const grubMiddle = `<svg xmlns="http://www.w3.org/2000/svg" width="8" height="${GRUB_ROW}" viewBox="0 0 8 ${GRUB_ROW}">
  <rect width="8" height="${GRUB_ROW}" fill="${GRUB_PILL}"/>
</svg>`;

function grubTheme() {
  return `# Amethystora boot menu
# Generated by branding/generate.mjs. The layout follows the GRUB gfxmenu theme format as used by
# vinceliuice/grub2-themes (GPL-3.0); the artwork is Amethystora's own.
#
# Installed on a running system by amethystora-grub-theme, which copies this directory to
# /boot/grub2/themes/amethystora: GRUB reads the boot filesystem, not the image.

title-text: ""
desktop-image: "background.png"
desktop-image-scale-method: "stretch"
desktop-color: "${SHADES[0]}"
# GRUB's terminal, shown once an entry is chosen, is black and can be no other colour (the signed GRUB has no
# background_color): it fills the screen, so that choosing an entry turns the screen black rather than leaving a box.
terminal-font: "${GRUB_FONT}"
terminal-left: "0%"
terminal-top: "0%"
terminal-width: "100%"
terminal-height: "100%"
terminal-border: "0"

# The wordmark, above the menu
+ image {
  left = 50%-240
  top = 11%
  width = 480
  height = 142
  file = "logo.png"
}

+ boot_menu {
  left = 25%
  top = 34%
  width = 50%
  height = 42%
  item_font = "${GRUB_FONT}"
  selected_item_font = "${GRUB_FONT}"
  item_color = "#cdc2dc"
  selected_item_color = "#ffffff"
  icon_width = 28
  icon_height = 28
  item_icon_space = 14
  item_height = ${GRUB_ROW}
  item_padding = 10
  item_spacing = 4
  selected_item_pixmap_style = "select_*.png"
}

# The countdown, as a bar and as words under it
+ progress_bar {
  id = "__timeout__"
  left = 25%
  top = 80%
  width = 50%
  height = 4
  show_text = false
  bg_color = "#2a2136"
  fg_color = "${SHADES[6]}"
  border_color = "#2a2136"
}

+ label {
  id = "__timeout__"
  left = 25%
  top = 80%+18
  width = 50%
  align = "center"
  text = "Booting in %d s"
  color = "#8f839f"
  font = "${GRUB_FONT}"
}

+ label {
  left = 0
  top = 100%-56
  width = 100%
  align = "center"
  text = "Arrow keys to choose    Enter to boot    E to edit    C for a console"
  color = "#6b5f7d"
  font = "${GRUB_FONT}"
}
`;
}

const GRUB_PNGS = [
  { out: `${GRUB}/background.png`, width: GRUB_W, height: GRUB_H, html: grubBackgroundHtml },
  logo({ out: `${GRUB}/logo.png`, width: 480, height: 142, text: "white" }),
  { out: `${GRUB}/select_w.png`, width: GRUB_CAP, height: GRUB_ROW, html: () => svgPage(grubCap("w"), GRUB_CAP, GRUB_ROW) },
  { out: `${GRUB}/select_e.png`, width: GRUB_CAP, height: GRUB_ROW, html: () => svgPage(grubCap("e"), GRUB_CAP, GRUB_ROW) },
  { out: `${GRUB}/select_c.png`, width: 8, height: GRUB_ROW, html: () => svgPage(grubMiddle, 8, GRUB_ROW) },
  // GRUB picks an icon by the menu entry's class. Fedora's entries carry "fedora"; the others are
  // there for the entries a dual-boot machine adds.
  logo({ out: `${GRUB}/icons/fedora.png`, width: 64, height: 64 }),
  logo({ out: `${GRUB}/icons/amethystora.png`, width: 64, height: 64 }),
  logo({ out: `${GRUB}/icons/gnu-linux.png`, width: 64, height: 64 }),
];

// ------------------------------------------------------------- installer --

// The installer on the ISO is Fedora's Anaconda, installed from packages rather than taken from the
// image, so nothing above reaches it. Anaconda takes a product's look from a product.img, which it
// finds in the images/ directory of the install media and lays over its own root before it starts:
// .github/workflows/build-iso.yml packs iso/product into one and adds it to the ISO. It holds this
// stylesheet, which 99-amethystora.conf names in place of Fedora's, and the pictures it draws: the
// login screen's picture, blurred, behind the sidebar and the top bar, at the head of the sidebar the gem,
// glowing as it does on the login screen, and the desktop's accent on the buttons, bars and
// selections GTK would draw in blue. The product name beside them is the image's os-release NAME.
const PRODUCT = join(ROOT, "iso/product");
const ANACONDA = "usr/share/anaconda/pixmaps/amethystora";
const INSTALLER_SKY = 1024;

// Anaconda loads it after anaconda-gtk.css, which leaves .logo-sidebar, .logo and the spoke's
// #nav-box for a product to fill. cover lets the one square of sky serve the tall sidebar and the
// wide top bar alike.
const anacondaCss = () => `/* Amethystora's look for the Anaconda installer
 * Generated by branding/generate.mjs */

@define-color amethystora ${CROWN_SKY[1]};

.logo-sidebar {
    background-color: @amethystora;
    background-image: url('/${ANACONDA}/sky.png');
    background-size: cover;
    background-position: center;
    background-repeat: no-repeat;
}

.logo {
    background-image: url('/${ANACONDA}/sidebar-logo.png');
    background-position: 50% 8px;
    background-repeat: no-repeat;
    background-color: transparent;
}

AnacondaSpokeWindow #nav-box {
    background-color: @amethystora;
    background-image: url('/${ANACONDA}/sky.png');
    background-size: cover;
    background-position: center;
    background-repeat: no-repeat;
    color: white;
}

/* The desktop's accent where Adwaita draws its blue. A rule in this stylesheet outranks the
 * theme's whatever their selectors, so it covers every state it matches: each state that has to
 * look different is given its own rule, and disabled ones are left to the theme to grey out. */
button.suggested-action:not(:disabled) {
    color: white;
    border-color: ${SHADES[4]};
    background-image: image(${ACCENT});
}

button.suggested-action:not(:disabled):hover {
    background-image: image(${SHADES[6]});
}

button.suggested-action:not(:disabled):active,
button.suggested-action:not(:disabled):checked {
    background-image: image(${SHADES[4]});
}

progressbar progress {
    border-color: ${SHADES[4]};
    background-color: ${ACCENT};
    background-image: none;
}

.view:selected,
row:selected,
entry selection,
label selection,
textview text selection {
    color: white;
    background-color: ${ACCENT};
}

check:checked:not(:disabled),
radio:checked:not(:disabled) {
    color: white;
    border-color: ${SHADES[4]};
    background-image: image(${ACCENT});
}

switch:checked:not(:disabled) {
    border-color: ${SHADES[4]};
    background-color: ${ACCENT};
    background-image: none;
}
`;

const INSTALLER_PNGS = [
  { root: PRODUCT, out: `${ANACONDA}/sky.png`, width: INSTALLER_SKY, height: INSTALLER_SKY,
    html: () => installerSkyHtml({ width: INSTALLER_SKY, height: INSTALLER_SKY, soft: true }) },
  // The glow of the login screen's logo, with room around the gem for it to fade out in
  logo({ root: PRODUCT, out: `${ANACONDA}/sidebar-logo.png`, width: 180, height: 180, glow: 10, fit: 0.55 }),
];

const PNGS = [
  logo({ out: "usr/share/pixmaps/amethystora-wordmark.png", width: 690, height: 204, text: "dark" }),
  logo({ out: "usr/share/pixmaps/amethystora-wordmark-medium.png", width: 345, height: 102, text: "dark" }),
  logo({ out: "usr/share/pixmaps/amethystora-wordmark-small.png", width: 205, height: 61, text: "dark" }),
  logo({ out: "usr/share/pixmaps/amethystora-wordmark-white.png", width: 345, height: 102, text: "white" }),
  // The login screen's logo, with the gem still glowing. GDM draws it at its own size and gives it
  // no box to fit, so the canvas can be as large as the glow needs without the wordmark shrinking.
  logo({ out: "usr/share/pixmaps/amethystora-wordmark-glow.png", width: 460, height: 150, text: "white", glow: 10, fit: 0.66 }),
  logo({ out: "usr/share/pixmaps/amethystora-logo-512.png", width: 512, height: 512 }),
  logo({ out: "usr/share/pixmaps/amethystora-logo-400.png", width: 400, height: 400 }),
  logo({ out: "usr/share/pixmaps/amethystora-logo-white.png", width: 252, height: 252, white: true }),
  // Fedora's spinner theme, the fallback splash, looks its watermark up by this name
  logo({ out: "usr/share/plymouth/themes/spinner/watermark.png", width: 330, height: 64, text: "white" }),
  ...BOOT_PNGS,
  { out: "usr/share/backgrounds/amethystora/amethystora-l.png", width: 3840, height: 2160, html: () => wallpaperHtml("light") },
  { out: "usr/share/backgrounds/amethystora/amethystora-d.png", width: 3840, height: 2160, html: () => wallpaperHtml("dark") },
  ...[
    ["facets", (theme) => svgPage(facetsSvg(theme), WALL_W, WALL_H)],
    ["nebula", nebulaWallpaperHtml],
  ].flatMap(([name, html]) =>
    [["l", "light"], ["d", "dark"]].map(([suffix, theme]) => ({
      out: `usr/share/backgrounds/amethystora/amethystora-${name}-${suffix}.png`, width: WALL_W, height: WALL_H, html: () => html(theme),
    }))),
  // Smaller than the wallpaper on purpose: every byte of it is in GNOME Shell's theme bundle, and GNOME
  // scales it to the screen either way
  { out: "usr/share/backgrounds/amethystora/amethystora-login.png", width: LOGIN.width, height: LOGIN.height,
    html: () => svgPage(facetsSvg("dark"), LOGIN.width, LOGIN.height) },
  ...Object.entries(THEME_WALLPAPERS).flatMap(([theme, pictures]) => {
    const drawn = pictures(palette(theme));
    return Object.keys(drawn).map((name) => ({
      out: `usr/share/backgrounds/amethystora/${theme}/${name}.png`, width: WALL_W, height: WALL_H, html: () => svgPage(drawn[name](), WALL_W, WALL_H),
    }));
  }),
  ...DAYLIGHT_PNGS,
  ...GRUB_PNGS,
  ...INSTALLER_PNGS,
];

function findBrowser() {
  const candidates = [
    process.env.BROWSER,
    "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
    "C:/Program Files/Microsoft/Edge/Application/msedge.exe",
    "C:/Program Files/Google/Chrome/Application/chrome.exe",
    "/usr/bin/chromium-browser",
    "/usr/bin/chromium",
    "/usr/bin/google-chrome",
    "/usr/bin/microsoft-edge",
  ].filter(Boolean);
  const found = candidates.find((c) => existsSync(c));
  if (!found) throw new Error("No Chromium-based browser found; set BROWSER=/path/to/browser");
  return found;
}

// ONLY=substring renders just the PNGs whose path contains it, for quick previews; ONLY=daylight renders Daylight's
function renderPngs() {
  const browser = findBrowser();
  const work = join(tmpdir(), "amethystora-branding");
  // Best effort: the browser can hold its profile there for a while after it has returned
  const clean = () => { try { rmSync(work, { recursive: true, force: true }); } catch {} };
  clean();
  mkdirSync(work, { recursive: true });
  const only = process.env.ONLY;
  for (const png of PNGS.filter((p) => !only || (only === "daylight" ? p.daylight : p.out.includes(only)))) {
    const html = join(work, "page.html");
    writeFileSync(html, png.html());
    const out = join(png.root ?? SHARED, png.out);
    mkdirSync(dirname(out), { recursive: true });
    rmSync(out, { force: true });
    execFileSync(browser, [
      "--headless", "--disable-gpu", "--hide-scrollbars", "--force-device-scale-factor=1",
      "--default-background-color=00000000", `--user-data-dir=${join(work, "profile")}`,
      `--window-size=${png.width},${png.height}`, `--screenshot=${out}`, pathToFileURL(html).href,
    ], { stdio: "ignore" });
    // Edge on Windows can return before its screenshot is on disk, or while it is still writing it,
    // so wait for the PNG's last chunk
    const done = () => { try { return readFileSync(out).subarray(-8, -4).toString() === "IEND"; } catch { return false; } };
    for (const start = Date.now(); !done() && Date.now() - start < 60000; ) Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 250);
    if (!done()) throw new Error(`Rendering ${png.out} failed`);
    console.log(`rendered ${png.out}`);
  }
  clean();
}

// ---------------------------------------------------------------- write --

function write(rel, content, root = SHARED) {
  const out = join(root, rel);
  mkdirSync(dirname(out), { recursive: true });
  writeFileSync(out, content);
  console.log(`wrote ${rel}`);
}

// --preview only writes the animated preview of the boot splash, and prints where
if (process.argv.includes("--preview")) {
  const out = join(tmpdir(), "amethystora-boot-preview.html");
  writeFileSync(out, bootPreviewHtml());
  console.log(out);
  process.exit(0);
}

write("usr/share/icons/hicolor/scalable/apps/amethystora-logo.svg", gemSvg());
// The gem the wordmark sets as its "a", cropped as logoHtml crops it, and its stem alone in the same box:
// the apps draw the wordmark live from these polygons (apps/common/src/gem.rs), in the same font, size and
// glow, so that their sidebars carry the mark of the boot splash and login screen, and Notes lifts the stem
// while it unlocks and Logs blinks it while it reads
const APPS = join(ROOT, "apps");
write("common/assets/gem.svg", gemSvg(false, BOUNDS.join(" ")), APPS);
write("common/assets/gem-stem.svg", gemSvg(false, BOUNDS.join(" "), FACETS.filter((f) => f.part === "stem")), APPS);
write("usr/share/icons/hicolor/scalable/actions/amethystora-logo-symbolic.svg", symbolicSvg);
write("usr/share/icons/hicolor/scalable/places/amethystora-docs.svg", tileSvg(docsGlyph));
write("usr/share/icons/hicolor/scalable/places/amethystora-community.svg", tileSvg(communityGlyph));
write("usr/share/icons/hicolor/scalable/places/amethystora-update.svg", tileSvg(updateGlyph));
write("usr/share/icons/hicolor/scalable/places/amethystora-logs.svg", tileSvg(logsGlyph));
write("usr/share/icons/hicolor/scalable/places/amethystora-control.svg", tileSvg(controlGlyph));
write("usr/share/icons/hicolor/scalable/places/amethystora-backups.svg", tileSvg(backupsGlyph));
write("usr/share/icons/hicolor/scalable/places/amethystora-vault.svg", tileSvg(vaultGlyph));
write("usr/share/amethystora/logos/symbols/amethystora", ansiLogo());
write("usr/share/amethystora/logos/console/amethystora", consoleLogo());
rmSync(join(SHARED, "usr/share/amethystora/logos/glint"), { recursive: true, force: true });
glintFrames().forEach((frame, k) => write(`usr/share/amethystora/logos/glint/${String(k).padStart(2, "0")}`, frame));
write(`${THEME}/amethystora.script`, bootScript());
write(`${GRUB}/theme.txt`, grubTheme());
write(`${ANACONDA}/amethystora.css`, anacondaCss(), PRODUCT);
write("usr/share/amethystora/tour/welcome.svg", tourSvg);
write("usr/share/amethystora/themed/wallpaper-contours.svg.tpl", contoursTemplate());
write("usr/share/amethystora/themed/wallpaper-ridges.svg.tpl", ridgesTemplate());
write("usr/share/amethystora/themed/wallpaper-waves.svg.tpl", wavesTemplate());
// Each theme's four pictures by the time of day, which amethystora-theme makes Daylight's slideshow of
write("usr/share/amethystora/themes/amethystora/daylight.list", daylightList(FACETS_DARK_DAYLIGHT));
write("usr/share/amethystora/themes/amethystora-light/daylight.list", daylightList(FACETS_DAYLIGHT));
for (const { theme, files } of DAYLIGHT) write(`usr/share/amethystora/themes/${theme}/daylight.list`, daylightList(files));
renderPngs();
