// The glyphs of the Amethystora icon packs, drawn for Amethystora: none is traced from or modelled on another set.
//
// Each glyph is drawn in a 256 box, its content kept inside 36..220, with two inks: `ink`, the body, and `soft`, the
// details on it. The packs render the same glyph twice (branding/icons.mjs): Glass fills the body with a gradient and
// the details with a lighter one, Neon draws the body's outline lit and the details lit inside it. So a detail is a
// separate shape over the body, never a hole cut into it, and a body is one silhouette. Neon draws every soft shape
// over every body shape, so a shape drawn in `ink` on top of a soft one (the dots on a speech bubble) would vanish
// under it: such a shape is drawn in a third ink, `over` (`over = ink` in the glyph's arguments: Glass and Duotone
// give it the body's, Neon its own, lit in the details' colour, on top of everything).

const f = n => +n.toFixed(1);

// A circle as a path, for cutting holes with fill-rule="evenodd"
const circlePath = (cx, cy, r) => `M${cx + r} ${cy}A${r} ${r} 0 1 0 ${cx - r} ${cy}A${r} ${r} 0 1 0 ${cx + r} ${cy}Z`;

function gearPath(cx, cy, outer, inner, teeth, hole) {
    let d = '';
    for (let i = 0; i < teeth * 4; i++) {
        const a = (i / (teeth * 4)) * 2 * Math.PI - Math.PI / 2;
        const r = (i % 4 === 0 || i % 4 === 1) ? outer : inner;
        d += `${i ? 'L' : 'M'}${f(cx + r * Math.cos(a))} ${f(cy + r * Math.sin(a))}`;
    }
    return d + 'Z' + circlePath(cx, cy, hole);
}

function starPath(cx, cy, outer, inner) {
    let d = '';
    for (let i = 0; i < 10; i++) {
        const a = (i / 10) * 2 * Math.PI - Math.PI / 2, r = i % 2 ? inner : outer;
        d += `${i ? 'L' : 'M'}${f(cx + r * Math.cos(a))} ${f(cy + r * Math.sin(a))}`;
    }
    return d + 'Z';
}

// An arc of a circle from one angle to another (degrees, clockwise from three o'clock) as a stroked path, and the
// filled arrowhead at its end, pointing the way the arc runs
function arcArrow(cx, cy, r, from, to, width, colour) {
    const rad = a => (a * Math.PI) / 180, at = a => [cx + r * Math.cos(rad(a)), cy + r * Math.sin(rad(a))];
    const [x1, y1] = at(from), [x2, y2] = at(to), sweep = to > from ? 1 : 0;
    const large = Math.abs(to - from) > 180 ? 1 : 0;
    const dir = sweep ? 1 : -1, t = [-Math.sin(rad(to)) * dir, Math.cos(rad(to)) * dir], n = [Math.cos(rad(to)), Math.sin(rad(to))];
    const head = width * 1.25, back = [x2 - t[0] * 2, y2 - t[1] * 2];
    const tip = [x2 + t[0] * head * 1.1, y2 + t[1] * head * 1.1];
    const p1 = [back[0] + n[0] * head, back[1] + n[1] * head], p2 = [back[0] - n[0] * head, back[1] - n[1] * head];
    return `<path d="M${f(x1)} ${f(y1)}A${r} ${r} 0 ${large} ${sweep} ${f(x2)} ${f(y2)}" fill="none" stroke="${colour}" stroke-width="${width}" stroke-linecap="round"/>` +
        `<path d="M${f(tip[0])} ${f(tip[1])}L${f(p1[0])} ${f(p1[1])}L${f(p2[0])} ${f(p2[1])}Z" fill="${colour}" stroke="${colour}" stroke-width="4" stroke-linejoin="round"/>`;
}

// A stroked line. The packs paint in gradients laid over each shape's box, and a box with no width or height (a
// straight line along an axis) takes no gradient and is not drawn at all, so such a line is drawn as the rounded bars
// it looks like
function line(d, colour, width) {
    const stroke = `<path d="${d}" fill="none" stroke="${colour}" stroke-width="${width}" stroke-linecap="round" stroke-linejoin="round"/>`;
    if (/[^MmLlHhVv\d\s.,-]/.test(d))
        return stroke;
    const segments = [];
    let x = 0, y = 0;
    for (const [, cmd, args] of d.matchAll(/([MmLlHhVv])([^MmLlHhVv]*)/g)) {
        const n = args.match(/-?\d*\.?\d+/g).map(Number), rel = cmd === cmd.toLowerCase();
        const step = 'HhVv'.includes(cmd) ? 1 : 2;
        for (let i = 0; i < n.length; i += step) {
            const from = [x, y];
            if ('Hh'.includes(cmd))
                x = rel ? x + n[i] : n[i];
            else if ('Vv'.includes(cmd))
                y = rel ? y + n[i] : n[i];
            else
                [x, y] = rel ? [x + n[i], y + n[i + 1]] : [n[i], n[i + 1]];
            // A moveto's first pair only moves; the pairs after it are lines
            if (!('Mm'.includes(cmd) && i === 0))
                segments.push([...from, x, y]);
        }
    }
    const xs = segments.flatMap(s => [s[0], s[2]]), ys = segments.flatMap(s => [s[1], s[3]]);
    if (!segments.length || (Math.min(...xs) !== Math.max(...xs) && Math.min(...ys) !== Math.max(...ys)))
        return stroke;
    return segments.map(([x1, y1, x2, y2]) => `<rect x="${f(Math.min(x1, x2) - width / 2)}" y="${f(Math.min(y1, y2) - width / 2)}" ` +
        `width="${f(Math.abs(x2 - x1) + width)}" height="${f(Math.abs(y2 - y1) + width)}" rx="${f(width / 2)}" fill="${colour}"/>`).join('');
}

// A document page and its folded corner, which every file type shares
const PAGE = 'M76 40h84l34 34v128a14 14 0 0 1-14 14H76a14 14 0 0 1-14-14V54a14 14 0 0 1 14-14z';
const page = ({ink, soft}, emblem = '') => `<path d="${PAGE}" fill="${ink}"/><path d="M160 40v22a12 12 0 0 0 12 12h22" fill="${soft}"/>${emblem}`;

// A folder: its back flap in the soft ink, its front in the body ink, and an emblem on the front, centred on 128 148
const folder = ({ink, soft}, emblem = '') =>
    `<path d="M40 78a14 14 0 0 1 14-14h50l20 20h78a14 14 0 0 1 14 14v10H40z" fill="${soft}"/>` +
    `<path d="M36 106a14 14 0 0 1 14-14h156a14 14 0 0 1 14 14v84a16 16 0 0 1-16 16H52a16 16 0 0 1-16-16z" fill="${ink}"/>${emblem}`;

// The emblems, drawn in the soft ink around 128 148: on a folder's front, and on a page
const EMBLEM = {
    documents: s => line('M100 132h56M100 150h56M100 168h34', s, 10),
    download: s => line('M128 118v40M110 142l18 18 18-18', s, 12) + line('M102 180h52', s, 10),
    music: s => `<circle cx="120" cy="170" r="13" fill="${s}"/><path d="M128 118h8v52h-8z" fill="${s}"/><path d="M132 116c12 2 26 10 24 26-6-8-14-10-24-10z" fill="${s}"/>`,
    pictures: s => `<path d="M96 180l24-30 16 18 10-12 18 24z" fill="${s}" stroke="${s}" stroke-width="4" stroke-linejoin="round"/><circle cx="150" cy="126" r="10" fill="${s}"/>`,
    videos: s => `<path d="M114 122v52l42-26z" fill="${s}" stroke="${s}" stroke-width="8" stroke-linejoin="round"/>`,
    templates: s => `<rect x="100" y="122" width="56" height="54" rx="9" fill="none" stroke="${s}" stroke-width="8" stroke-dasharray="11 8"/>`,
    publicshare: s => line('M108 150l40-22M108 150l40 22', s, 7) + `<circle cx="106" cy="150" r="11" fill="${s}"/><circle cx="150" cy="126" r="11" fill="${s}"/><circle cx="150" cy="174" r="11" fill="${s}"/>`,
    desktop: s => `<rect x="96" y="120" width="64" height="42" rx="7" fill="${s}"/>` + line('M128 164v10M112 178h32', s, 8),
    remote: s => `<path d="M108 176h42a17 17 0 0 0 3-34 23 23 0 0 0-44-5 20 20 0 0 0-1 39z" fill="${s}"/>`,
    locked: s => line('M114 140v-10a14 14 0 0 1 28 0v10', s, 8) + `<rect x="102" y="138" width="52" height="40" rx="9" fill="${s}"/>`,
    bookmarks: s => `<path d="M108 120h40v60l-20-14-20 14z" fill="${s}" stroke="${s}" stroke-width="4" stroke-linejoin="round"/>`,
};

