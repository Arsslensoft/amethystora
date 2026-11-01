# A wallpaper drawn from a theme's palette (ame desktop pattern), as SVG on stdout: one of seven patterns, drawn by a
# seeded generator so that the same palette, pattern and seed always give the same picture, and Shuffle a new seed. Input,
# one key=value per line on stdin: background, foreground, accent, color1-6 (#rrggbb), pattern (facets waves contour mesh
# grain dots bands), seed (a whole number), tone (dawn day dusk night) and light (1 for a light theme). amethystora-theme
# draws it once when a theme is applied, beside the theme's pictures, and nothing at idle. A tone is the palette moved
# towards the time of day, so that Daylight has four pictures of the pattern: night is a dark theme as it is and day a light
# one, the others tinted with the palette's own yellow, cyan and magenta.

function hexval(s,   i, n) {
    s = tolower(s); n = 0
    for (i = 1; i <= length(s); i++) n = n * 16 + index("0123456789abcdef", substr(s, i, 1)) - 1
    return n
}
function ch(h, i) { return hexval(substr(h, 2 * i, 2)) }
function rgb(r, g, b) { return sprintf("#%02x%02x%02x", clamp(int(r + 0.5)), clamp(int(g + 0.5)), clamp(int(b + 0.5))) }
function clamp(v) { return v < 0 ? 0 : (v > 255 ? 255 : v) }
function mix(a, b, t) { return rgb(ch(a, 1) * (1 - t) + ch(b, 1) * t, ch(a, 2) * (1 - t) + ch(b, 2) * t, ch(a, 3) * (1 - t) + ch(b, 3) * t) }
# Park and Miller's generator, whose products stay within a double's exact integers
function rnd() { SEED = (SEED * 16807) % 2147483647; return SEED / 2147483647 }
function between(a, b) { return a + (b - a) * rnd() }
function n1(v) { return sprintf("%.1f", v) }

# The colours of a tone: G the ground, A B C D the four the pattern is drawn in, F the text, faint
function tone(t,   bg) {
    bg = IN["background"]
    A = IN["accent"]; B = IN["color5"]; C = IN["color4"]; D = IN["color6"]; F = IN["foreground"]
    if (LIGHT) {
        if (t == "day") G = bg
        else if (t == "dawn") G = mix(bg, IN["color3"], 0.18)
        else if (t == "dusk") G = mix(mix(bg, IN["color5"], 0.2), "#000000", 0.1)
        else { G = mix(mix(bg, IN["color4"], 0.3), "#000000", 0.62); F = mix(F, "#ffffff", 0.6) }
    } else {
        if (t == "night") G = bg
        else if (t == "dawn") { G = mix(mix(mix(bg, IN["color3"], 0.22), IN["color1"], 0.12), "#ffffff", 0.06); A = mix(A, IN["color3"], 0.25); B = mix(B, IN["color1"], 0.25) }
        else if (t == "day") { G = mix(mix(bg, IN["color6"], 0.28), "#ffffff", 0.2); A = mix(A, "#ffffff", 0.12); C = mix(C, "#ffffff", 0.15) }
        else { G = mix(mix(bg, IN["color5"], 0.24), IN["color1"], 0.1); A = mix(A, IN["color1"], 0.2) }
    }
}

function head() {
    printf "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"
    printf "<!-- %s, drawn by amethystora-theme from the theme's palette (/usr/lib/amethystora/theme/pattern.awk) -->\n", PATTERN
    printf "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"%d\" height=\"%d\" viewBox=\"0 0 %d %d\">\n", W, H, W, H
}

function facets(   cols, rows, x, y, i, j, k, cx, cy, t, base, X, Y) {
    cols = 12; rows = 7
    for (j = 0; j <= rows; j++)
        for (i = 0; i <= cols; i++) {
            X[i, j] = i * W / cols + (i > 0 && i < cols ? between(-0.35, 0.35) * W / cols : 0)
            Y[i, j] = j * H / rows + (j > 0 && j < rows ? between(-0.35, 0.35) * H / rows : 0)
        }
    printf "  <rect width=\"%d\" height=\"%d\" fill=\"%s\"/>\n", W, H, G
    for (j = 0; j < rows; j++)
        for (i = 0; i < cols; i++)
            for (k = 0; k < 2; k++) {
                if (k == 0) { ax = X[i, j]; ay = Y[i, j]; bx = X[i + 1, j]; by = Y[i + 1, j]; cx = X[i, j + 1]; cy = Y[i, j + 1] }
                else { ax = X[i + 1, j]; ay = Y[i + 1, j]; bx = X[i + 1, j + 1]; by = Y[i + 1, j + 1]; cx = X[i, j + 1]; cy = Y[i, j + 1] }
                x = (ax + bx + cx) / 3; y = (ay + by + cy) / 3
                t = 0.15 + 0.45 * (x / W) * (1 - y / H) + between(0, 0.18)
                base = mix(G, mix(A, B, x / W), t)
                printf "  <path d=\"M%s,%sL%s,%sL%s,%sZ\" fill=\"%s\" stroke=\"%s\" stroke-width=\"1\"/>\n", n1(ax), n1(ay), n1(bx), n1(by), n1(cx), n1(cy), base, base
            }
}

