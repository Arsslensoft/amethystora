// Living wallpapers (modules/living.js): four slow pictures drawn in the theme's colours, picked by u_kind. Written for
// a Cogl snippet on every GPU (GLSL ES 1.0 too): no version or precision line, no loops, no texture sampling.

uniform float u_time;
// The screen's size in pixels, for the pictures' proportions
uniform vec2 u_size;
// 0 aurora, 1 drifting facets, 2 rain on glass, 3 slow dusk
uniform float u_kind;
uniform vec3 u_ground;
uniform vec3 u_accent;
uniform vec3 u_second;

float living_hash(vec2 p) {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

// A square, where pow() of a negative number is undefined
float living_sq(float x) {
    return x * x;
}

float living_noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(living_hash(i), living_hash(i + vec2(1.0, 0.0)), u.x),
               mix(living_hash(i + vec2(0.0, 1.0)), living_hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

// Bands of light that sway across the upper sky
vec3 living_aurora(vec2 uv, float t) {
    vec3 sky = mix(u_ground * 0.8, u_ground, uv.y);
    float wave = living_noise(vec2(uv.x * 3.0 + t * 0.4, t * 0.2));
    float y1 = 0.30 + 0.07 * sin(uv.x * 3.1 + t * 0.9) + 0.06 * wave;
    float y2 = 0.46 + 0.09 * sin(uv.x * 2.3 - t * 0.7 + 1.3) - 0.04 * wave;
    float b1 = exp(-living_sq((uv.y - y1) / 0.075)) * (0.6 + 0.4 * sin(uv.x * 9.0 + t * 1.3));
    float b2 = exp(-living_sq((uv.y - y2) / 0.11));
    vec3 c = mix(sky, u_accent, clamp(b1, 0.0, 1.0) * 0.55);
    return mix(c, u_second, clamp(b2, 0.0, 1.0) * 0.30);
}

// The nearest of a cell's moving point and its neighbours'
float living_cell(vec2 i, vec2 f, vec2 o, float t, inout float id) {
    vec2 g = i + o;
    vec2 point = o + 0.5 + 0.4 * sin(t * 0.25 + 6.2831 * vec2(living_hash(g), living_hash(g + 7.3)));
    float d = length(point - f);
    if (d < id)
        id = d;
    return d < id + 1e-6 ? living_hash(g) : -1.0;
}

// Large flat facets whose corners drift, each a shade between the ground and the accent
vec3 living_facets(vec2 uv, float t) {
    vec2 p = uv * vec2(u_size.x / max(u_size.y, 1.0), 1.0) * 3.5;
    vec2 i = floor(p);
    vec2 f = fract(p);
    float best = 8.0;
    float shade = 0.0;
    float s;
    s = living_cell(i, f, vec2(-1.0, -1.0), t, best); if (s >= 0.0) shade = s;
    s = living_cell(i, f, vec2(0.0, -1.0), t, best); if (s >= 0.0) shade = s;
    s = living_cell(i, f, vec2(1.0, -1.0), t, best); if (s >= 0.0) shade = s;
    s = living_cell(i, f, vec2(-1.0, 0.0), t, best); if (s >= 0.0) shade = s;
    s = living_cell(i, f, vec2(0.0, 0.0), t, best); if (s >= 0.0) shade = s;
    s = living_cell(i, f, vec2(1.0, 0.0), t, best); if (s >= 0.0) shade = s;
    s = living_cell(i, f, vec2(-1.0, 1.0), t, best); if (s >= 0.0) shade = s;
    s = living_cell(i, f, vec2(0.0, 1.0), t, best); if (s >= 0.0) shade = s;
    s = living_cell(i, f, vec2(1.0, 1.0), t, best); if (s >= 0.0) shade = s;
    vec3 c = mix(u_ground, u_accent, 0.15 + 0.45 * shade);
    return mix(c, u_second, 0.25 * uv.y);
}

// Drops running down a window pane, over the ground fading into the accent
vec3 living_rain(vec2 uv, float t) {
    vec3 c = mix(u_ground, mix(u_ground, u_accent, 0.35), uv.y);
    float columns = floor(u_size.x / 14.0);
    float column = floor(uv.x * columns);
    float speed = 0.04 + 0.08 * living_hash(vec2(column, 1.0));
    float head = fract(living_hash(vec2(column, 2.0)) + t * speed);
    float x = fract(uv.x * columns) - 0.5;
    float along = head - uv.y;
    float trail = along > 0.0 ? exp(-along * 18.0) : 0.0;
    float drop = exp(-living_sq(x / 0.12)) * trail * step(0.55, living_hash(vec2(column, 3.0)));
    float bead = exp(-dot(vec2(x / 0.18, along / 0.012), vec2(x / 0.18, along / 0.012)));
    return mix(c, mix(u_second, vec3(1.0), 0.4), clamp(drop * 0.35 + bead * 0.45, 0.0, 1.0));
}

// An evening sky whose colours sink and rise slowly, with a faint grain
vec3 living_dusk(vec2 uv, float t) {
    float shift = 0.08 * sin(t * 0.05);
    vec3 top = mix(u_ground, u_second, 0.25);
    vec3 low = mix(u_accent, u_second, 0.5 + 0.5 * sin(t * 0.03));
    vec3 c = mix(top, low, smoothstep(0.15 + shift, 1.05 + shift, uv.y));
    float glow = exp(-living_sq((uv.y - 0.78 - shift) / 0.18)) * 0.25;
    c = mix(c, u_accent, glow);
    return c + (living_hash(uv * u_size + t) - 0.5) * 0.015;
}

vec4 living_color() {
    vec2 uv = cogl_tex_coord_in[0].st;
    float t = u_time;
    vec3 c;
    if (u_kind < 0.5)
        c = living_aurora(uv, t);
    else if (u_kind < 1.5)
        c = living_facets(uv, t);
    else if (u_kind < 2.5)
        c = living_rain(uv, t);
    else
        c = living_dusk(uv, t);
    return vec4(clamp(c, 0.0, 1.0), 1.0);
}