// The letters a file type's page names it with (`label-PY`), drawn as strokes in a 20 by 28 box, so that no font is
// needed to show them
const LETTERS = {
    A: 'M0 28L10 0l10 28M4 18h12', B: 'M0 14h11a7 7 0 0 1 0 14H0V0h9a7 7 0 0 1 0 14', C: 'M19 6A10 13 0 1 0 19 22',
    D: 'M0 0h5a14 14 0 0 1 0 28H0z', E: 'M20 0H0v28h20M0 14h14', F: 'M20 0H0v28M0 14h14', G: 'M19 6A10 13 0 1 0 20 18h-8',
    H: 'M0 0v28M20 0v28M0 14h20', I: 'M10 0v28M4 0h12M4 28h12', J: 'M16 0v20a8 8 0 0 1-16 0', K: 'M0 0v28M20 0L0 17M7 12l13 16',
    L: 'M0 0v28h20', M: 'M0 28V0l10 16L20 0v28', N: 'M0 28V0l20 28V0', O: 'M10 0A10 14 0 0 0 10 28A10 14 0 0 0 10 0',
    P: 'M0 28V0h12a7 7 0 0 1 0 14H0', Q: 'M10 0A10 14 0 0 0 10 28A10 14 0 0 0 10 0M13 20l7 8', R: 'M0 28V0h12a7 7 0 0 1 0 14H0M10 14l10 14',
    S: 'M19 4C15-1 1-1 1 7c0 9 18 5 18 14 0 8-14 9-19 3', T: 'M0 0h20M10 0v28', U: 'M0 0v18a10 10 0 0 0 20 0V0', V: 'M0 0l10 28L20 0',
    W: 'M0 0l5 28 5-18 5 18 5-28', X: 'M0 0l20 28M20 0L0 28', Y: 'M0 0l10 14L20 0M10 14v14', Z: 'M0 0h20L0 28h20',
    0: 'M10 0A9 14 0 0 0 10 28A9 14 0 0 0 10 0', 1: 'M4 6l8-6v28', '+': 'M10 6v16M2 14h16', '#': 'M8 2L5 26M16 2l-3 24M2 10h17M1 18h17',
};

// A word of those letters, centred on x, its letters' tops at y
function word(text, x, y, colour) {
    const chars = [...text], size = chars.length > 3 ? 30 : chars.length > 2 ? 36 : 44, s = size / 28, gap = 8 * s;
    const width = chars.length * 20 * s + (chars.length - 1) * gap;
    return chars.map((ch, i) => `<path d="${LETTERS[ch]}" transform="translate(${f(x - width / 2 + i * (20 * s + gap))} ${y}) scale(${f(s)})" ` +
        `fill="none" stroke="${colour}" stroke-width="${f(9 / s)}" stroke-linecap="round" stroke-linejoin="round"/>`).join('');
}

// The words file types are named with, each a page of its own (`label-<word>`)
const LABELS = ['C', 'H', 'C++', 'C#', 'GO', 'RS', 'PY', 'JS', 'TS', 'CSS', 'PHP', 'RB', 'LUA', 'KT', 'JAVA', 'SQL', 'MD',
    'YML', 'TOML', 'XML', 'TEX', 'HS', 'PL', 'CSV', 'SVG', '01'];