function waves(   k, x, y, d, a1, a2, f1, f2, p1, p2, base, col, op) {
    printf "  <defs><radialGradient id=\"glow\" cx=\"%d\" cy=\"%d\" r=\"%d\" gradientUnits=\"userSpaceOnUse\">", W * 0.75, H * 0.3, W * 0.5
    printf "<stop offset=\"0\" stop-color=\"%s\" stop-opacity=\"0.18\"/><stop offset=\"1\" stop-color=\"%s\" stop-opacity=\"0\"/></radialGradient></defs>\n", A, A
    printf "  <rect width=\"%d\" height=\"%d\" fill=\"%s\"/>\n  <rect width=\"%d\" height=\"%d\" fill=\"url(#glow)\"/>\n", W, H, G, W, H
    for (k = 0; k < 6; k++) {
        base = H * (0.42 + k * 0.1); a1 = between(60, 160); a2 = between(20, 70)
        f1 = between(1.2, 2.4) * 6.2832 / W; f2 = between(3, 6) * 6.2832 / W; p1 = between(0, 6.2832); p2 = between(0, 6.2832)
        d = sprintf("M0,%d", H)
        for (x = 0; x <= W; x += 60) d = d sprintf("L%d,%s", x, n1(base + a1 * sin(x * f1 + p1) + a2 * sin(x * f2 + p2)))
        d = d sprintf("L%d,%dZ", W, H)
        col = k % 3 == 0 ? A : (k % 3 == 1 ? B : C)
        op = 0.22 + k * 0.1
        printf "  <path d=\"%s\" fill=\"%s\" fill-opacity=\"%.2f\"/>\n", d, mix(G, col, 0.35 + k * 0.08), op
    }
}

function contour(   c, k, s, th, r, d, cx, cy, p1, p2, a1, col) {
    printf "  <rect width=\"%d\" height=\"%d\" fill=\"%s\"/>\n", W, H, G
    for (c = 0; c < 2; c++) {
        cx = between(0.1, 0.9) * W; cy = between(0.1, 0.9) * H; p1 = between(0, 6.2832); p2 = between(0, 6.2832)
        a1 = between(50, 120); col = c == 0 ? A : D
        for (k = 1; k <= 26; k++) {
            d = ""
            for (s = 0; s <= 72; s++) {
                th = s * 6.2832 / 72
                r = k * 120 + a1 * sin(3 * th + p1) + a1 * 0.6 * sin(5 * th + p2 + k * 0.15)
                d = d (s == 0 ? "M" : "L") n1(cx + r * cos(th)) "," n1(cy + r * sin(th))
            }
            printf "  <path d=\"%sZ\" fill=\"none\" stroke=\"%s\" stroke-opacity=\"%.2f\" stroke-width=\"5\"/>\n", d, mix(G, col, 0.6), 0.7 - k * 0.02
        }
    }
}

function mesh(   k, col) {
    printf "  <defs>\n"
    for (k = 0; k < 5; k++) {
        col = k % 4 == 0 ? A : (k % 4 == 1 ? B : (k % 4 == 2 ? C : D))
        printf "    <radialGradient id=\"m%d\" cx=\"%d\" cy=\"%d\" r=\"%d\" gradientUnits=\"userSpaceOnUse\">", k, between(0, 1) * W, between(0, 1) * H, between(900, 1600)
        printf "<stop offset=\"0\" stop-color=\"%s\" stop-opacity=\"0.8\"/><stop offset=\"1\" stop-color=\"%s\" stop-opacity=\"0\"/></radialGradient>\n", mix(G, col, 0.75), col
    }
    printf "  </defs>\n  <rect width=\"%d\" height=\"%d\" fill=\"%s\"/>\n", W, H, G
    for (k = 0; k < 5; k++) printf "  <rect width=\"%d\" height=\"%d\" fill=\"url(#m%d)\"/>\n", W, H, k
}

