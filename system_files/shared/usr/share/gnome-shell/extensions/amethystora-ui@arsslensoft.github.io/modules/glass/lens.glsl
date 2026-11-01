// The glass, drawn under a surface from the frosted picture of its screen (modules/glass/frost.js). Every length is
// in device pixels, measured from the top left of the surface. The outline is a rounded rectangle whose corners are
// continuous (the curve starts further out and eases in) or circular. Inside it the picture is bent inwards near the
// edge, as through the rim of thick glass, its colour lifted or calmed and made milky by the material, veiled by the
// theme's background just enough for the text on it, and lit: a lift fading downwards, glows gathering along the
// bottom and up the left side, thin lines along the top and sides, a ring, the hairline edge and a sheen across.
// Outside it, a soft drop shadow. While the surface fades in or out the frost condenses or clears with it.
// Written for Amethystora (Apache-2.0).

uniform sampler2D u_picture;
// The same spot unblurred, and how frosted the glass is: 1 as made, 0 clear
uniform sampler2D u_clear;
uniform float u_frost;
// Where the surface's top left falls in the picture, how much of the picture one pixel is, and the surface's size
uniform vec2 u_origin;
uniform vec2 u_pixel;
uniform vec2 u_size;
uniform float u_opacity;
// The drop shadow (its strength, its blur and how far down it falls) and a hairline ring just outside the outline
uniform vec4 u_shadow;
// The corners: how far from the corner the curve starts, and its exponent (2 is a circle)
uniform vec2 u_corner;
// Saturation, brightness, the share of white, and the rim's light (frost only)
uniform vec4 u_tone;
// The bend at the rim: pull at the very edge, reach, steepness, and how much further the red goes than the blue
uniform vec4 u_rim;
// The darkening towards the bottom (frost only)
uniform float u_shade;
// The theme's background, and how much of it lies over the picture
uniform vec4 u_veil;
// The hairline just inside the outline: its colour and strength
uniform vec4 u_edge;
// White over the pane at its top and at its bottom
uniform vec2 u_lift;
// The top line's strength and width, the left and right lines' strengths
uniform vec4 u_light;
// A faint white ring just inside the outline
uniform float u_ring;
// The accent's light along the bottom: its colour and strength; then where it gathers from (down, its blur, how far
// it reaches in) and the strength of the white light up the left side
uniform vec4 u_glow;
uniform vec4 u_glowshape;
// The left side's light: across, its blur, how far it reaches in; and one stylesheet pixel in device pixels
uniform vec4 u_sideglow;
// The sheen: white at the top left, where it ends, where the accent's begins, accent at the bottom right
uniform vec4 u_sheen;
// Above 0, the theme's background is drawn at that strength instead of the picture: while there is no picture yet,
// and for a bare bar, which only ever shows the veil its text needs
uniform float u_solid;

// Signed distance to the outline of a box of half size h centred on the origin, its corners starting s from each
// corner with exponent n: negative inside. For n above 2 the distance in the corner is the n-norm's, divided by the
// length of its gradient so that it is a distance in pixels, which keeps the antialiasing one pixel wide all round.
float glass_outline(vec2 c, vec2 h, float s, float n)
{
    vec2 q = abs(c) - (h - s);
    if (s < 0.5)
        return max(q.x, q.y);
    if (q.x > 0.0 && q.y > 0.0) {
        if (n < 2.001)
            return length(q) - s;
        vec2 a = max(q / s, vec2(1e-4));
        vec2 up = vec2(pow(a.x, n - 1.0), pow(a.y, n - 1.0));
        float k = up.x * a.x + up.y * a.y;
        float norm = pow(k, 1.0 / n);
        vec2 g = up * (norm / k);
        return s * (norm - 1.0) / max(length(g), 1e-4);
    }
    return max(q.x, q.y) - s;
}

// The way out of the outline at c: straight out of the nearest side, or along the corner's normal
vec2 glass_outwards(vec2 c, vec2 h, float s, float n)
{
    vec2 q = abs(c) - (h - s);
    vec2 away;
    if (s >= 0.5 && q.x > 0.0 && q.y > 0.0) {
        if (n < 2.001) {
            away = normalize(q);
        } else {
            vec2 a = max(q / s, vec2(1e-4));
            away = normalize(vec2(pow(a.x, n - 1.0), pow(a.y, n - 1.0)));
        }
    } else if (q.x > q.y) {
        away = vec2(1.0, 0.0);
    } else {
        away = vec2(0.0, 1.0);
    }
    return away * vec2(c.x < 0.0 ? -1.0 : 1.0, c.y < 0.0 ? -1.0 : 1.0);
}

// How much of a pixel at signed distance d lies inside
float glass_cover(float d)
{
    return clamp(0.5 - d, 0.0, 1.0);
}

// The normal distribution's integral, near enough
float glass_soft(float t)
{
    return 1.0 / (1.0 + exp(-1.702 * t));
}

vec3 glass_sample(vec2 at)
{
    vec2 spot = u_origin + at * u_pixel;
    vec3 frosted = texture2D(u_picture, spot).rgb;
    if (u_frost >= 1.0)
        return frosted;
    return mix(texture2D(u_clear, spot).rgb, frosted, u_frost);
}