// The application glyphs approved on the design canvas of 2026-10-23 (files, settings, terminal, editor, calculator,
// manual, security, control), then the rest of the set drawn the same way
export const DRAW = {
    files: ({ink, soft}) => `
        <path d="M58 86a12 12 0 0 1 12-12h40l16 16h60a12 12 0 0 1 12 12v8H58z" fill="${soft}"/>
        <path d="M54 110a12 12 0 0 1 12-12h124a12 12 0 0 1 12 12v66a14 14 0 0 1-14 14H68a14 14 0 0 1-14-14z" fill="${ink}"/>`,
    settings: ({ink}) => `<path fill-rule="evenodd" d="${gearPath(128, 128, 74, 58, 9, 24)}" fill="${ink}"/>`,
    terminal: ({ink, soft}) => `
        <rect x="52" y="64" width="152" height="128" rx="18" fill="none" stroke="${ink}" stroke-width="12"/>
        ${line('M80 104l26 22-26 22', ink, 14)}${line('M118 152h44', soft, 14)}`,
    editor: c => page(c, line('M86 112h84M86 140h84M86 168h52', c.soft, 11)),
    calculator: ({ink, soft}) => `
        <rect x="70" y="48" width="116" height="160" rx="20" fill="${ink}"/>
        <rect x="86" y="64" width="84" height="32" rx="8" fill="${soft}"/>
        ${[0, 1, 2].map(r => [0, 1, 2].map(c => `<circle cx="${100 + c * 28}" cy="${122 + r * 28}" r="9" fill="${soft}"/>`).join('')).join('')}`,
    manual: ({ink, soft}) => `
        <path d="M128 52l58 33v58l-58 33-58-33V85z" fill="${ink}"/>
        <path d="M128 52l58 33-58 33-58-33z" fill="${soft}" fill-opacity="0.55"/>
        <path d="M186 118v74l-16 12V128z" fill="${ink}"/>`,
    security: ({ink, soft}) => `
        <path d="M128 46l68 26v52c0 44-30 74-68 88-38-14-68-44-68-88V72z" fill="${ink}"/>
        <path d="M128 46l68 26v52c0 44-30 74-68 88z" fill="${soft}" fill-opacity="0.35"/>
        ${line('M100 128l20 20 38-40', soft, 14)}`,
    control: ({ink, soft}) => `
        ${line('M64 84h128M64 128h128M64 172h128', soft, 14)}
        <circle cx="150" cy="84" r="20" fill="${ink}"/><circle cx="98" cy="128" r="20" fill="${ink}"/><circle cx="170" cy="172" r="20" fill="${ink}"/>`,

    // ---- Amethystora's other apps -----------------------------------------------------------------------------------
    // Updates: an arrow coming round, and one coming down into the middle of it
    update: ({ink, soft}) => arcArrow(128, 128, 70, -60, 230, 18, ink) + line('M128 94v54M106 128l22 22 22-22', soft, 16),
    // Logs: a sheet of lines and the lens going over them
    logs: ({ink, soft}) => `<rect x="44" y="44" width="148" height="164" rx="22" fill="${ink}"/>` +
        line('M70 86h96M70 114h72M70 142h40', soft, 11) +
        `<circle cx="160" cy="164" r="30" fill="none" stroke="${soft}" stroke-width="13"/>` + line('M182 186l22 22', soft, 17),
    // Notes: a note with its list ticked
    notes: ({ink, soft}) => `<rect x="52" y="40" width="152" height="176" rx="26" fill="${ink}"/>` +
        [92, 132, 172].map((y, i) => `<rect x="76" y="${y - 14}" width="28" height="28" rx="8" fill="${i < 2 ? soft : 'none'}" stroke="${soft}" stroke-width="7"/>` +
            line(`M120 ${y}h58`, soft, 11)).join(''),
    // Backups: the clock, with time going back round it
    backups: ({ink, soft}) => `<circle cx="128" cy="128" r="54" fill="${ink}"/>` + arcArrow(128, 128, 78, 160, -100, 15, ink) +
        line('M128 98v32l22 14', soft, 13),
    // Vault: a vault door, its dial in four quarters with the door's own colour showing between them (the maintainer's
    // choice of 2026-11-02, option A of the Vault's icon board). The gaps are the body, so the dial is one soft shape of
    // four pieces: no ink over the soft one, which Line would paint over, and no hole cut
    door: ({ink, soft}) => {
        const cx = 128, cy = 124.5, r = 45.5, gap = 7, reach = f(Math.sqrt(r * r - gap * gap));
        const quarter = (sx, sy) => `M${cx + sx * gap} ${f(cy + sy * gap)}V${f(cy + sy * reach)}A${r} ${r} 0 0 ${sx * sy < 0 ? 1 : 0} ${f(cx + sx * reach)} ${f(cy + sy * gap)}Z`;
        return `<rect x="44" y="44" width="168" height="161" rx="38" fill="${ink}"/>` +
            `<rect x="65" y="198" width="28" height="21" rx="7" fill="${ink}"/><rect x="163" y="198" width="28" height="21" rx="7" fill="${ink}"/>` +
            `<path d="${[[1, -1], [1, 1], [-1, 1], [-1, -1]].map(([sx, sy]) => quarter(sx, sy)).join('')}" fill="${soft}"/>`;
    },
    // Community: two speech bubbles
    community: ({ink, soft, over = ink}) =>
        `<path d="M62 52h92a22 22 0 0 1 22 22v50a22 22 0 0 1-22 22h-50l-30 26v-26h-12a22 22 0 0 1-22-22V74a22 22 0 0 1 22-22z" fill="${ink}"/>` +
        `<path d="M122 112h74a22 22 0 0 1 22 22v40a22 22 0 0 1-22 22h-6v24l-28-24h-40a22 22 0 0 1-22-22v-40a22 22 0 0 1 22-22z" fill="${soft}"/>` +
        [136, 158, 180].map(x => `<circle cx="${x}" cy="154" r="7" fill="${over}"/>`).join(''),

    // ---- The system's apps -------------------------------------------------------------------------------------------
    monitor: ({ink, soft}) => `<rect x="40" y="48" width="176" height="128" rx="20" fill="${ink}"/>` +
        `<path d="M116 172h24v24h-24z" fill="${ink}"/><rect x="84" y="192" width="88" height="16" rx="8" fill="${ink}"/>` +
        line('M60 118h30l14-32 22 60 18-40 12 12h40', soft, 12),
    computer: ({ink, soft}) => `<rect x="40" y="48" width="176" height="128" rx="20" fill="${ink}"/>` +
        `<rect x="56" y="64" width="144" height="96" rx="10" fill="${soft}" fill-opacity="0.6"/>` +
        `<path d="M116 172h24v24h-24z" fill="${ink}"/><rect x="84" y="192" width="88" height="16" rx="8" fill="${ink}"/>`,
    drive: ({ink, soft}) => `<rect x="36" y="80" width="184" height="96" rx="24" fill="${ink}"/>` +
        line('M64 128h84', soft, 12) + `<circle cx="186" cy="128" r="10" fill="${soft}"/>`,
    // Tour: a compass
    compass: ({ink, soft, over = ink}) => `<circle cx="128" cy="128" r="80" fill="${ink}"/>` +
        `<path d="M128 60l24 68-24 68-24-68z" fill="${soft}" stroke="${soft}" stroke-width="6" stroke-linejoin="round"/>` +
        `<circle cx="128" cy="128" r="9" fill="${over}"/>`,
    // Tweaks: a wrench
    wrench: ({ink}) => {
        const r = 40, hw = 14, y = 80, top = f(y - Math.sqrt(r * r - hw * hw));
        return `<g transform="rotate(45 128 128)"><path d="M${128 + hw} ${top}A${r} ${r} 0 1 1 ${128 - hw} ${top}V${y - 14}a${hw} ${hw} 0 0 0 ${hw * 2} 0z" fill="${ink}"/>` +
            `<rect x="114" y="104" width="28" height="118" rx="14" fill="${ink}"/></g>`;
    },
    // Parental Controls: a person under a shield
    guardian: ({ink, soft}) => `<circle cx="108" cy="90" r="32" fill="${ink}"/><path d="M44 204a64 64 0 0 1 128 0z" fill="${ink}"/>` +
        `<path d="M184 120l30 11v24c0 22-13 36-30 42-17-6-30-20-30-42v-24z" fill="${soft}"/>`,
    // Firewall: a wall
    wall: ({ink, soft}) => `<rect x="40" y="56" width="176" height="144" rx="20" fill="${ink}"/>` +
        line('M40 104h176M40 152h176', soft, 8) + line('M100 56v48M158 56v48M72 104v48M128 104v48M186 104v48M100 152v48M158 152v48', soft, 8),
    // Colour: a painter's palette
    palette: ({ink, soft}) =>
        `<path d="M128 44c-50 0-88 38-88 84s38 84 84 84c14 0 22-8 22-19 0-6-4-10-4-17 0-9 7-15 15-15h20c24 0 43-19 43-43 0-42-40-74-92-74z" fill="${ink}"/>` +
        [[90, 112], [118, 80], [158, 84], [84, 152]].map(([x, y]) => `<circle cx="${x}" cy="${y}" r="13" fill="${soft}"/>`).join(''),
    // Media sharing: a beacon sending both ways
    broadcast: ({ink, soft}) => `<circle cx="128" cy="116" r="22" fill="${ink}"/><path d="M118 132h20l14 76h-48z" fill="${ink}"/>` +
        line('M92 80a50 50 0 0 0 0 72M164 80a50 50 0 0 1 0 72M66 56a86 86 0 0 0 0 120M190 56a86 86 0 0 1 0 120', soft, 12),
    // Help: a lifebuoy
    lifebuoy: ({ink, soft}) => `<circle cx="128" cy="128" r="62" fill="none" stroke="${ink}" stroke-width="38"/>` +
        `<circle cx="128" cy="128" r="62" fill="none" stroke="${soft}" stroke-width="38" stroke-dasharray="42 55.4" stroke-dashoffset="-27.7"/>`,
    // An app with no icon of its own
    app: ({ink, soft}) => `<rect x="48" y="48" width="160" height="160" rx="42" fill="${ink}"/>` +
        [[80, 80], [136, 80], [80, 136], [136, 136]].map(([x, y]) => `<rect x="${x}" y="${y}" width="40" height="40" rx="12" fill="${soft}"/>`).join(''),

    // ---- GNOME's apps ------------------------------------------------------------------------------------------------
    calendar: ({ink, soft}) => `<rect x="44" y="56" width="168" height="156" rx="26" fill="${ink}"/>` +
        `<path d="M44 82a26 26 0 0 1 26-26h116a26 26 0 0 1 26 26v18H44z" fill="${soft}"/>` +
        `<rect x="82" y="40" width="16" height="36" rx="8" fill="${soft}"/><rect x="158" y="40" width="16" height="36" rx="8" fill="${soft}"/>` +
        [0, 1].map(r => [0, 1, 2].map(c => `<rect x="${70 + c * 44}" y="${124 + r * 40}" width="28" height="26" rx="7" fill="${soft}"/>`).join('')).join(''),
    smile: ({ink, soft}) => `<circle cx="128" cy="128" r="80" fill="${ink}"/>` +
        `<circle cx="100" cy="108" r="12" fill="${soft}"/><circle cx="156" cy="108" r="12" fill="${soft}"/>` + line('M90 146a42 42 0 0 0 76 0', soft, 14),
    remote: ({ink, soft}) => `<rect x="36" y="44" width="140" height="100" rx="16" fill="${ink}"/>` + line('M106 144v18M84 168h44', ink, 13) +
        `<rect x="108" y="108" width="112" height="78" rx="14" fill="${soft}"/>` + line('M164 186v14M144 206h40', soft, 12),
    contact: ({ink, soft}) => `<rect x="36" y="60" width="184" height="136" rx="24" fill="${ink}"/>` +
        `<circle cx="90" cy="112" r="20" fill="${soft}"/><path d="M58 168a32 28 0 0 1 64 0z" fill="${soft}"/>` +
        line('M142 104h52M142 128h52M142 152h34', soft, 11),
    archive: ({ink, soft, over = ink}) => `<rect x="56" y="40" width="144" height="176" rx="26" fill="${ink}"/>` +
        [0, 1, 2, 3, 4].map(i => `<rect x="${i % 2 ? 128 : 114}" y="${54 + i * 16}" width="14" height="10" rx="3" fill="${soft}"/>`).join('') +
        `<rect x="110" y="138" width="36" height="46" rx="11" fill="${soft}"/><rect x="120" y="150" width="16" height="22" rx="5" fill="${over}"/>`,
    chip: ({ink, soft}) => [0, 1, 2, 3].map(i => {
        const o = 82 + i * 26;
        return `<rect x="${o}" y="40" width="14" height="28" rx="5" fill="${soft}"/><rect x="${o}" y="188" width="14" height="28" rx="5" fill="${soft}"/>` +
            `<rect x="40" y="${o}" width="28" height="14" rx="5" fill="${soft}"/><rect x="188" y="${o}" width="28" height="14" rx="5" fill="${soft}"/>`;
    }).join('') + `<rect x="64" y="64" width="128" height="128" rx="22" fill="${ink}"/>` +
        `<rect x="96" y="96" width="64" height="64" rx="12" fill="none" stroke="${soft}" stroke-width="9"/>`,
    photo: ({ink, soft}) => `<rect x="36" y="52" width="184" height="152" rx="26" fill="${ink}"/>` +
        `<path d="M60 180l40-50 30 34 20-22 46 38z" fill="${soft}" stroke="${soft}" stroke-width="6" stroke-linejoin="round"/><circle cx="166" cy="98" r="17" fill="${soft}"/>`,
    map: ({ink, soft}) => `<path d="M40 70l58-24 60 24 58-24v140l-58 24-60-24-58 24z" fill="${ink}" stroke="${ink}" stroke-width="8" stroke-linejoin="round"/>` +
        `<path fill-rule="evenodd" d="M128 72c-21 0-38 17-38 38 0 28 38 62 38 62s38-34 38-62c0-21-17-38-38-38z${circlePath(128, 110, 13)}" fill="${soft}"/>`,
    pages: ({ink, soft}) => `<rect x="100" y="40" width="108" height="144" rx="16" fill="${soft}" transform="rotate(10 154 112)"/>` +
        `<rect x="52" y="64" width="116" height="152" rx="16" fill="${ink}"/>` + line('M76 100h68M76 126h68M76 152h68M76 178h40', soft, 10),
    play: ({ink, soft}) => `<rect x="36" y="56" width="184" height="144" rx="30" fill="${ink}"/>` +
        `<path d="M110 96v64l54-32z" fill="${soft}" stroke="${soft}" stroke-width="12" stroke-linejoin="round"/>`,
    scanner: ({ink, soft}) => `<path d="M52 118l148-46a8 8 0 0 1 10 5l3 10-160 50z" fill="${ink}"/>` +
        `<rect x="40" y="130" width="176" height="76" rx="18" fill="${ink}"/>` + line('M64 158h128', soft, 9) + `<circle cx="188" cy="186" r="7" fill="${soft}"/>`,
    camera: ({ink, soft}) => `<path d="M40 92a18 18 0 0 1 18-18h30l14-20h52l14 20h30a18 18 0 0 1 18 18v98a18 18 0 0 1-18 18H58a18 18 0 0 1-18-18z" fill="${ink}"/>` +
        `<circle cx="128" cy="140" r="36" fill="none" stroke="${soft}" stroke-width="14"/><circle cx="128" cy="140" r="12" fill="${soft}"/><circle cx="184" cy="100" r="8" fill="${soft}"/>`,
    notes2: ({ink}) => `<ellipse cx="92" cy="178" rx="26" ry="20" transform="rotate(-20 92 178)" fill="${ink}"/>` +
        `<ellipse cx="172" cy="160" rx="26" ry="20" transform="rotate(-20 172 160)" fill="${ink}"/>` +
        `<path d="M106 172V74l88-20v100h-14V86l-60 14v72z" fill="${ink}"/>`,
    weather: ({ink, soft}) => `<circle cx="96" cy="88" r="28" fill="${soft}"/>` +
        line([0, 45, 90, 135, 180, 225, 270, 315].map(a => {
            const r = (a * Math.PI) / 180;
            return `M${f(96 + 40 * Math.cos(r))} ${f(88 + 40 * Math.sin(r))}L${f(96 + 52 * Math.cos(r))} ${f(88 + 52 * Math.sin(r))}`;
        }).join(''), soft, 9) +
        `<path d="M82 204h100a38 38 0 0 0 6-75 54 54 0 0 0-101-12 44 44 0 0 0-5 87z" fill="${ink}"/>`,
    pie: ({ink, soft}) => `<path d="M120 140V64a76 76 0 1 0 76 76z" fill="${ink}"/><path d="M134 126V50a76 76 0 0 1 76 76z" fill="${soft}"/>`,
    alarm: ({ink, soft}) => `<circle cx="74" cy="78" r="26" fill="${soft}"/><circle cx="182" cy="78" r="26" fill="${soft}"/>` +
        line('M84 196l-14 16M172 196l14 16', soft, 12) + `<circle cx="128" cy="138" r="70" fill="${ink}"/>` + line('M128 98v42l26 18', soft, 13),
    letter: ({ink, soft}) => `<path fill-rule="evenodd" d="M110 48h36l62 160h-36l-14-38H98l-14 38H48zM128 92l-20 52h40z" fill="${ink}"/>` +
        line('M98 152h60', soft, 8),
    code: ({ink, soft}) => `<rect x="44" y="44" width="168" height="168" rx="40" fill="${ink}"/>` +
        line('M106 100l-28 28 28 28M150 100l28 28-28 28', soft, 14) + line('M138 90l-20 76', soft, 12),

    // ---- Places --------------------------------------------------------------------------------------------------
    folder: c => folder(c),
    ...Object.fromEntries(Object.entries(EMBLEM).map(([name, draw]) => [`folder-${name}`, c => folder(c, draw(c.soft))])),
    home: ({ink, soft}) => `<path d="M48 122l80-70 80 70v78a14 14 0 0 1-14 14H62a14 14 0 0 1-14-14z" fill="${ink}" stroke="${ink}" stroke-width="8" stroke-linejoin="round"/>` +
        `<rect x="108" y="150" width="40" height="64" rx="10" fill="${soft}"/>`,
    trash: ({ink, soft}) => `<path d="M70 94h116l-10 106a14 14 0 0 1-14 12H94a14 14 0 0 1-14-12z" fill="${ink}"/>` +
        `<rect x="56" y="66" width="144" height="20" rx="10" fill="${soft}"/>` + line('M108 64v-8a8 8 0 0 1 8-8h24a8 8 0 0 1 8 8v8', soft, 8) +
        line('M106 118v70M128 118v70M150 118v70', soft, 9),
    'trash-full': ({ink, soft}) => `<path d="M70 104h116l-10 96a14 14 0 0 1-14 12H94a14 14 0 0 1-14-12z" fill="${ink}"/>` +
        `<circle cx="102" cy="84" r="20" fill="${soft}"/><circle cx="142" cy="78" r="24" fill="${soft}"/><circle cx="170" cy="92" r="14" fill="${soft}"/>` +
        `<rect x="66" y="52" width="140" height="20" rx="10" fill="${soft}" transform="rotate(-14 200 62)"/>` + line('M106 128v62M128 128v62M150 128v62', soft, 9),
    server: ({ink, soft}) => [48, 106, 164].map(y => `<rect x="52" y="${y}" width="152" height="46" rx="14" fill="${ink}"/>` +
        line(`M76 ${y + 23}h44`, soft, 9) + `<circle cx="178" cy="${y + 23}" r="8" fill="${soft}"/>`).join(''),
    bookmark: ({ink, soft}) => `<path d="M80 40h96a14 14 0 0 1 14 14v164l-62-40-62 40V54a14 14 0 0 1 14-14z" fill="${ink}"/>` +
        `<path d="${starPath(128, 108, 30, 13)}" fill="${soft}" stroke="${soft}" stroke-width="4" stroke-linejoin="round"/>`,
    avatar: ({ink, soft}) => `<circle cx="128" cy="128" r="84" fill="${ink}"/><circle cx="128" cy="104" r="32" fill="${soft}"/>` +
        `<path d="M74 190a54 48 0 0 1 108 0 84 84 0 0 1-108 0z" fill="${soft}"/>`,

    // ---- File types --------------------------------------------------------------------------------------------------
    text: c => page(c, line('M86 112h84M86 140h84M86 168h52', c.soft, 11)),
    script: c => page(c, line('M106 122l-22 24 22 24M150 122l22 24-22 24', c.soft, 12) + line('M136 114l-16 64', c.soft, 10)),
    html: c => page(c, `<circle cx="128" cy="146" r="36" fill="none" stroke="${c.soft}" stroke-width="9"/>` +
        `<ellipse cx="128" cy="146" rx="15" ry="36" fill="none" stroke="${c.soft}" stroke-width="8"/>` + line('M92 146h72', c.soft, 8)),
    image: c => page(c, `<path d="M84 190l32-40 24 28 14-16 32 28z" fill="${c.soft}" stroke="${c.soft}" stroke-width="5" stroke-linejoin="round"/><circle cx="150" cy="114" r="14" fill="${c.soft}"/>`),
    audio: c => page(c, `<circle cx="108" cy="178" r="14" fill="${c.soft}"/><circle cx="154" cy="166" r="14" fill="${c.soft}"/>` +
        `<path d="M114 178v-58l48-12v58h-8v-44l-32 8v48z" fill="${c.soft}"/>`),
    video: c => page(c, `<path d="M110 118v58l48-29z" fill="${c.soft}" stroke="${c.soft}" stroke-width="9" stroke-linejoin="round"/>`),
    pdf: c => page(c, `<rect x="84" y="100" width="88" height="48" rx="9" fill="${c.soft}"/>` + line('M86 168h84M86 190h56', c.soft, 9)),
    document: c => page(c, line('M86 104h56', c.soft, 15) + line('M86 132h84M86 156h84M86 180h60', c.soft, 9)),
    spreadsheet: c => page(c, `<rect x="84" y="102" width="88" height="90" rx="9" fill="none" stroke="${c.soft}" stroke-width="8"/>` +
        line('M84 132h88M84 162h88M114 102v90M144 102v90', c.soft, 6)),
    presentation: c => page(c, `<rect x="88" y="150" width="20" height="42" rx="5" fill="${c.soft}"/><rect x="118" y="124" width="20" height="68" rx="5" fill="${c.soft}"/><rect x="148" y="104" width="20" height="88" rx="5" fill="${c.soft}"/>`),
    drawing: c => page(c, `<circle cx="108" cy="124" r="22" fill="${c.soft}"/><path d="M152 106l26 46h-52z" fill="${c.soft}" stroke="${c.soft}" stroke-width="4" stroke-linejoin="round"/><rect x="94" y="160" width="76" height="30" rx="8" fill="${c.soft}"/>`),
    font: c => page(c, `<path fill-rule="evenodd" d="M116 100h24l34 92h-24l-7-20h-34l-7 20H78zM128 128l-10 28h20z" fill="${c.soft}"/>`),
    template: c => page(c, `<rect x="88" y="104" width="80" height="82" rx="9" fill="none" stroke="${c.soft}" stroke-width="8" stroke-dasharray="12 9"/>`),
    package: ({ink, soft}) => `<rect x="48" y="92" width="160" height="120" rx="16" fill="${ink}"/>` +
        `<rect x="40" y="56" width="176" height="44" rx="14" fill="${ink}"/>` + line('M48 104h160', soft, 6) +
        `<rect x="114" y="56" width="28" height="156" fill="${soft}"/>`,
    addon: ({ink}) => `<path d="M62 78h32a20 20 0 1 1 40 0h40v40a20 20 0 1 1 0 40v40H62v-40a20 20 0 1 0 0-40z" fill="${ink}"/>`,
    // GNOME's Extensions: the add-ons a machine has, each switched on or off
    extensions: ({ink, soft}) => `<path d="M58 84h32a26 26 0 1 1 36 0h32a14 14 0 0 1 14 14v32a26 26 0 1 1 0 36v32a14 14 0 0 1-14 14H58a14 14 0 0 1-14-14V98a14 14 0 0 1 14-14z" fill="${ink}"/>` +
        `<rect x="66" y="126" width="84" height="44" rx="22" fill="none" stroke="${soft}" stroke-width="10"/><circle cx="128" cy="148" r="12" fill="${soft}"/>`,
    // Extension Manager: a bag to carry add-ons home in, from where they are published
    'extension-store': ({ink, soft}) => line('M94 104V84a34 34 0 0 1 68 0v20', soft, 13) +
        `<path d="M58 96h140l-10 104a16 16 0 0 1-16 14H84a16 16 0 0 1-16-14z" fill="${ink}" stroke="${ink}" stroke-width="6" stroke-linejoin="round"/>` +
        `<path d="M100 134h15a13 13 0 1 1 26 0h15v15a13 13 0 1 1 0 26v15h-56z" fill="${soft}"/>`,

    // ---- Devices -----------------------------------------------------------------------------------------------------
    laptop: ({ink, soft}) => `<rect x="56" y="52" width="144" height="104" rx="16" fill="${ink}"/>` +
        `<rect x="70" y="66" width="116" height="76" rx="8" fill="${soft}" fill-opacity="0.6"/>` +
        `<path d="M32 168h192l-10 24a14 14 0 0 1-13 8H55a14 14 0 0 1-13-8z" fill="${ink}"/>`,
    phone: ({ink, soft}) => `<rect x="74" y="36" width="108" height="184" rx="26" fill="${ink}"/>` +
        `<rect x="88" y="60" width="80" height="132" rx="10" fill="${soft}" fill-opacity="0.6"/><rect x="112" y="46" width="32" height="7" rx="3.5" fill="${soft}"/>` +
        `<rect x="108" y="202" width="40" height="7" rx="3.5" fill="${soft}"/>`,
    // The phone link: a phone, and the waves between it and this computer
    "phone-link": ({ink, soft}) => `<rect x="58" y="40" width="104" height="176" rx="26" fill="${ink}"/>` +
        `<rect x="72" y="64" width="76" height="116" rx="10" fill="${soft}" fill-opacity="0.45"/><rect x="92" y="196" width="36" height="7" rx="3.5" fill="${soft}"/>` +
        line("M184 104a34 34 0 0 1 0 48", soft, 12) + line("M204 84a62 62 0 0 1 0 88", soft, 12),
    usb: ({ink, soft, over = ink}) => `<rect x="104" y="40" width="48" height="56" rx="6" fill="${soft}"/>` +
        `<rect x="114" y="54" width="10" height="10" rx="2" fill="${over}"/><rect x="132" y="54" width="10" height="10" rx="2" fill="${over}"/>` +
        `<rect x="88" y="92" width="80" height="124" rx="20" fill="${ink}"/>` + line('M114 150h28', soft, 9),
    card: ({ink, soft}) => `<path d="M72 40h84l44 44v116a16 16 0 0 1-16 16H72a16 16 0 0 1-16-16V56a16 16 0 0 1 16-16z" fill="${ink}"/>` +
        [0, 1, 2, 3].map(i => `<rect x="${78 + i * 20}" y="56" width="11" height="34" rx="4" fill="${soft}"/>`).join(''),
    disc: ({ink, soft}) => `<path fill-rule="evenodd" d="${circlePath(128, 128, 84)}${circlePath(128, 128, 16)}" fill="${ink}"/>` +
        `<circle cx="128" cy="128" r="46" fill="none" stroke="${soft}" stroke-width="10" stroke-opacity="0.8"/>` + line('M78 90a64 64 0 0 1 40-26', soft, 9),
    webcam: ({ink, soft}) => `<circle cx="128" cy="104" r="64" fill="${ink}"/><path d="M116 164h24v28h-24z" fill="${ink}"/>` +
        line('M84 204h88', ink, 16) + `<circle cx="128" cy="104" r="26" fill="${soft}"/><circle cx="168" cy="70" r="7" fill="${soft}"/>`,
    printer: ({ink, soft}) => `<rect x="76" y="44" width="104" height="60" rx="8" fill="${soft}"/>` +
        `<rect x="36" y="92" width="184" height="88" rx="20" fill="${ink}"/>` +
        `<rect x="70" y="156" width="116" height="60" rx="8" fill="${soft}"/>` + `<circle cx="190" cy="120" r="8" fill="${soft}"/>`,
    keyboard: ({ink, soft}) => `<rect x="32" y="72" width="192" height="112" rx="22" fill="${ink}"/>` +
        [0, 1].map(r => [0, 1, 2, 3, 4, 5].map(k => `<rect x="${54 + k * 26}" y="${92 + r * 26}" width="18" height="18" rx="5" fill="${soft}"/>`).join('')).join('') +
        `<rect x="80" y="146" width="96" height="18" rx="6" fill="${soft}"/>`,
    mouse: ({ink, soft}) => `<rect x="78" y="40" width="100" height="176" rx="50" fill="${ink}"/>` +
        `<rect x="120" y="70" width="16" height="34" rx="8" fill="${soft}"/>` + line('M128 40v22', soft, 6),
    gamepad: ({ink, soft}) => `<g transform="translate(128 128) scale(0.88) translate(-128 -128)">` +
        `<path d="M72 74h112c28 0 42 24 46 62l6 48c3 24-22 36-39 19l-27-27H86l-27 27c-17 17-42 5-39-19l6-48c4-38 18-62 46-62z" fill="${ink}"/>` +
        line('M78 112v38M59 131h38', soft, 13) + `<circle cx="172" cy="116" r="11" fill="${soft}"/><circle cx="196" cy="140" r="11" fill="${soft}"/></g>`,
    tablet: ({ink, soft}) => `<rect x="36" y="60" width="156" height="148" rx="22" fill="${ink}"/>` +
        `<path d="M206 46l18 18-84 84-28 10 10-28z" fill="${soft}" stroke="${soft}" stroke-width="4" stroke-linejoin="round"/>`,
    headphones: ({ink, soft}) => line('M60 160v-28a68 68 0 0 1 136 0v28', ink, 18) +
        `<rect x="40" y="136" width="44" height="76" rx="20" fill="${ink}"/><rect x="172" y="136" width="44" height="76" rx="20" fill="${ink}"/>` +
        `<rect x="54" y="150" width="16" height="48" rx="8" fill="${soft}"/><rect x="186" y="150" width="16" height="48" rx="8" fill="${soft}"/>`,
    speaker: ({ink, soft}) => `<rect x="68" y="36" width="120" height="184" rx="24" fill="${ink}"/>` +
        `<circle cx="128" cy="80" r="16" fill="${soft}"/><circle cx="128" cy="158" r="36" fill="none" stroke="${soft}" stroke-width="12"/><circle cx="128" cy="158" r="10" fill="${soft}"/>`,
    microphone: ({ink, soft}) => `<rect x="94" y="36" width="68" height="116" rx="34" fill="${ink}"/>` +
        line('M108 72h40M108 94h40M108 116h40', soft, 8) + line('M66 118a62 62 0 0 0 124 0M128 182v26M98 212h60', ink, 14),
    plug: ({ink, soft}) => `<rect x="100" y="44" width="14" height="44" rx="4" fill="${soft}"/><rect x="142" y="44" width="14" height="44" rx="4" fill="${soft}"/>` +
        `<path d="M76 84h104v56a36 36 0 0 1-36 36h-32a36 36 0 0 1-36-36z" fill="${ink}"/>` + line('M128 176v36', ink, 20),

    // ---- Categories --------------------------------------------------------------------------------------------------
    globe: ({ink, soft}) => `<circle cx="128" cy="128" r="80" fill="${ink}"/>` +
        `<ellipse cx="128" cy="128" rx="32" ry="80" fill="none" stroke="${soft}" stroke-width="9"/>` + line('M48 128h160M62 90h132M62 166h132', soft, 8),
    flask: ({ink, soft}) => `<path d="M106 40h44v18h-4v42l54 88a16 16 0 0 1-14 24H70a16 16 0 0 1-14-24l54-88V58h-4z" fill="${ink}" stroke="${ink}" stroke-width="6" stroke-linejoin="round"/>` +
        `<path d="M84 160h88l18 30a8 8 0 0 1-7 12H73a8 8 0 0 1-7-12z" fill="${soft}"/><circle cx="118" cy="136" r="7" fill="${soft}"/><circle cx="140" cy="120" r="5" fill="${soft}"/>`,
    cap: ({ink, soft}) => `<g transform="translate(128 128) scale(0.92) translate(-128 -128)"><path d="M128 52l100 46-100 46-100-46z" fill="${ink}" stroke="${ink}" stroke-width="8" stroke-linejoin="round"/>` +
        `<path d="M72 126v44c0 18 25 32 56 32s56-14 56-32v-44l-56 26z" fill="${ink}"/>` + line('M210 104v58', soft, 8) + `<circle cx="210" cy="170" r="11" fill="${soft}"/></g>`,

    // ---- Other makers' apps, by what they do --------------------------------------------------------------------------
    mail: ({ink, soft}) => `<rect x="36" y="64" width="184" height="128" rx="22" fill="${ink}"/>` + line('M58 88l70 52 70-52', soft, 13),
    chat: ({ink, soft}) =>
        `<path d="M64 52h128a24 24 0 0 1 24 24v72a24 24 0 0 1-24 24h-60l-42 34v-34H64a24 24 0 0 1-24-24V76a24 24 0 0 1 24-24z" fill="${ink}"/>` +
        [92, 128, 164].map(x => `<circle cx="${x}" cy="112" r="12" fill="${soft}"/>`).join(''),
    feed: ({ink, soft}) => `<rect x="44" y="44" width="168" height="168" rx="40" fill="${ink}"/>` +
        `<circle cx="92" cy="164" r="15" fill="${soft}"/>` + line('M84 124a48 48 0 0 1 48 48M84 84a88 88 0 0 1 88 88', soft, 15),
    download: ({ink, soft}) => `<path d="M40 148h40l14 20h68l14-20h40v40a20 20 0 0 1-20 20H60a20 20 0 0 1-20-20z" fill="${ink}"/>` +
        line('M128 44v84M96 98l32 32 32-32', soft, 18),
    key: ({ink, soft}) => `<g transform="rotate(-35 128 128)"><circle cx="86" cy="128" r="44" fill="${ink}"/>` +
        `<rect x="116" y="116" width="104" height="24" rx="8" fill="${ink}"/><rect x="168" y="132" width="18" height="28" rx="5" fill="${ink}"/>` +
        `<rect x="198" y="132" width="18" height="36" rx="5" fill="${ink}"/><circle cx="76" cy="128" r="15" fill="${soft}"/></g>`,
    book: ({ink, soft}) => `<path d="M124 70c-24-14-52-18-80-14v132c28-4 56 0 80 14z" fill="${ink}"/><path d="M132 70c24-14 52-18 80-14v132c-28-4-56 0-80 14z" fill="${ink}"/>` +
        line('M64 90c16-2 30 0 42 6M64 116c16-2 30 0 42 6M64 142c16-2 30 0 42 6M150 96c12-6 26-8 42-6M150 122c12-6 26-8 42-6M150 148c12-6 26-8 42-6', soft, 8),
    brush: ({ink, soft}) => `<g transform="rotate(40 128 128)"><rect x="114" y="34" width="28" height="104" rx="14" fill="${ink}"/>` +
        `<rect x="106" y="132" width="44" height="24" rx="6" fill="${soft}"/><path d="M106 160h44v18c0 24-22 44-22 44s-22-20-22-44z" fill="${ink}"/></g>`,
    screenshot: ({ink, soft}) => line('M48 98V68a20 20 0 0 1 20-20h30M158 48h30a20 20 0 0 1 20 20v30M208 158v30a20 20 0 0 1-20 20h-30M98 208H68a20 20 0 0 1-20-20v-30', ink, 18) +
        `<path d="M82 176l32-40 22 26 14-16 26 30z" fill="${soft}" stroke="${soft}" stroke-width="5" stroke-linejoin="round"/><circle cx="156" cy="100" r="13" fill="${soft}"/>`,
    record: ({ink, soft}) => `<circle cx="128" cy="128" r="80" fill="${ink}"/><circle cx="128" cy="128" r="38" fill="${soft}"/>`,
    eyedropper: ({ink, soft}) => `<g transform="rotate(45 128 128)"><rect x="106" y="34" width="44" height="60" rx="22" fill="${ink}"/>` +
        `<rect x="92" y="88" width="72" height="18" rx="9" fill="${ink}"/><path d="M114 106h28v82l-14 30-14-30z" fill="${soft}" stroke="${soft}" stroke-width="4" stroke-linejoin="round"/></g>`,
    translate: ({ink, soft, over = ink}) => `<path d="M60 44h80a20 20 0 0 1 20 20v62a20 20 0 0 1-20 20H96l-30 24v-24h-6a20 20 0 0 1-20-20V64a20 20 0 0 1 20-20z" fill="${ink}"/>` +
        word('A', 100, 66, soft) + `<path d="M136 112h64a20 20 0 0 1 20 20v52a20 20 0 0 1-20 20h-4v22l-26-22h-34a20 20 0 0 1-20-20v-52a20 20 0 0 1 20-20z" fill="${soft}"/>` +
        line('M140 146h52M140 172h34', over, 10),
    vm: ({ink, soft, over = ink}) => `<rect x="40" y="48" width="176" height="128" rx="20" fill="${ink}"/>` +
        `<path d="M116 172h24v24h-24z" fill="${ink}"/><rect x="84" y="192" width="88" height="16" rx="8" fill="${ink}"/>` +
        `<path d="M128 66l38 20v44l-38 20-38-20V86z" fill="${soft}"/>` + line('M90 86l38 20 38-20M128 106v44', over, 6),
    containers: ({ink, soft, over = ink}) => `<rect x="38" y="130" width="86" height="80" rx="14" fill="${ink}"/><rect x="132" y="130" width="86" height="80" rx="14" fill="${ink}"/>` +
        line('M58 156h46M152 156h46', soft, 9) + `<rect x="85" y="44" width="86" height="80" rx="14" fill="${soft}"/>` + line('M105 70h46', over, 9),
    store: ({ink, soft}) => line('M94 104V84a34 34 0 0 1 68 0v20', soft, 13) +
        `<path d="M58 96h140l-10 104a16 16 0 0 1-16 14H84a16 16 0 0 1-16-14z" fill="${ink}" stroke="${ink}" stroke-width="6" stroke-linejoin="round"/>` +
        `<circle cx="94" cy="128" r="9" fill="${soft}"/><circle cx="162" cy="128" r="9" fill="${soft}"/>`,
    broom: ({ink, soft}) => `<g transform="rotate(28 128 128) translate(128 128) scale(0.9) translate(-128 -128)"><rect x="120" y="28" width="16" height="104" rx="8" fill="${ink}"/>` +
        `<path d="M92 128h72l22 92H70z" fill="${ink}" stroke="${ink}" stroke-width="8" stroke-linejoin="round"/>` +
        line('M86 152h84', soft, 8) + line('M102 178l-5 32M128 178v32M154 178l5 32', soft, 7) + '</g>',
    search: ({ink, soft}) => `<circle cx="112" cy="112" r="44" fill="${soft}" fill-opacity="0.45"/>` +
        `<circle cx="112" cy="112" r="56" fill="none" stroke="${ink}" stroke-width="22"/>` + line('M154 154l46 46', ink, 28) +
        line('M84 104a30 30 0 0 1 22-24', soft, 10),
    toolbox: ({ink, soft, over = ink}) => line('M96 96V74a10 10 0 0 1 10-10h44a10 10 0 0 1 10 10v22', ink, 12) +
        `<rect x="36" y="94" width="184" height="114" rx="18" fill="${ink}"/>` +
        `<path d="M36 112a18 18 0 0 1 18-18h148a18 18 0 0 1 18 18v18H36z" fill="${soft}"/><rect x="112" y="118" width="32" height="24" rx="6" fill="${over}"/>`,
    film: ({ink, soft}) => `<rect x="48" y="40" width="160" height="176" rx="18" fill="${ink}"/>` +
        [0, 1, 2, 3, 4].map(i => `<rect x="60" y="${56 + i * 32}" width="14" height="16" rx="4" fill="${soft}"/><rect x="182" y="${56 + i * 32}" width="14" height="16" rx="4" fill="${soft}"/>`).join('') +
        `<rect x="88" y="62" width="80" height="58" rx="8" fill="${soft}"/><rect x="88" y="136" width="80" height="58" rx="8" fill="${soft}"/>`,
    radio: ({ink, soft}) => line('M82 84L176 42', ink, 10) + `<rect x="36" y="80" width="184" height="128" rx="22" fill="${ink}"/>` +
        `<circle cx="94" cy="144" r="32" fill="${soft}"/><rect x="144" y="112" width="52" height="14" rx="7" fill="${soft}"/>` +
        `<circle cx="154" cy="168" r="11" fill="${soft}"/><circle cx="186" cy="168" r="11" fill="${soft}"/>`,
    equalizer: ({ink, soft}) => `<rect x="44" y="44" width="168" height="168" rx="40" fill="${ink}"/>` +
        line('M88 76v104M128 76v104M168 76v104', soft, 8) +
        [[88, 150], [128, 100], [168, 132]].map(([x, y]) => `<circle cx="${x}" cy="${y}" r="15" fill="${soft}"/>`).join(''),
    access: ({ink, soft}) => `<circle cx="128" cy="128" r="84" fill="${ink}"/><circle cx="128" cy="78" r="15" fill="${soft}"/>` +
        line('M80 108c32 8 64 8 96 0M128 112v42M128 154l-24 42M128 154l24 42', soft, 13),
    battery: ({ink, soft}) => `<rect x="36" y="76" width="164" height="104" rx="20" fill="${ink}"/><rect x="198" y="106" width="22" height="44" rx="7" fill="${ink}"/>` +
        `<rect x="56" y="96" width="96" height="64" rx="10" fill="${soft}"/>`,
    wifi: ({ink, soft}) => line([44, 84, 124].map(r => `M${f(128 - r * 0.72)} ${f(196 - r * 0.69)}A${r} ${r} 0 0 1 ${f(128 + r * 0.72)} ${f(196 - r * 0.69)}`).join(''), ink, 20) +
        `<circle cx="128" cy="196" r="18" fill="${soft}"/>`,
    touchpad: ({ink, soft}) => `<rect x="36" y="56" width="184" height="144" rx="24" fill="${ink}"/>` + line('M56 158h144M128 158v26', soft, 8),
    floppy: ({ink, soft, over = ink}) => `<path d="M56 40h120l40 40v120a16 16 0 0 1-16 16H56a16 16 0 0 1-16-16V56a16 16 0 0 1 16-16z" fill="${ink}"/>` +
        `<rect x="82" y="40" width="84" height="54" rx="6" fill="${soft}"/><rect x="136" y="52" width="16" height="30" rx="4" fill="${over}"/>` +
        `<rect x="70" y="130" width="116" height="70" rx="10" fill="${soft}"/>`,
    player: ({ink, soft}) => `<rect x="70" y="36" width="116" height="184" rx="24" fill="${ink}"/><rect x="86" y="54" width="84" height="70" rx="8" fill="${soft}"/>` +
        `<path d="M118 150v44l34-22z" fill="${soft}" stroke="${soft}" stroke-width="8" stroke-linejoin="round"/>`,
    cards: ({ink, soft}) => `<rect x="44" y="52" width="104" height="148" rx="16" fill="${soft}" transform="rotate(-14 96 126)"/>` +
        `<rect x="104" y="60" width="104" height="148" rx="16" fill="${ink}"/>` +
        `<path d="M156 168c-28-18-36-32-36-44a18 18 0 0 1 36-6 18 18 0 0 1 36 6c0 12-8 26-36 44z" fill="${soft}"/>`,
    chess: ({ink, soft}) => `<circle cx="128" cy="76" r="30" fill="${ink}"/><rect x="90" y="108" width="76" height="18" rx="9" fill="${ink}"/>` +
        `<path d="M104 124h48l14 64H90z" fill="${ink}"/><rect x="62" y="184" width="132" height="30" rx="12" fill="${ink}"/><circle cx="118" cy="66" r="9" fill="${soft}"/>` +
        line('M84 199h88', soft, 6),
    branch: ({ink, soft}) => line('M88 76v104M88 156c0-44 80-28 80-72', soft, 13) +
        `<circle cx="88" cy="64" r="24" fill="${ink}"/><circle cx="88" cy="192" r="24" fill="${ink}"/><circle cx="168" cy="76" r="24" fill="${ink}"/>`,
    notepad: ({ink, soft}) => `<rect x="56" y="56" width="144" height="160" rx="20" fill="${ink}"/>` +
        line('M88 40v32M128 40v32M168 40v32', soft, 12) + line('M80 110h96M80 138h96M80 166h60', soft, 10),
    tasks: ({ink, soft}) => `<circle cx="128" cy="128" r="84" fill="${ink}"/>` + line('M88 130l28 28 54-60', soft, 18),
    toggles: ({ink, soft}) => `<rect x="40" y="56" width="176" height="64" rx="32" fill="${ink}"/><circle cx="184" cy="88" r="22" fill="${soft}"/>` +
        `<rect x="40" y="136" width="176" height="64" rx="32" fill="${ink}" fill-opacity="0.6"/><circle cx="72" cy="168" r="22" fill="${soft}"/>`,
    // Sending something across, whatever carries it: never the Bluetooth mark
    transfer: ({ink, soft}) => `<rect x="44" y="44" width="168" height="168" rx="40" fill="${ink}"/>` +
        line('M80 102h88M146 78l24 24-24 24', soft, 14) + line('M176 154H88M110 130l-24 24 24 24', soft, 14),
    vpn: ({ink, soft}) => `<rect x="44" y="44" width="168" height="168" rx="40" fill="${ink}"/>` + word('VPN', 128, 110, soft),
    lock: ({ink, soft}) => line('M88 112V84a40 40 0 0 1 80 0v28', ink, 18) + `<rect x="56" y="108" width="144" height="104" rx="24" fill="${ink}"/>` +
        `<circle cx="128" cy="150" r="14" fill="${soft}"/><rect x="120" y="150" width="16" height="34" rx="6" fill="${soft}"/>`,
    bell: ({ink, soft}) => `<path d="M128 40a14 14 0 0 1 14 14v8c28 6 46 30 46 58v36l20 28H48l20-28v-36c0-28 18-52 46-58v-8a14 14 0 0 1 14-14z" fill="${ink}"/>` +
        `<path d="M102 192h52a26 26 0 0 1-52 0z" fill="${soft}"/>`,
    // Windows over each other: two panes, never four
    stacked: ({ink, soft}) => `<rect x="88" y="44" width="128" height="104" rx="18" fill="${soft}"/>` +
        `<rect x="40" y="92" width="144" height="120" rx="20" fill="${ink}"/>` + line('M62 124h100', soft, 10),
    smartcard: ({ink, soft}) => `<rect x="36" y="64" width="184" height="128" rx="20" fill="${ink}"/><rect x="60" y="92" width="52" height="46" rx="9" fill="${soft}"/>` +
        line('M136 104h60M136 130h60M60 164h96', soft, 9),
    modem: ({ink, soft}) => line('M78 124L62 50M178 124l16-74', ink, 12) + `<rect x="40" y="118" width="176" height="84" rx="20" fill="${ink}"/>` +
        [76, 110, 144].map(x => `<circle cx="${x}" cy="160" r="10" fill="${soft}"/>`).join('') + line('M170 160h20', soft, 10),
    tape: ({ink, soft}) => `<rect x="36" y="60" width="184" height="136" rx="18" fill="${ink}"/>` +
        `<circle cx="92" cy="118" r="18" fill="none" stroke="${soft}" stroke-width="10"/><circle cx="164" cy="118" r="18" fill="none" stroke="${soft}" stroke-width="10"/>` +
        `<path d="M84 196l12-26h64l12 26z" fill="${soft}"/>`,
    dialpad: ({ink, soft}) => `<rect x="64" y="36" width="128" height="184" rx="26" fill="${ink}"/>` +
        [0, 1, 2, 3].map(r => [0, 1, 2].map(c => `<circle cx="${96 + c * 32}" cy="${72 + r * 37}" r="10" fill="${soft}"/>`).join('')).join(''),
    ereader: ({ink, soft}) => `<rect x="66" y="36" width="124" height="184" rx="22" fill="${ink}"/>` +
        line('M92 76h72M92 104h72M92 132h72M92 160h40', soft, 10) + `<circle cx="128" cy="196" r="8" fill="${soft}"/>`,
    'shield-ok': ({ink, soft}) => `<path d="M128 46l68 26v52c0 44-30 74-68 88-38-14-68-44-68-88V72z" fill="${ink}"/>` + line('M98 126l22 22 40-44', soft, 17),
    'image-missing': ({ink, soft}) => `<rect x="36" y="52" width="184" height="152" rx="26" fill="${ink}"/>` +
        `<path d="M60 180l40-50 30 34 20-22 46 38z" fill="${soft}" fill-opacity="0.4"/>` + line('M66 186L190 70', soft, 15),
    'image-loading': ({ink, soft}) => `<rect x="36" y="52" width="184" height="152" rx="26" fill="${ink}"/>` +
        [88, 128, 168].map(x => `<circle cx="${x}" cy="128" r="14" fill="${soft}"/>`).join(''),

    // ---- Dialogs -----------------------------------------------------------------------------------------------------
    info: ({ink, soft}) => `<circle cx="128" cy="128" r="84" fill="${ink}"/><circle cx="128" cy="86" r="13" fill="${soft}"/>` + line('M128 118v62', soft, 20),
    warning: ({ink, soft}) => `<path d="M128 52l84 146H44z" fill="${ink}" stroke="${ink}" stroke-width="24" stroke-linejoin="round"/>` +
        line('M128 104v48', soft, 18) + `<circle cx="128" cy="180" r="11" fill="${soft}"/>`,
    error: ({ink, soft}) => `<circle cx="128" cy="128" r="84" fill="${ink}"/>` + line('M98 98l60 60M158 98l-60 60', soft, 20),
    question: ({ink, soft}) => `<circle cx="128" cy="128" r="84" fill="${ink}"/>` +
        line('M100 106a28 28 0 1 1 40 25c-8 4-12 10-12 18v4', soft, 18) + `<circle cx="128" cy="184" r="11" fill="${soft}"/>`,
    'shield-alert': ({ink, soft}) => `<path d="M128 46l68 26v52c0 44-30 74-68 88-38-14-68-44-68-88V72z" fill="${ink}"/>` +
        line('M128 92v52', soft, 18) + `<circle cx="128" cy="172" r="11" fill="${soft}"/>`,

    // ---- More file types ---------------------------------------------------------------------------------------------
    zip: c => page(c, [0, 1, 2, 3, 4].map(i => `<rect x="${i % 2 ? 128 : 114}" y="${76 + i * 14}" width="14" height="10" rx="3" fill="${c.soft}"/>`).join('') +
        `<rect x="110" y="152" width="36" height="44" rx="10" fill="${c.soft}"/>`),
    'key-file': c => page(c, `<circle cx="110" cy="150" r="24" fill="${c.soft}"/>` + line('M130 150h42M160 150v16M172 150v12', c.soft, 11)),
    certificate: c => page(c, `<circle cx="128" cy="136" r="28" fill="${c.soft}"/><path d="M112 156l-10 40 26-14 26 14-10-40z" fill="${c.soft}"/>`),
    'book-file': c => page(c, `<path d="M126 124c-12-8-26-10-42-8v64c16-2 30 0 42 8zM130 124c12-8 26-10 42-8v64c-16-2-30 0-42 8z" fill="${c.soft}"/>`),
    'mail-file': c => page(c, `<rect x="82" y="118" width="92" height="64" rx="10" fill="${c.soft}"/>` + line('M94 130l34 24 34-24', c.over ?? c.ink, 8)),
    'download-file': c => page(c, line('M128 104v56M104 138l24 24 24-24M96 188h64', c.soft, 12)),
    blank: c => page(c),
    'colour-profile': c => page(c, [[108, 128], [148, 128], [128, 164]].map(([x, y]) => `<circle cx="${x}" cy="${y}" r="25" fill="${c.soft}" fill-opacity="0.65"/>`).join('')),
    symlink: c => page(c, line('M92 186v-20a32 32 0 0 1 32-32h36', c.soft, 12) + line('M140 110l24 24-24 24', c.soft, 12)),
    'model-file': c => page(c, `<path d="M128 102l42 23v47l-42 23-42-23v-47z" fill="none" stroke="${c.soft}" stroke-width="8" stroke-linejoin="round"/>` +
        line('M88 126l40 22 40-22M128 150v43', c.soft, 7)),
    database: ({ink, soft}) => `<path d="M56 64c0-14 32-24 72-24s72 10 72 24v128c0 14-32 24-72 24s-72-10-72-24z" fill="${ink}"/>` +
        `<ellipse cx="128" cy="64" rx="72" ry="24" fill="${soft}"/>` + line('M56 112c0 14 32 24 72 24s72-10 72-24M56 156c0 14 32 24 72 24s72-10 72-24', soft, 8),
    json: c => page(c, line('M110 106c-12 0-14 6-14 16v10c0 6-4 10-10 10 6 0 10 4 10 10v10c0 10 2 16 14 16M146 106c12 0 14 6 14 16v10c0 6 4 10 10 10-6 0-10 4-10 10v10c0 10-2 16-14 16', c.soft, 10)),
    diff: c => page(c, line('M128 100v40M108 120h40M108 172h40', c.soft, 13)),
    ...Object.fromEntries(LABELS.map(text => [`label-${text}`, c => page(c, word(text, 128, 120, c.soft))])),
};
