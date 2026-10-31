// The three kinds of glass a person can pick (ame desktop glass, Control): frost, the thick milky glass the desktop
// has always had; prism, with less tint, more colour, a lit top edge and light gathering along the bottom; and clear,
// the thinnest, with the brightest rim. Each is the picture its glass is cut from (frost.js) and what each material is
// made of (effect.js draws it with lens.glsl). Lengths are in the stylesheet's pixels, alphas from 0 to 1.
//
// A material:
//   picture   the frosted picture it is cut from: 'glass' (this variant's) or 'frost' (the overview's, the same in all)
//   sat       saturation (1 leaves the picture as it is)       dim    brightness factor     tint   share of white
//   rim       [pull, reach, steep, fringe]: near the edge the picture is drawn from further in, `pull` pixels at the
//             very edge, dying away as exp(-(d / reach) ^ steep), the red a share `fringe` further than the blue
//   glint     light caught by the rim (frost only)               shade  darkening towards the bottom (frost only)
//   veil      [dark, light]: how much of the theme's background lies over the picture, in a dark and a light scheme
//   edge      the hairline inside the outline, as a share of the text colour
//   lift      [top, bottom]: white over the pane, fading downwards
//   top       [alpha, width]: a white line along the top          sides  [left, right]: white lines down the sides
//   ring      a faint white ring all round, just inside the outline
//   glow      {alpha, y, blur, grow}: the accent's light gathering along the bottom
//   sideGlow  {alpha, x, blur, grow}: white light up the left side
//   sheen     [white at the top left, where it ends, where the accent's begins, accent at the bottom right]
//   shadow    {dark, light}: the drop shadow {alpha, blur, offset} of the popups that cast one
//   bare      the veil alone, under a bar a style leaves bare over the wallpaper (capsule, segments): nothing is drawn
//             until the wallpaper right behind the bar is too bright (or dark) for its text, and then only the veil
//   guard     the light the text may count on (the lift and sheen where the first line sits), or null: with a
//             number, the veil is thickened over a bright (or, for dark text, a dark) wallpaper until text reads
//             at 4.5:1 (contrast.js)

const NONE = {
    sat: 1, dim: 1, tint: 0, rim: [0, 1, 1, 0], glint: 0, shade: 0, veil: [0, 0], edge: 0,
    lift: null, top: null, sides: null, ring: 0, glow: null, sideGlow: null, sheen: null, shadow: null, guard: null,
};

const FROST_LENS = {rim: [12.8, 7.28, 1.59, 0.024], glint: 0.08, shade: 0.044};
const FROST_DOCK_LENS = {rim: [16.8, 8.56, 1.71, 0.028], glint: 0.09, shade: 0.048};

const PRISM_LIGHTS = {
    lift: [0.10, 0.02], top: [0.50, 1], sides: [0.12, 0.06], ring: 0,
    glow: {alpha: 0.55, y: -14, blur: 22, grow: 16}, sideGlow: null, sheen: [0.22, 0.32, 0.70, 0.14],
};
const CLEAR_LIGHTS = {
    lift: null, top: [0.65, 1.5], sides: null, ring: 0.08,
    glow: {alpha: 0.70, y: -16, blur: 24, grow: 16}, sideGlow: {alpha: 0.40, x: 14, blur: 22, grow: 18},
    sheen: [0.22, 0.32, 0.70, 0.14],
};

// The overview and the workspace slide: the darker frost, the same in every variant
const OVERVIEW = {...NONE, picture: 'frost'};
// A bare bar: the veil its text needs over the wallpaper right behind it, and nothing else
const BARE = {...NONE, picture: 'glass', bare: true, guard: 0};

export const VARIANTS = {
    frost: {
        recipe: {radius: 48, brightness: 1.0, shrink: 4},
        materials: {
            'glass': {
                ...NONE, picture: 'glass', tint: 0.16, ...FROST_LENS, veil: [0.22, 0.58], edge: 0.05,
                shadow: {dark: {alpha: 0.16, blur: 24, offset: 8}, light: {alpha: 0.08, blur: 22, offset: 8}},
                guard: 0,
            },
            'pane': {...NONE, picture: 'glass', tint: 0.16, ...FROST_LENS},
            'dock': {...NONE, picture: 'glass', tint: 0.16, ...FROST_DOCK_LENS, guard: 0},
            'menu-bar': {...NONE, picture: 'glass', dim: 0.85, guard: 0},
            'frost': OVERVIEW,
            'bare': BARE,
        },
    },
    prism: {
        recipe: {radius: 33, brightness: 1.0, shrink: 4},
        materials: {
            'glass': {
                ...NONE, ...PRISM_LIGHTS, picture: 'glass', sat: 1.85, tint: 0.10, rim: [6, 3, 2, 0.024],
                veil: [0.14, 0.36], edge: 0.08,
                shadow: {dark: {alpha: 0.17, blur: 27, offset: 9}, light: {alpha: 0.09, blur: 25, offset: 9}},
                guard: 0.20,
            },
            'pane': {...NONE, picture: 'glass', sat: 1.85, tint: 0.10, rim: [6, 3, 2, 0.024]},
            'dock': {
                ...NONE, ...PRISM_LIGHTS, picture: 'glass', sat: 1.85, tint: 0.10, rim: [8, 3, 2, 0.028], edge: 0.08,
                guard: 0.20,
            },
            'menu-bar': {
                ...NONE, ...PRISM_LIGHTS, picture: 'glass', sat: 1.85, rim: [6, 3, 2, 0.024], edge: 0.08, guard: 0.20,
            },
            'frost': OVERVIEW,
            'bare': BARE,
        },
    },
    clear: {
        recipe: {radius: 13, brightness: 1.0, shrink: 2},
        materials: {
            'glass': {
                ...NONE, ...CLEAR_LIGHTS, picture: 'glass', sat: 2.0, dim: 0.82, rim: [10, 3.5, 2, 0.024],
                veil: [0.09, 0.23], edge: 0.11,
                shadow: {dark: {alpha: 0.14, blur: 27, offset: 9}, light: {alpha: 0.07, blur: 25, offset: 9}},
                guard: 0.14,
            },
            'pane': {...NONE, picture: 'glass', sat: 2.0, dim: 0.82, rim: [10, 3.5, 2, 0.024]},
            'dock': {
                ...NONE, ...CLEAR_LIGHTS, picture: 'glass', sat: 2.0, dim: 0.82, rim: [13, 3.5, 2, 0.028], edge: 0.11,
                guard: 0.14,
            },
            'menu-bar': {
                ...NONE, ...CLEAR_LIGHTS, picture: 'glass', sat: 2.0, dim: 0.82, rim: [10, 3.5, 2, 0.024], edge: 0.11,
                guard: 0.14,
            },
            'frost': OVERVIEW,
            'bare': BARE,
        },
    },
};

export const DEFAULT_VARIANT = 'prism';

// Continuous corners: the curve starts `reach` times the radius from the corner, and passes `diagonal` times the
// radius in from the corner on each axis, as the boards draw it
export const CONTINUOUS = {reach: 1.4, diagonal: 0.3101};