vec4 glass_color()
{
    vec2 p = (cogl_tex_coord_in[0].st - u_origin) / u_pixel;
    vec2 size = u_size;
    vec2 h = size * 0.5;
    vec2 c = p - h;
    float s = u_corner.x;
    float n = u_corner.y;
    float unit = u_sideglow.w;
    float edge = glass_outline(c, h, s, n);
    float cover = glass_cover(edge);
    vec4 color = vec4(0.0);

    if (cover > 0.0 && u_solid > 0.0) {
        color = vec4(u_veil.rgb, 1.0) * (u_solid * cover);
    } else if (cover > 0.0) {
        float depth = max(-edge, 0.0);
        vec3 col;
        float pull = u_rim.x > 0.0 ? u_rim.x * exp(-pow(max(depth / u_rim.y, 1e-4), u_rim.z)) : 0.0;
        if (pull > 0.01) {
            // The picture shown here comes from further in, the red a little further than the blue
            vec2 inwards = -glass_outwards(c, h, s, n);
            col.r = glass_sample(p + inwards * pull * (1.0 + u_rim.w)).r;
            col.g = glass_sample(p + inwards * pull).g;
            col.b = glass_sample(p + inwards * pull * (1.0 - u_rim.w)).b;
        } else {
            col = glass_sample(p);
        }

        // The material's colour; a surface that is fading clears towards the plain picture
        float luma = dot(col, vec3(0.2126, 0.7152, 0.0722));
        col = clamp(vec3(luma) + (col - vec3(luma)) * mix(1.0, u_tone.x, u_frost), 0.0, 1.0);
        col *= mix(1.0, u_tone.y, u_frost);
        col = mix(col, vec3(1.0), u_tone.z);

        if (u_tone.w > 0.0) {
            // Light caught by the rim: strongest on the outermost pixels, with a faint long tail, and weaker over
            // dark parts of the picture, more so away from the edge. Added as a screen, so it never darkens.
            float light = u_tone.w * (0.75 * exp(-pow(depth / 2.0, 2.0)) + 0.25 * exp(-depth / 6.0));
            float bright = dot(col, vec3(0.2126, 0.7152, 0.0722));
            light *= mix(mix(0.8, 0.5, clamp(depth / 8.0, 0.0, 1.0)), 1.0, bright);
            col += (vec3(1.0) - col) * light;
        }
        if (u_shade > 0.0)
            col *= 1.0 - u_shade * smoothstep(0.25, 1.0, p.y / size.y);

        col = mix(col, u_veil.rgb, u_veil.a);

        if (u_lift.x > 0.0 || u_lift.y > 0.0)
            col = mix(col, vec3(1.0), mix(u_lift.x, u_lift.y, clamp(p.y / size.y, 0.0, 1.0)));

        // The glows: soft inset shadows whose hole is the outline grown and moved, worked out only near their side
        if (u_glow.a > 0.0) {
            float sigma = u_glowshape.y;
            if (size.y - p.y < 3.0 * sigma - u_glowshape.x - u_glowshape.z) {
                float hole = glass_outline(c - vec2(0.0, u_glowshape.x), h + u_glowshape.z, s + u_glowshape.z, n);
                col = mix(col, u_glow.rgb, u_glow.a * glass_soft(hole / sigma));
            }
        }
        if (u_glowshape.w > 0.0) {
            float sigma = u_sideglow.y;
            if (p.x < 3.0 * sigma + u_sideglow.x - u_sideglow.z) {
                float hole = glass_outline(c - vec2(u_sideglow.x, 0.0), h + u_sideglow.z, s + u_sideglow.z, n);
                col = mix(col, vec3(1.0), u_glowshape.w * glass_soft(hole / sigma));
            }
        }

        // Thin lines just inside the outline, following its corners; the outline's own coverage, applied last,
        // softens their outer side
        if (u_light.x > 0.0 && p.y < s + u_light.y + unit) {
            float line = 1.0 - glass_cover(glass_outline(c - vec2(0.0, u_light.y), h, s, n));
            col = mix(col, vec3(1.0), u_light.x * line);
        }
        if (u_light.z > 0.0 && p.x < s + 2.0 * unit) {
            float line = 1.0 - glass_cover(glass_outline(c - vec2(unit, 0.0), h, s, n));
            col = mix(col, vec3(1.0), u_light.z * line);
        }
        if (u_light.w > 0.0 && p.x > size.x - s - 2.0 * unit) {
            float line = 1.0 - glass_cover(glass_outline(c + vec2(unit, 0.0), h, s, n));
            col = mix(col, vec3(1.0), u_light.w * line);
        }
        if (u_ring > 0.0 || u_edge.a > 0.0) {
            float rim = 1.0 - glass_cover(edge + unit);
            col = mix(col, vec3(1.0), u_ring * rim);
            col = mix(col, u_edge.rgb, u_edge.a * rim);
        }

        if (u_sheen.x > 0.0 || u_sheen.w > 0.0) {
            // Across the pane from its top left to its bottom right, as a gradient at 125 degrees runs
            vec2 along = vec2(0.8192, 0.5736);
            float t = dot(c, along) / (size.x * along.x + size.y * along.y) + 0.5;
            if (t < u_sheen.y)
                col = mix(col, vec3(1.0), u_sheen.x * (1.0 - t / u_sheen.y));
            else if (t > u_sheen.z)
                col = mix(col, u_glow.rgb, u_sheen.w * (t - u_sheen.z) / (1.0 - u_sheen.z));
        }

        color = vec4(col, 1.0) * cover;
    }

    if (u_shadow.w > 0.0 && edge > -0.5) {
        // A hairline just outside the outline
        float ring = u_shadow.w * clamp(1.0 - abs(edge - 0.5), 0.0, 1.0);
        color += vec4(0.0, 0.0, 0.0, ring) * (1.0 - color.a);
    }

    if (u_shadow.x > 0.0) {
        float below = glass_outline(c - vec2(0.0, u_shadow.z), h, s, n);
        float shadow = u_shadow.x * (1.0 - smoothstep(-u_shadow.y, u_shadow.y, below));
        color += vec4(0.0, 0.0, 0.0, shadow) * (1.0 - color.a);
    }

    return color * u_opacity;
}