function grain(   x1, y1) {
    x1 = between(0, 0.4); y1 = between(0, 0.4)
    printf "  <defs>\n    <linearGradient id=\"g\" x1=\"%.2f\" y1=\"%.2f\" x2=\"1\" y2=\"1\">", x1, y1
    printf "<stop offset=\"0\" stop-color=\"%s\"/><stop offset=\"0.55\" stop-color=\"%s\"/><stop offset=\"1\" stop-color=\"%s\"/></linearGradient>\n", G, mix(G, A, 0.45), mix(G, B, 0.35)
    printf "    <filter id=\"n\" x=\"0\" y=\"0\" width=\"100%%\" height=\"100%%\"><feTurbulence type=\"fractalNoise\" baseFrequency=\"0.85\" numOctaves=\"2\" seed=\"%d\"/>", SEED % 1000
    printf "<feColorMatrix values=\"0 0 0 0 1  0 0 0 0 1  0 0 0 0 1  0 0 0 0.09 0\"/></filter>\n  </defs>\n"
    printf "  <rect width=\"%d\" height=\"%d\" fill=\"url(#g)\"/>\n  <rect width=\"%d\" height=\"%d\" filter=\"url(#n)\"/>\n", W, H, W, H
}

function dots(   step, x, y, cx, cy, f, r) {
    printf "  <defs><linearGradient id=\"g\" x1=\"0\" y1=\"0\" x2=\"0.3\" y2=\"1\"><stop offset=\"0\" stop-color=\"%s\"/><stop offset=\"1\" stop-color=\"%s\"/></linearGradient></defs>\n", mix(G, A, 0.12), G
    printf "  <rect width=\"%d\" height=\"%d\" fill=\"url(#g)\"/>\n", W, H
    step = 60; cx = between(0.2, 0.8) * W; cy = between(0.2, 0.8) * H
    for (y = step / 2; y < H; y += step)
        for (x = step / 2; x < W; x += step) {
            f = 1 - sqrt((x - cx) ^ 2 + (y - cy) ^ 2) / (W * 0.75)
            f = f + 0.18 * sin(x / 310 + SEED % 7) * cos(y / 260)
            if (f < 0.05) continue
            r = 2 + 15 * (f > 1 ? 1 : f)
            printf "  <circle cx=\"%d\" cy=\"%d\" r=\"%.1f\" fill=\"%s\"/>\n", x, y, r, mix(A, B, x / W)
        }
}

function bands(   angle, x, w, t, col) {
    angle = between(100, 130)
    printf "  <rect width=\"%d\" height=\"%d\" fill=\"%s\"/>\n  <g transform=\"rotate(%.1f %d %d)\">\n", W, H, G, angle, W / 2, H / 2
    for (x = -W; x < 2 * W; x += w) {
        w = between(80, 280)
        t = rnd()
        if (t > 0.9) col = A
        else col = mix(G, t > 0.5 ? A : B, t * 0.55)
        printf "    <rect x=\"%s\" y=\"%d\" width=\"%s\" height=\"%d\" fill=\"%s\"/>\n", n1(x), -W, n1(w + 1), 3 * W, col
    }
    printf "  </g>\n"
}

BEGIN { FS = "="; W = 3840; H = 2160 }
{ IN[$1] = tolower($2) }
END {
    PATTERN = IN["pattern"]
    SEED = int(IN["seed"]) % 2147483646 + 1
    LIGHT = IN["light"] == 1
    for (key in IN) if (key ~ /^(background|foreground|accent|color[1-6])$/ && IN[key] !~ /^#[0-9a-f]{6}$/) exit 2
    if (IN["background"] == "" || IN["accent"] == "") exit 2
    if (IN["color5"] == "") IN["color5"] = IN["accent"]
    if (IN["color4"] == "") IN["color4"] = IN["accent"]
    if (IN["color6"] == "") IN["color6"] = IN["color4"]
    if (IN["color3"] == "") IN["color3"] = IN["accent"]
    if (IN["color1"] == "") IN["color1"] = IN["color5"]
    tone(IN["tone"] == "" ? (LIGHT ? "day" : "night") : IN["tone"])
    # A few draws first, so that two seeds close together do not start alike
    rnd(); rnd(); rnd()
    if (PATTERN == "facets") { head(); facets() }
    else if (PATTERN == "waves") { head(); waves() }
    else if (PATTERN == "contour") { head(); contour() }
    else if (PATTERN == "mesh") { head(); mesh() }
    else if (PATTERN == "grain") { head(); grain() }
    else if (PATTERN == "dots") { head(); dots() }
    else if (PATTERN == "bands") { head(); bands() }
    else exit 2
    print "</svg>"
}
