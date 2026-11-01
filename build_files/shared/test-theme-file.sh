#!/usr/bin/bash
# Themes into files and out of them (/usr/libexec/amethystora-theme-file, or the copy named first), for 20-tests.sh: a
# kitty, an Alacritty, a base16, a base24 and an Xresources file each give the colours they name, and only those, with
# every other line (an include, an import) counted and left out; a picture beside one is decoded and written again as
# PNG; an address, a file of /proc, a folder and a file that names no colour are refused. A palette saved from the theme
# maker is written whole, a bad line or a shipped theme's name refused. A look file shared holds exactly its three names,
# reads back as it was, and one with another name, a link, a path out of it, a name twice or too large is refused whole.
# It runs as an account, never as root, which the build is: there it runs as nobody, in a folder of its own.

set -euo pipefail

HELPER="${1:-/usr/libexec/amethystora-theme-file}"
fail() { echo "test-theme-file: $*" >&2; exit 1; }
T="$(mktemp -d)"
trap 'rm -rf "${T}"' EXIT
mkdir -p "${T}/home" "${T}/in"
AS=(env -i "HOME=${T}/home" "XDG_CONFIG_HOME=${T}/home/.config" PATH=/usr/bin:/usr/sbin)
if [[ ${EUID} == 0 ]]; then
    AS=(setpriv --reuid=nobody --regid=nobody --clear-groups "${AS[@]}")
fi
run() { "${AS[@]}" python3 -I "${HELPER}" "$@"; }
THEMES="${T}/home/.config/amethystora/themes"

cat >"${T}/in/kanagawa.conf" <<'EOF'
# Kanagawa, for kitty
include ~/.config/kitty/other.conf
font_size 12
background #1F1F28
foreground #dcd7ba
selection_background #2d4f67
cursor #c8c093
color0 #16161d
color1 #c34043
color2 #76946a
color3 #c0a36e
color4 #7e9cd8
color5 #957fb8
color6 #6a9589
color7 #c8c093
color8 #727169
color9 #e82424
color10 #98bb6c
color11 #e6c384
color12 #7fb4ca
color13 #938aa9
color14 #7aa89f
color15 #dcd7ba
EOF
cat >"${T}/in/night.toml" <<'EOF'
[general]
import = ["~/.config/alacritty/evil.toml"]

[colors.primary]
background = "0x101418"
foreground = "#e0e0e0"

[colors.normal]
black = "#101418"
red = "#ff5555"
green = "#50fa7b"
yellow = "#f1fa8c"
blue = "#6272a4"
magenta = "#ff79c6"
cyan = "#8be9fd"
white = "#f8f8f2"

[colors.selection]
background = "#44475a"

[window]
opacity = 0.9
EOF
cat >"${T}/in/ocean.yaml" <<'EOF'
scheme: "Ocean Deep"
author: "someone"
base00: "2b303b"
base01: "343d46"
base02: "4f5b66"
base03: "65737e"
base04: "a7adba"
base05: "c0c5ce"
base06: "dfe1e8"
base07: "eff1f5"
base08: "bf616a"
base09: "d08770"
base0A: "ebcb8b"
base0B: "a3be8c"
base0C: "96b5b4"
base0D: "8fa1b3"
base0E: "b48ead"
base0F: "ab7967"
EOF
cat >"${T}/in/bright.yaml" <<'EOF'
system: "base24"
name: "Bright Day"
palette:
  base00: "#fafafa"
  base01: "#f0f0f0"
  base02: "#d0d0d0"
  base03: "#a0a0a0"
  base04: "#808080"
  base05: "#202020"
  base06: "#101010"
  base07: "#000000"
  base08: "#c41a16"
  base09: "#d06a00"
  base0A: "#a07000"
  base0B: "#007400"
  base0C: "#0e7c86"
  base0D: "#1c4bd0"
  base0E: "#8a2bb0"
  base0F: "#804000"
  base10: "#ffffff"
  base11: "#ffffff"
  base12: "#e02020"
  base13: "#b08000"
  base14: "#108410"
  base15: "#1e8c96"
  base16: "#2c5be0"
  base17: "#9a3bc0"
EOF
cat >"${T}/in/Xresources" <<'EOF'
! Solarized, the old way
#define S_base03 #002b36
#define S_base0 #839496
#include ".Xresources.d/other"
*background: S_base03
*foreground: S_base0
*cursorColor: #93a1a1
URxvt*color1: #dc322f
*.color2: #859900
*color4: #268bd2
*color16: #ffffff
URxvt.font: xft:Monospace
EOF

