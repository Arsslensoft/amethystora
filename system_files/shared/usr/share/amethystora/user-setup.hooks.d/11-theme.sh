#!/usr/bin/env bash
#
# Apply the Amethystora theme once, at first login, so a new account starts with the terminal
# palette, prompt and wallpaper already matching the desktop. After that the theme is the user's:
# this only runs again when the version below is raised, which is how an account picks up something
# the theme system has learnt to set since it was set up. Raising it re-applies the account's own
# theme rather than the default, so a theme the user chose is kept.
#
#   2  the candy-icons icon theme (build_files/base/10-icons.sh). Accounts set up before it existed
#      hold an explicit icon-theme in their dconf, which wins over the image's new default, so they
#      would stay on the desktop's plain icons until the theme is applied again.
#   3  the theme mirror in ~/.themes and the libadwaita user stylesheet that imports from it
#      (theme-set.hooks.d/20-gtk-apps.sh). Accounts set up before it existed have neither, so every
#      Flatpak application on them keeps the runtime's stock Adwaita until the theme is applied again.
#   4  the kitty palette, for as long as kitty was the terminal (see 11).
#   5  the account after user-setup.hooks.d/09-retire-dms.sh, which runs just before this and removes
#      what DankMaterialShell left from the Hyprland era. Its gtk.css carried no amethystora-theme
#      marker, so 20-gtk-apps.sh had taken it for the user's own and never written the theme's.
#   6  a photograph for every theme (backgrounds.list), where all but the two Amethystora themes had
#      opened on a palette glow that looks like a plain colour, and the geode taken out of the image:
#      an account showing it would be left on GNOME's fill colour.
#   7  the themes' photographs taken out of the image for pictures drawn in their palettes
#      (branding/generate.mjs): an account showing one would be left on GNOME's fill colour.
#   8  the Amethystora icon packs (branding/icons.mjs) in place of candy-icons as the default: an account
#      that applied a theme holds candy-icons in its dconf, which wins over the image's new default.
#   9  candy-icons gone from the image: an account that chose it (~/.config/amethystora/icons) is put back on the
#      theme's own pack, and the theme applied again writes Glass or Duotone in place of the name it held.
#  10  the dock's, the tiling's and the glass's colours (themed/shell-dock|tiling|glass.css.tpl): an account's rendered
#      theme folder has none of them until the theme is applied again.
#  11  kitty as the terminal (themed/kitty-theme.conf.tpl, which /etc/xdg/kitty/kitty.conf includes) and the prompt
#      drawn as a powerline (themed/starship.toml.tpl): neither is in the folder of an account set up before.
#  12  the desktop styles (themed/shell-styles.css.tpl, themed/style-*.gtk.css.tpl), the themes' fonts and their icon
#      packs (style.theme, font.theme, icons.theme): written only when the theme is applied again.
#  13  the palette layer in the GTK stylesheets (themed/palette-gtk*.css.tpl, in place of the Sweet import), each
#      style's shape (themed/style-base.gtk.css.tpl), the themes' monospace fonts (mono.theme), the nine icon packs
#      and the shell's search box, Bazaar and clipboard colours: all written only when the theme is applied again.
#  14  the Pixel icon pack gone from the image: an account that chose it is put back on the theme's own pack, and
#      Gruvbox, whose pack it was, now names Duotone.
#  15  Amethystora's own pointers (branding/cursors.mjs) in place of Breeze: Halo on a dark theme, Ink on a light one,
#      and the shake-to-find ring's colour (themed/shell-pointer.css.tpl), written only when the theme is applied again.
#  16  Daylight wallpapers and the twelve accents: the theme is applied again (the accent and Daylight are read by it)
#      and, for an account still on the theme's first picture, Daylight shown (amethystora-theme daylight adopt).
#  17  Round 4's shell colours (themed/shell-agent.css.tpl, shell-quit.css.tpl, and the other groups' of that round),
#      written only when the theme is applied again.
#  18  Quick settings tiles with an arrow as one card in every style (themed/shell-styles.css.tpl), written only when the
#      theme is applied again.
#  19  The top bar's empty workspaces as dim as a label may be and still read (themed/shell-workspaces.css.tpl, the
#      palette's dim_alpha, 4.5:1), written only when the theme is applied again.
#  20  The dark Amethystora theme's Daylight day, a dark amethyst in place of the light theme's near-white facets: an
#      account's daylight.list is a copy made when the theme is applied, and its slideshow is written from that copy.

source /usr/lib/amethystora/setup-services/libsetup.sh

# The version the account was at, before version-script writes this one. From 7 on the pictures it was given are
# still in the image, so an update that only changes the icons must leave the wallpaper as the account has it, one it
# chose in GNOME Settings included: the theme is applied again without picking a new one.
PREVIOUS="$(jq -r '.version.user.theme // 0' "${SETUP_CHECKER_FILE}" 2>/dev/null || echo 0)"

version-script theme user 20 || exit 0

[[ "$(cat "${XDG_CONFIG_HOME:-${HOME}/.config}/amethystora/icons" 2>/dev/null)" == candy-icons ]] &&
    rm -f "${XDG_CONFIG_HOME:-${HOME}/.config}/amethystora/icons"
[[ "$(cat "${XDG_CONFIG_HOME:-${HOME}/.config}/amethystora/icons" 2>/dev/null)" == amethystora-pixel ]] &&
    rm -f "${XDG_CONFIG_HOME:-${HOME}/.config}/amethystora/icons"

set -xeuo pipefail

if [[ "${PREVIOUS}" =~ ^[0-9]+$ ]] && ((PREVIOUS >= 7)); then
    export AMETHYSTORA_THEME_SKIP_BACKGROUND=1
fi
# A new account starts on the theme an image of one's own baked in (amethystora-setup bake), else Amethystora's
AMETHYSTORA_THEME_QUIET=1 /usr/bin/amethystora-theme set "$(/usr/bin/amethystora-theme current 2>/dev/null ||
    head -n1 /usr/share/amethystora/image-setup/theme 2>/dev/null || echo amethystora)"
# Below 7 the theme was given its first picture again just now, Daylight included; from 7 on, an account still on the
# theme's first picture is shown it by the time of day, and a picture of its own is kept
if [[ -n ${AMETHYSTORA_THEME_SKIP_BACKGROUND:-} ]]; then
    /usr/bin/amethystora-theme daylight adopt || true
    /usr/bin/amethystora-theme daylight refresh || true
fi