chmod -R a+rX "${T}"
if [[ ${EUID} == 0 ]]; then
    chown -R nobody:nobody "${T}/home"
fi

# What read says of a file, as `KEY VALUE` lines of its JSON
read_json() { run read "$1" | python3 -I -c 'import json, sys
d = json.load(sys.stdin)
for k in ("format", "name", "read", "left_out", "light"):
    print(k, d[k])
for k, v in d["colors"].items():
    print(k, v)
print("picture", d["picture"] or "")'; }
expect() {
    local out="$1" key="$2" want="$3" got
    got="$(sed -n "s/^${key} //p" <<<"${out}")"
    [[ ${got} == "${want}" ]] || fail "$4: ${key} is '${got}', not '${want}'"
}

out="$(read_json "${T}/in/kanagawa.conf")"
expect "${out}" format kitty kitty
expect "${out}" background '#1f1f28' kitty
expect "${out}" color12 '#7fb4ca' kitty
expect "${out}" accent '#7e9cd8' kitty
expect "${out}" read 20 kitty
expect "${out}" left_out 2 kitty
expect "${out}" light False kitty
expect "${out}" name kanagawa kitty

out="$(read_json "${T}/in/night.toml")"
expect "${out}" format alacritty alacritty
expect "${out}" background '#101418' alacritty
expect "${out}" color5 '#ff79c6' alacritty
expect "${out}" color13 '#ff79c6' alacritty
expect "${out}" selection_background '#44475a' alacritty
expect "${out}" read 11 alacritty
expect "${out}" left_out 2 alacritty

out="$(read_json "${T}/in/ocean.yaml")"
expect "${out}" format base16 base16
expect "${out}" name 'Ocean Deep' base16
expect "${out}" foreground '#c0c5ce' base16
expect "${out}" accent '#8fa1b3' base16
expect "${out}" color9 '#bf616a' base16
expect "${out}" read 16 base16

out="$(read_json "${T}/in/bright.yaml")"
expect "${out}" format base24 base24
expect "${out}" color9 '#e02020' base24
expect "${out}" color12 '#2c5be0' base24
expect "${out}" light True base24
expect "${out}" read 24 base24

out="$(read_json "${T}/in/Xresources")"
expect "${out}" format xresources xresources
expect "${out}" background '#002b36' xresources
expect "${out}" foreground '#839496' xresources
expect "${out}" cursor '#93a1a1' xresources
expect "${out}" color1 '#dc322f' xresources
expect "${out}" color2 '#859900' xresources
expect "${out}" read 6 xresources
expect "${out}" left_out 3 xresources

# Refused: an address, a file of the kernel's, a folder, a file with no colour in it
for bad in "https://example.org/theme.conf" "/proc/self/status" "${T}/in" "${T}/in/empty"; do
    : >"${T}/in/empty"
    if run read "${bad}" >/dev/null 2>&1; then fail "${bad} was read"; fi
done

# A picture beside the file comes along, written again as PNG
python3 -I -c 'import sys, gi
gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf
p = GdkPixbuf.Pixbuf.new(GdkPixbuf.Colorspace.RGB, False, 8, 64, 36)
p.fill(0x7e9cd8ff)
p.savev(sys.argv[1], "jpeg", [], [])' "${T}/in/kanagawa.jpg" 2>/dev/null
chmod a+r "${T}/in/kanagawa.jpg"
[[ "$(read_json "${T}/in/kanagawa.conf" | sed -n 's/^picture //p')" == "${T}/in/kanagawa.jpg" ]] || fail "the picture beside it is not found"
[[ "$(run import "${T}/in/kanagawa.conf" Kanagawa)" == kanagawa ]] || fail "import did not say the theme's folder"
[[ -f "${THEMES}/kanagawa/colors.toml" && ! -e "${THEMES}/kanagawa/light.mode" ]] || fail "import wrote no dark theme"
head -c 8 "${THEMES}/kanagawa/backgrounds/kanagawa.png" | od -An -c | grep -q 'P   N   G' || fail "the picture is not a PNG"
grep -q '^color12 = "#7fb4ca"$' "${THEMES}/kanagawa/colors.toml" || fail "the imported colours are not written"
[[ "$(grep -c ' = "#' "${THEMES}/kanagawa/colors.toml")" == 22 ]] || fail "the imported theme has not every key"
run import "${T}/in/bright.yaml" "Bright Day" >/dev/null
[[ -e "${THEMES}/bright-day/light.mode" ]] || fail "a light ground makes no light theme"

# A palette from the maker: whole, and only a palette
palette="$(grep ' = "#' "${THEMES}/kanagawa/colors.toml")"
[[ "$(run save 'My Own' <<<"${palette}")" == my-own ]] || fail "save did not say the theme's folder"
cmp -s <(grep ' = "#' "${THEMES}/my-own/colors.toml") <(echo "${palette}") || fail "save changed the palette"
for bad in "$(sed 's/#1f1f28/#1f1f2/' <<<"${palette}")" "$(sed '/^accent/d' <<<"${palette}")" \
    "$(echo "${palette}"; echo 'evil = "#000000"')" "$(echo "${palette}"; echo 'background = "#000000"')" \
    "$(sed 's/^background = .*/background = "$(id)"/' <<<"${palette}")"; do
    if run save Bad <<<"${bad}" >/dev/null 2>&1; then fail "save took a palette it should refuse"; fi
done
[[ ! -e "${THEMES}/bad" ]] || fail "a refused palette left a folder"
for name in nord ../x '' "$(printf 'a%.0s' {1..41})"; do
    if run save "${name}" <<<"${palette}" >/dev/null 2>&1; then fail "save took the name '${name}'"; fi
done

# A look file: three fixed names, read back as they were
run share "${T}/home/Kanagawa.amethystora-look" --theme "${THEMES}/kanagawa" --name Kanagawa \
    --picture "${THEMES}/kanagawa/backgrounds/kanagawa.png" --style islands --icons amethystora-glass --font 'Space Grotesk' \
    --accent amethyst >/dev/null
[[ "$(tar -tf "${T}/home/Kanagawa.amethystora-look" | sort | tr '\n' ' ')" == "colors.toml look.conf wallpaper.png " ]] ||
    fail "the look file does not hold exactly its three names"
look="$(run look "${T}/home/Kanagawa.amethystora-look")"
python3 -I -c 'import json, sys
d = json.loads(sys.argv[1])
assert d["name"] == "Kanagawa" and d["picture"] and d["look"]["style"] == "islands", d
assert d["look"]["font"] == "Space Grotesk" and d["colors"]["color12"] == "#7fb4ca", d' "${look}" ||
    fail "the look file does not read back as it was written"
[[ "$(run look-import "${T}/home/Kanagawa.amethystora-look" 'Kanagawa Look' | python3 -I -c 'import json, sys; print(json.load(sys.stdin)["slug"])')" == kanagawa-look ]] ||
    fail "look-import did not write the theme"
[[ -f "${THEMES}/kanagawa-look/backgrounds/wallpaper.png" ]] || fail "look-import wrote no picture"

# Each a look file to refuse whole
mkdir -p "${T}/bad"
cp "${THEMES}/kanagawa/colors.toml" "${T}/bad/colors.toml"
printf 'name = Bad\n' >"${T}/bad/look.conf"
( cd "${T}/bad" && ln -s /etc/passwd wallpaper.png && tar -cf ../link.amethystora-look look.conf colors.toml wallpaper.png )
( cd "${T}/bad" && rm wallpaper.png && echo x >extra.sh && tar -cf ../extra.amethystora-look look.conf colors.toml extra.sh )
( cd "${T}/bad" && tar -cf ../twice.amethystora-look look.conf colors.toml look.conf )
( cd "${T}/bad" && tar -cf ../path.amethystora-look look.conf ../bad/colors.toml 2>/dev/null )
( cd "${T}/bad" && head -c 30000 /dev/zero >big && cp big colors.toml && tar -cf ../big.amethystora-look look.conf colors.toml )
( cd "${T}/bad" && cp "${THEMES}/kanagawa/colors.toml" colors.toml && gzip -c ../extra.amethystora-look >../gzip.amethystora-look )
chmod -R a+rX "${T}/bad" "${T}"/*.amethystora-look 2>/dev/null || true
for bad in link extra twice path big gzip; do
    if run look "${T}/${bad}.amethystora-look" >/dev/null 2>&1; then fail "the ${bad} look file was read"; fi
done

echo "test-theme-file: every check passed"
