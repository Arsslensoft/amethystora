#!/usr/bin/bash

echo "::group:: ===$(basename "$0")==="

set -eoux pipefail

# No Universal Blue names left in paths or text (07-debrand.sh)
python3 /ctx/build_files/shared/debrand.py --check

# ame: the commands in /usr/share/amethystora/just, in groups. ujust runs the same ones, and also under
# the names they had before the groups.
for i in bin/ame bin/ujust share/amethystora/just/{00-entry,agent,apps,backup,changelog,desktop,report,security,setup,system,update}.just \
    share/bash-completion/completions/ame share/fish/vendor_completions.d/ame.fish share/zsh/site-functions/_ame ; do
   stat /usr/$i
done
[[ "$(ame --summary)" == "$(ujust --summary)" ]]
# just pastes {{ PARAM }} into a recipe's text before the shell reads it, so a " in one broke out of its
# quotes: a recipe takes its parameters as positional arguments ([positional-arguments], "$1") instead
grep -n '{{ [A-Z_]* }}' /usr/share/amethystora/just/*.just && false
grep -qx "alias setup-backup := backup" <<<"$(ujust --show setup-backup)"
# The fish completions answer, with no error before a subcommand is typed or after: a condition such as
# `test (...) = x` fails while there is no word yet. --no-config loads none of them, so they are sourced.
for line in "ame " "ame security " "ame pkg " "amethystora-pkg " "amethystora-pkg containers "; do
    FISH_COMPLETE="source /usr/share/fish/vendor_completions.d/amethystora-pkg.fish
        source /usr/share/fish/vendor_completions.d/ame.fish; complete -C '${line}'"
    [[ -z "$(fish --no-config -c "${FISH_COMPLETE}" 2>&1 >/dev/null)" ]]
    [[ -n "$(fish --no-config -c "${FISH_COMPLETE}" 2>/dev/null)" ]]
done
# Every command the image tells somebody to run is one: the report's, the notifications', the apps', the
# manual's and the agent skill's. A group named alone is one too.
grep -rhoIE --exclude-dir=node_modules '\bame [a-z][a-z0-9-]*( [a-z][a-z0-9-]*)?' /usr/bin/ame* \
    /usr/libexec/amethystora-* /ctx/apps/*/src /usr/lib/systemd/{system,user}/amethystora-* \
    /usr/share/amethystora /etc/amethystora /etc/profile.d/amethystora-greeting.sh | sort -u |
    while read -r _ first second; do
        ame --show "${first}" >/dev/null 2>&1 || ame --show "${first}::${second}" >/dev/null 2>&1 ||
            { [[ -z "${second}" ]] && ame --list "${first}" >/dev/null 2>&1; } ||
            { echo "the image names \"ame ${first} ${second}\", which is no command"; false; }
    done
# The debranding turns upstream's GitHub names into amethystora/..., an organisation that does not exist:
# the commands reach this image's repository through amethystora-image-repo, and no tap of upstream's
grep -rqE "github\.com/amethystora/|repos/amethystora/|tap \"?amethystora/" /usr/share/amethystora/just \
    /usr/share/amethystora/homebrew && false
grep -q "/usr/libexec/amethystora-image-repo" /usr/share/amethystora/just/changelog.just
grep -q "/usr/libexec/amethystora-image-repo" /usr/share/amethystora/just/report.just
# JetBrains Toolbox comes from JetBrains, and only when it matches the checksum JetBrains publishes
grep -q "checksumLink" /usr/share/amethystora/just/apps.just

test -f /usr/share/amethystora/homebrew/fonts.Brewfile
test -x /usr/bin/amethystora-fastfetch
# The fetch new terminals show (amethystora-fetch): the glint's frames, each as tall as the still logo
# so that it covers the one before, and the last of them the still logo, which fastfetch prints over it
test -x /usr/bin/amethystora-fetch
GLINT=(/usr/share/amethystora/logos/glint/*)
((${#GLINT[@]} > 1))
cmp -s "${GLINT[-1]}" /usr/share/amethystora/logos/symbols/amethystora
for frame in "${GLINT[@]}"; do
    [[ "$(wc -l <"${frame}")" == "$(wc -l </usr/share/amethystora/logos/symbols/amethystora)" ]]
done
# Logo in the text console colours (amethystora-greeting)
test -f /usr/share/amethystora/logos/console/amethystora
test -x /usr/libexec/amethystora-greeting
# ...which also draws the banner, so the upstream glow banner is not shipped; its tips are
command -v glow jq >/dev/null
jq -e . /usr/share/amethystora/greeting/style.json >/dev/null
test -f /usr/share/amethystora/image-info.json
test ! -e /etc/profile.d/amethystora-motd.sh
test ! -e /usr/bin/amethystora-motd
compgen -G "/usr/share/amethystora/motd/tips/*.md" >/dev/null
test -f /usr/lib/amethystora/setup-services/libsetup.sh

# If this file is not on the image bazaar will automatically be removed from users systems :(
# See: https://docs.flatpak.org/en/latest/flatpak-command-reference.html#flatpak-preinstall
test -f /usr/share/flatpak/preinstall.d/bazaar.preinstall
# ...which is also how ClamUI leaves the machines that have it: the Security app scans in its place
test ! -e /usr/share/flatpak/preinstall.d/clamui.preinstall
# The apps every machine comes with are installed the same way, on the first boot that has a network:
# the ISO carries none. Flatpak takes a group without a Branch to mean "master", which Flathub has not.
DEFAULT_FLATPAKS=/usr/share/flatpak/preinstall.d/amethystora.preinstall
grep -q "^\[Flatpak Preinstall org.gnome.Calculator\]$" "${DEFAULT_FLATPAKS}"
for file in /usr/share/flatpak/preinstall.d/*.preinstall; do
    [[ "$(grep -c '^\[Flatpak Preinstall ' "${file}")" == "$(grep -c '^Branch=' "${file}")" ]]
done
# It retries until it gets through, without holding up the boot while it waits or downloads
grep -qx "Type=exec" /usr/lib/systemd/system/flatpak-preinstall.service
grep -qx "StartLimitIntervalSec=0" /usr/lib/systemd/system/flatpak-preinstall.service
# ...and tells the sessions already there about the apps it put in, which flatpak run as root cannot: the
# ones that start over D-Bus did not open until the next login (25-every-app.sh on a new machine)
grep -q "flatpak preinstall -y && exec /usr/libexec/amethystora-reload-session-buses" /usr/lib/systemd/system/flatpak-preinstall.service
test -x /usr/libexec/amethystora-reload-session-buses

# Firefox is the browser: Fedora's package, in the image, and so not on the Flatpak list as well
test -f /usr/share/applications/org.mozilla.firefox.desktop
grep -q "^x-scheme-handler/https=org.mozilla.firefox.desktop" /etc/xdg/mimeapps.list
grep -rq "org.mozilla.firefox" /usr/share/flatpak/preinstall.d && false
# Déjà Dup left the list for Amethystora Backups. An account whose scheduled backups stopped with it is told
# once (user-setup.hooks.d/19-deja-dup.sh), in its own language (po/)
grep -rq "org.gnome.DejaDup" /usr/share/flatpak/preinstall.d && false
test -x /usr/libexec/amethystora-deja-dup-notice
# Mozilla lets Firefox be redistributed only unaltered, so the image adds no policy to it, and with
# no policy no extension
test ! -e /etc/firefox/policies/policies.json
# Fedora's start page is taken out of its preferences (04-packages.sh), which leaves Firefox's own
test -f /usr/lib64/firefox/browser/defaults/preferences/firefox-redhat-default-prefs.js
grep -rqs "start\.fedoraproject\.org" /usr/lib64/firefox/browser/defaults /usr/lib64/firefox/defaults \
    /usr/lib64/firefox/distribution && false
# Brave, which used to be the browser, is gone with the policy written for it
rpm -q brave-browser >/dev/null && false
test ! -e /etc/brave
# Firefox is pinned in the dash
FAVORITES="$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell favorite-apps)"
grep -q "'org.mozilla.firefox.desktop'" <<<"${FAVORITES}"
grep -qi "brave" <<<"${FAVORITES}" && false
# Every app pinned there is one the image or its first-boot Flatpaks bring (VSCodium on the developer image)
for favorite in $(grep -oE "[A-Za-z0-9._-]+\.desktop" <<<"${FAVORITES}"); do
    [[ "${favorite}" == codium.desktop ]] && continue
    test -f "/usr/share/applications/${favorite}" ||
        grep -qxF "[Flatpak Preinstall ${favorite%.desktop}]" /usr/share/flatpak/preinstall.d/*.preinstall
done

# Multimedia (05-override-install.sh): FFmpeg is Fedora's with RPM Fusion's libavcodec-freeworld, and no
# FFmpeg library in the image is a build FFmpeg itself calls unredistributable. negativo17's LC3plus,
# which has no licence to redistribute it, is gone, and aptX comes from RPM Fusion
for package in ffmpeg-free libavcodec-free fdk-aac-free libavcodec-freeworld pipewire-codec-aptx; do
    rpm -q "${package}" >/dev/null
done
for package in libavcodec libfdk-aac liblc3plus pipewire-libs-extra mpeghdec; do
    rpm -q "${package}" >/dev/null && false
done
find /usr/lib64 -name 'libav*.so.*' -type f -exec grep -laF 'nonfree and unredistributable' {} + | grep -q . && false
[[ "$(rpm -q --queryformat '%{VENDOR}' libavcodec-freeworld)" == "RPM Fusion" ]]

# No Fedora logo package is left (build.sh swaps each for its generic one)
[[ -z "$(rpm -qa 'fedora-logos*')" ]]
# Nor a Fedora release package. generic-release holds their place in the package database, which is
# what dnf reads the Fedora version from, and the release line no longer calls the system Fedora
[[ -z "$(rpm -qa 'fedora-release*')" ]]
rpm -q generic-release generic-release-common >/dev/null
rpm -q --whatprovides system-release "system-release(${FEDORA_MAJOR_VERSION})" >/dev/null
[[ "$(rpm -E %fedora)" == "${FEDORA_MAJOR_VERSION}" ]]
grep -q "^Amethystora release ${FEDORA_MAJOR_VERSION} " /etc/system-release
grep -qi "fedora" /etc/system-release /etc/system-release-cpe && false
test ! -e /usr/lib/swidtag/fedoraproject.org

# NordVPN is proprietary, with no right to redistribute it, so the image does not carry it
rpm -q nordvpn >/dev/null && false
rpm -q nordvpn-gui >/dev/null && false

# The VPNs people are given at work (04-packages.sh): a plugin for each, and PPTP left out for MS-CHAPv2.
# Fedora 45 retired the Fortinet and vpnc plugins
VPN_PLUGINS=(l2tp libreswan openconnect openvpn sstp)
((FEDORA_MAJOR_VERSION < 45)) && VPN_PLUGINS+=(fortisslvpn vpnc)
for plugin in "${VPN_PLUGINS[@]}"; do
    rpm -q "NetworkManager-${plugin}" "NetworkManager-${plugin}-gnome" >/dev/null
    test -f "/usr/lib/NetworkManager/VPN/nm-${plugin}-service.name"
done
rpm -q NetworkManager-pptp pptp >/dev/null && false
# Nothing runs while no IPsec or L2TP VPN is connected: the plugins start pluto and the L2TP daemon themselves
for unit in ipsec.service xl2tpd.service; do
    [[ "$(systemctl is-enabled "${unit}" 2>/dev/null || true)" != enabled ]]
done
# L2TP's two modules load on demand (17-cleanup.sh); l2tp_eth, which no VPN uses, stays blacklisted by Fedora's
# kernel, which proves the folder is where the other two were taken from
test ! -e /etc/modprobe.d/l2tp_netlink-blacklist.conf
test ! -e /etc/modprobe.d/l2tp_ppp-blacklist.conf
grep -qx "blacklist l2tp_eth" /etc/modprobe.d/l2tp_eth-blacklist.conf

# kitty is the terminal: first for xdg-terminal-exec (Super+Return) in each of its lists, with Ptyxis behind it, and
# its launcher says how xdg-terminal-exec runs a command in it
for list in /etc/xdg/xdg-terminals.list /etc/xdg/gnome-xdg-terminals.list /usr/share/xdg-terminal-exec/gnome-xdg-terminals.list; do
    [[ "$(grep -v '^#' "${list}" | head -1)" == "kitty.desktop" ]]
done
rpm -q kitty >/dev/null
grep -q "^X-TerminalArgExec=--$" /usr/share/applications/kitty.desktop
# ...and it is the one Terminal in the app grid, with Ptyxis's launcher out of sight but not Hidden
grep -qx "Name=Terminal" /usr/share/applications/kitty.desktop
grep -qx "NoDisplay=true" /usr/share/applications/org.gnome.Ptyxis.desktop
[[ "$(grep -cx "NoDisplay=true" /usr/share/applications/kitty.desktop)" == 0 ]]
[[ "$(grep -cx "Hidden=true" /usr/share/applications/org.gnome.Ptyxis.desktop)" == 0 ]]
# ...no launcher shows kitty's name or icon, and asking for Ptyxis's bus name (Files' Open in Console) opens the default
grep -qx "Hidden=true" /usr/share/applications/kitty-open.desktop
grep -q "^Keywords=.*shell" /usr/share/applications/kitty.desktop
[[ "$(grep -ci ptyxis /usr/share/applications/kitty.desktop)" == 0 ]]
grep -qx "Exec=/usr/libexec/amethystora-terminal-here" /usr/share/dbus-1/services/org.gnome.Ptyxis.service
grep -qx "terminal='kitty'" /usr/share/glib-2.0/schemas/zz0-amethystora-modifications.gschema.override
# The image's kitty.conf loads, and takes the theme's colours from the account's own folder
grep -q "^include ~/.config/amethystora/current/theme/kitty-theme.conf$" /etc/xdg/kitty/kitty.conf
kitty +runpy '
from kitty.cli import create_default_opts
o = create_default_opts()
assert o.font_size == 11.0 and o.update_check_interval == 0, (o.font_size, o.update_check_interval)'

# Time and the terminal (plans/ui-ideas/spec-time.md). kitty says when a long command ends while its window is behind
# (its own setting), and reads the options `ame desktop` writes for the account last, which user-setup makes for
# every account so that kitty never warns about a missing include
kitty +runpy '
from kitty.cli import create_default_opts
n = create_default_opts().notify_on_cmd_finish
assert (n.when, n.duration, n.action) == ("unfocused", 10.0, "notify"), n'
[[ "$(grep -v '^#' /etc/xdg/kitty/kitty.conf | grep -v '^$' | tail -n1)" == "include ~/.config/amethystora/kitty.conf" ]]
test -x /usr/share/amethystora/user-setup.hooks.d/28-kitty-options.sh
# ...the search's time and reminder answers and the pill's meetings, whose link the helper reads from Evolution Data
# Server with the typelibs it needs, by rules that count a call's link and nothing else
UI_EXTENSION=/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io
for file in modules/search/times.js modules/search/timesProvider.js modules/search/remind.js \
    modules/search/remindProvider.js modules/pill/meeting.js; do
    test -s "${UI_EXTENSION}/${file}"
done
grep -q "'present', 'meeting', 'link'" "${UI_EXTENSION}/modules/pill/activities.js"
test -x /usr/libexec/amethystora-meeting-link
python3 -I -c 'import gi
for name, version in (("EDataServer", "1.2"), ("ECal", "2.0"), ("ICalGLib", "3.0")):
    gi.require_version(name, version)
from gi.repository import ECal, EDataServer, ICalGLib'
[[ "$(printf '{"location": "https://meet.example.org/abc-defg"}' | /usr/libexec/amethystora-meeting-link --rules)" == \
    '{"link": "https://meet.example.org/abc-defg", "calendar": ""}' ]]
[[ "$(printf '{"conference": ["https://example.org/x"]}' | /usr/libexec/amethystora-meeting-link --rules)" == *'"link": "https://example.org/x"'* ]]
[[ -z "$(printf '{"url": "https://example.org/about", "description": "menu at http://example.org/j/1"}' |
    /usr/libexec/amethystora-meeting-link --rules)" ]]
# ...and a missing command: the index of Fedora's commands, the helper answering from it as an account would and never
# naming a recipe the image lacks, and the shells' hooks, which only an account that turned it on gets
test -x /usr/libexec/amethystora-missing-command
[[ "$(wc -l </usr/share/amethystora/missing-command/fedora.tsv)" -gt 10000 ]]
grep -qx "$(printf 'rg\tripgrep')" /usr/share/amethystora/missing-command/fedora.tsv
MISSING_HOME="$(mktemp -d)"
HOME="${MISSING_HOME}" /usr/libexec/amethystora-missing-command rg | grep -q "ame pkg fedora install ripgrep"
HOME="${MISSING_HOME}" /usr/libexec/amethystora-missing-command steam | grep -q "ame apps gaming"
if HOME="${MISSING_HOME}" /usr/libexec/amethystora-missing-command "rm -rf" >/dev/null; then
    echo "amethystora-missing-command answered for a name that is not a command's"
    exit 1
fi
grep -q "^gaming ACTION" /usr/share/amethystora/just/apps.just
grep -q "^opentabletdriver ACTION" /usr/share/amethystora/just/apps.just
grep -q "^devmode ACTION" /usr/share/amethystora/just/system.just
for recipe in command-done missing-command; do
    grep -q "^${recipe} ACTION=\"\":" /usr/share/amethystora/just/desktop.just
done
HOOK=/etc/profile.d/zz-amethystora-missing-command.sh
[[ -z "$(HOME="${MISSING_HOME}" bash --norc -i -c "source ${HOOK}; declare -F command_not_found_handle" 2>/dev/null)" ]]
mkdir -p "${MISSING_HOME}/.config/amethystora"
touch "${MISSING_HOME}/.config/amethystora/missing-command"
[[ -n "$(HOME="${MISSING_HOME}" bash --norc -i -c "source ${HOOK}; declare -F command_not_found_handle" 2>/dev/null)" ]]
grep -q "command_not_found_handler()" /etc/zsh/zshrc
zsh -n /etc/zsh/zshrc
fish --no-execute /usr/share/fish/vendor_conf.d/amethystora-missing-command.fish
rm -rf "${MISSING_HOME}"

# Animated boot splash: the script theme, the images it loads, and both in the initramfs, which shows the
# splash up to and including the LUKS password prompt
[[ "$(plymouth-set-default-theme)" == "amethystora" ]]
rpm -q plymouth-plugin-script >/dev/null
test -f /usr/share/plymouth/themes/amethystora/amethystora.script
for image in crown halo dust gem light side-0 stem wordmark credit field dot; do
    test -f "/usr/share/plymouth/themes/amethystora/${image}.png"
done
INITRAMFS_FILES="$(lsinitrd /lib/modules/*/initramfs.img)"
grep -q "plymouth/script.so" <<<"${INITRAMFS_FILES}"
grep -q "themes/amethystora/amethystora.script" <<<"${INITRAMFS_FILES}"
# Boot and login text in Inter: fc-match falls back to another family when it is missing
[[ "$(fc-match -f '%{family[0]}' 'Inter')" == "Inter" ]]
grep -q "^Font=Inter " /usr/share/plymouth/themes/amethystora/amethystora.plymouth

# Boot menu: the theme ships read-only in the image and is copied to /boot by amethystora-grub-theme,
# because /boot is not part of a bootc image
test -x /usr/bin/amethystora-grub-theme
test -x /usr/share/amethystora/system-setup.hooks.d/30-grub-theme.sh
for file in theme.txt background.png logo.png select_c.png select_e.png select_w.png icons/fedora.png; do
    test -s "/usr/share/grub/themes/amethystora/${file}"
done
# The font theme.txt asks for, built from Inter by 06-branding.sh: without it the menu falls back
# to GRUB's own and the layout no longer lines up
test -s /usr/share/grub/themes/amethystora/font16.pf2
grep -aq "Inter Regular 16" /usr/share/grub/themes/amethystora/font16.pf2
grep -qE '^[[:space:]]*item_font = "Inter Regular 16"$' /usr/share/grub/themes/amethystora/theme.txt
grep -q '^desktop-image: "background.png"$' /usr/share/grub/themes/amethystora/theme.txt
# The theme is off until someone asks for it, so nothing here may write to /boot at build time
test -e /boot/grub2/themes/amethystora && false

# Amethystora wallpapers are the GNOME default
test -f /usr/share/backgrounds/amethystora/amethystora-facets-l.png
test -f /usr/share/backgrounds/amethystora/amethystora-facets-d.png
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.desktop.background picture-uri-dark)" == "'file:///usr/share/backgrounds/amethystora/amethystora-facets-d.png'" ]]
# ...and every one a theme cycles through or GNOME's picker offers is on the image. The PNGs are
# drawn by branding/generate.mjs, so one named here and never rendered stops the build. Every
# shipped theme lists pictures of its own, or it would open on the palette glow drawn from
# wallpaper.svg.tpl, which looks like a plain colour.
for theme in /usr/share/amethystora/themes/*/; do
    grep -q '^/' "${theme}backgrounds.list"
done
for list in /usr/share/amethystora/themes/*/backgrounds.list; do
    grep -v '^[[:space:]]*\(#\|$\)' "${list}" | xargs -r -n1 test -f
done
# Every picture in a theme's folder is accounted for, with its licence, in CREDITS.md
for picture in /usr/share/backgrounds/amethystora/*/*; do
    grep -qF "\`${picture#/usr/share/backgrounds/amethystora/}\`" /usr/share/backgrounds/amethystora/CREDITS.md
done
sed -n 's:.*<filename\(-dark\)\?>\(.*\)</filename\(-dark\)\?>.*:\2:p' /usr/share/gnome-background-properties/amethystora.xml |
    xargs -r -n1 test -f

# Login screen background (12-login-screen.sh): the crown the boot splash ends on, with no gem in
# it. GNOME reads the login background from the shell's own stylesheet and nowhere else,
# so both the picture and the rule naming it live inside the theme's gresource.
#
# The bundle itself is checked in 12-login-screen.sh, at the point it is rebuilt, and not here:
# gresource comes from glib2-devel, which that script takes back out of the image when it is done,
# so reading the bundle at this point would mean installing a toolchain again to re-check something
# nothing since has touched.
test -s /usr/share/backgrounds/amethystora/amethystora-login.png

# Logos (06-branding.sh). The upstream layers overwrite Fedora's logo files with their own pictures,
# under names that say nothing about Universal Blue for 07-debrand.sh to catch, so each
# one is checked here against the Amethystora artwork it has to be. These are what the login screen,
# the Settings About page and Fedora's fallback splash draw.
FEDORA_NAMED_LOGOS=(
    "/usr/share/pixmaps/fedora-gdm-logo.png:/usr/share/pixmaps/amethystora-wordmark-glow.png"
    "/usr/share/pixmaps/fedora-logo-small.png:/usr/share/pixmaps/amethystora-wordmark-small.png"
    "/usr/share/pixmaps/fedora-logo.png:/usr/share/pixmaps/amethystora-wordmark.png"
    "/usr/share/pixmaps/fedora_logo_med.png:/usr/share/pixmaps/amethystora-wordmark-medium.png"
    "/usr/share/pixmaps/fedora_whitelogo_med.png:/usr/share/pixmaps/amethystora-wordmark-white.png"
    "/usr/share/pixmaps/fedora-logo-icon.png:/usr/share/pixmaps/amethystora-logo-512.png"
    "/usr/share/pixmaps/fedora-logo-sprite.png:/usr/share/pixmaps/amethystora-logo-400.png"
    "/usr/share/pixmaps/system-logo-white.png:/usr/share/pixmaps/amethystora-logo-white.png"
    "/usr/share/icons/hicolor/scalable/places/fedora-logo-sprite.svg:/usr/share/icons/hicolor/scalable/apps/amethystora-logo.svg"
    "/usr/share/icons/hicolor/scalable/places/fedora_white_logo.svg:/usr/share/icons/hicolor/scalable/apps/amethystora-logo.svg"
    "/usr/share/icons/hicolor/scalable/places/fedora_whitelogo.svg:/usr/share/icons/hicolor/scalable/apps/amethystora-logo.svg"
    "/usr/share/plymouth/themes/spinner/silverblue-watermark.png:/usr/share/plymouth/themes/spinner/watermark.png"
)
for entry in "${FEDORA_NAMED_LOGOS[@]}"; do
    cmp -s "${entry%%:*}" "${entry#*:}"
done
# GDM's own setting names the Amethystora file, for the case where it is read instead of the path
# above. The wordmark in white, because the greeter's background is dark, and the variant whose
# stone is lit, so the logo carries on from the splash rather than restating it cold.
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.login-screen logo)" == "'/usr/share/pixmaps/amethystora-wordmark-glow.png'" ]]

# Top bar: the Logo Menu (logomenu@aryan_k), with its sixty-five other makers' logos, its settings and its two
# helpers that installed an app from a click, is gone for the image's own logo menu (below, beside the
# extensions written for this image)
test ! -e /usr/share/gnome-shell/extensions/logomenu@aryan_k
grep -q "logomenu@aryan_k" /usr/share/glib-2.0/schemas/zz0-amethystora-modifications.gschema.override && false
test ! -e /etc/dconf/db/distro.d/04-amethystora-logomenu-extension
test ! -e /usr/bin/missioncenter-helper
test ! -e /usr/bin/distroshelf-helper

# No GNOME Shell extension is built from a git submodule any more (build-gnome-extensions.sh), and the phone link
# is Amethystora's own program (plans/PLAN-amethystora-connect.md) in place of GSConnect and its Files extension:
# a user service nobody has enabled, its app and the module of Amethystora UI that shows it in the top bar
test -d /usr/share/gnome-shell/extensions/tmp && false
for package in nautilus-gsconnect gnome-shell-extension-gsconnect; do
    rpm -q "${package}" >/dev/null 2>&1 && false
done
test -x /usr/libexec/amethystora-connect
test -x /usr/bin/amethystora-phone
test -f /usr/lib/systemd/user/amethystora-connect.service
[[ "$(systemctl --global is-enabled amethystora-connect.service 2>/dev/null)" != enabled ]]
# ...which D-Bus never starts by itself (no activation file): the program listens only while the switch is on
test ! -e /usr/share/dbus-1/services/org.amethystora.Connect.service
grep -q '^\[Install\]' /usr/lib/systemd/user/amethystora-connect.service
grep -q '^WantedBy=graphical-session.target' /usr/lib/systemd/user/amethystora-connect.service
grep -qE '^(MemoryMax|TasksMax|LimitNOFILE)=' /usr/lib/systemd/user/amethystora-connect.service
# ...and no setting that puts it in a user namespace of its own, from which it cannot read the Phone app's or GNOME
# Shell's /proc/PID/exe and so refuses both (dbus.rs)
[[ -z "$(grep -E '^(Protect(System|Home|Kernel[A-Za-z]*|ControlGroups|Clock|Hostname)|ReadWritePaths|ReadOnlyPaths|InaccessiblePaths|Private[A-Za-z]*|CapabilityBoundingSet)=' /usr/lib/systemd/user/amethystora-connect.service)" ]]
# ...whose only settings are the name phones see, the two ways the clipboard may be shared, both off, and where
# pictures from a phone go, Pictures/Phone (plans/beyond/spec-phone-files-first.md 2.3), and whether other computers
# pair too, off (plans/beyond/spec-r4-later.md A.2), and music from either side and the phone as a slide clicker, both off
# (plans/ui-ideas/spec-phone.md)
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys org.amethystora.Connect | sort | paste -sd ' ')" == \
    "clicker clipboard-receive clipboard-send codes computers device-name music photo-folder" ]]
for key in music clicker; do
    [[ "$(GSETTINGS_BACKEND=memory gsettings get org.amethystora.Connect "${key}")" == false ]]
    grep -q "item desktop phone-${key} switch" /usr/libexec/amethystora-control-list
done
grep -q 'exec /usr/bin/amethystora-phone ring' /usr/share/amethystora/just/desktop.just
# Ringing, the phone's players and the clicker reach the module only through the program; Present presses the clicker's
# three keys and no other (the program sends only these three words)
for name in Ring MediaAction Players Presenting Clicker; do
    grep -q "name=\"${name}\"" /usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules/connect/interfaces.js
done
grep -q "^const CLICKER_KEYS = {next: Clutter.KEY_Page_Down, previous: Clutter.KEY_Page_Up, blank: Clutter.KEY_b};" \
    /usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules/present.js
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.amethystora.Connect clipboard-send)" == false ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.amethystora.Connect clipboard-receive)" == false ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.amethystora.Connect photo-folder)" == "'pictures'" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.amethystora.Connect computers)" == false ]]
grep -q 'gsettings set org.amethystora.Connect photo-folder downloads' /usr/share/amethystora/just/desktop.just
grep -q 'gsettings set org.amethystora.Connect computers true' /usr/share/amethystora/just/desktop.just
# The phone's signal in the top bar comes from the program's two device properties, which the module reads
for property in SignalStrength NetworkType; do
    grep -q "<property name=\"${property}\"" /usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules/connect/interfaces.js
done
# Files' scripts menu (spec 3): the two helpers and the hook that writes their entries once per account, as marked
# wrappers that pass Files' arguments on after --; the helpers read argv only, never Files' variables of names, and
# never give a name to a shell
test -x /usr/libexec/amethystora-folder-colour
test -x /usr/libexec/amethystora-back-in-time
test -x /usr/share/amethystora/user-setup.hooks.d/24-files-menu.sh
grep -q "^version-script files-menu user " /usr/share/amethystora/user-setup.hooks.d/24-files-menu.sh
grep -q 'exec %s -- "$@"' /usr/share/amethystora/user-setup.hooks.d/24-files-menu.sh
grep -vh '^ *#' /usr/libexec/amethystora-folder-colour /usr/libexec/amethystora-back-in-time |
    grep -qE 'NAUTILUS_SCRIPT_|eval |bash -c|sh -c' && false
grep -q 'metadata::custom-icon-name "folder-${colour}"' /usr/libexec/amethystora-folder-colour
bash -n /usr/libexec/amethystora-folder-colour
bash -n /usr/libexec/amethystora-back-in-time
[[ "$(/usr/libexec/amethystora-folder-colour purple -- /tmp >/dev/null 2>&1; echo $?)" == 64 ]]
# Copy without hidden data (plans/beyond/spec-r4-privacy.md, R4-18): the third entry, its helper, which reads argv only,
# never runs mat2 on the original (a copy in a private folder) and never writes over a file, and mat2 with what reads a
# picture's metadata for it
test -x /usr/libexec/amethystora-clean-copy
bash -n /usr/libexec/amethystora-clean-copy
grep -q "^version-script files-menu user 4 " /usr/share/amethystora/user-setup.hooks.d/24-files-menu.sh
grep -q '"Copy without hidden data")" /usr/libexec/amethystora-clean-copy$' /usr/share/amethystora/user-setup.hooks.d/24-files-menu.sh
grep -vh '^ *#' /usr/libexec/amethystora-clean-copy | grep -qE 'NAUTILUS_SCRIPT_|eval |bash -c|sh -c|--inplace' && false
grep -q 'mv -n -- ' /usr/libexec/amethystora-clean-copy
[[ "$(/usr/libexec/amethystora-clean-copy /tmp/x >/dev/null 2>&1; echo $?)" == 64 ]]
rpm -q mat2 perl-Image-ExifTool >/dev/null
mat2 -l | grep -q 'image/jpeg'
# ...and every icon pack draws the six colours (the packs' equal counts are held below)
for pack in /usr/share/icons/amethystora-{glass,glass-light,neon,neon-light,duotone,line,clay,muted}; do
    for colour in amethyst sky jade amber coral slate; do
        test -s "${pack}/places/scalable/folder-${colour}.svg"
    done
done
# ame desktop phone turns the service on and off, not an extension
grep -q "amethystora-connect.service" /usr/share/amethystora/just/desktop.just
grep -qi "gsconnect" /usr/share/amethystora/just/desktop.just && false
test -x /usr/share/amethystora/user-setup.hooks.d/21-phone-link.sh
# ...and an account that had paired a computer keeps it, the switch for computers turned on once for it
test -x /usr/share/amethystora/user-setup.hooks.d/26-phone-computers.sh
grep -q '^version-script phone-computers user ' /usr/share/amethystora/user-setup.hooks.d/26-phone-computers.sh
test -x /usr/libexec/amethystora-phone-link-notice
test ! -e /usr/share/amethystora/user-setup.hooks.d/18-home-networks.sh

# No extension ships turned off: GNOME Classic and the four it alone turned on are gone
test ! -e /usr/share/wayland-sessions/gnome-classic.desktop
for uuid in apps-menu places-menu window-list launch-new-instance; do
    test ! -e "/usr/share/gnome-shell/extensions/${uuid}@gnome-shell-extensions.gcampax.github.com"
done

# Every extension has to be readable by the account that logs in, not just by the root this build
# runs as. GNOME Shell reads metadata.json as the user; one it cannot open is not a broken
# extension, it is no extension at all, missing from the shell and from Extension Manager. The
# extensions.gnome.org downloads carry metadata.json as 0600, which build-gnome-extensions.sh
# undoes right after unpacking them.
for extension in /usr/share/gnome-shell/extensions/*/; do
    [[ -z "$(find "${extension}" -type f ! -perm -o=r)" ]]
    [[ -z "$(find "${extension}" -type d ! -perm -o=rx)" ]]
done

# Extension Manager's row and GNOME's Extensions app show the image's version for each of its own
for metadata in /usr/share/gnome-shell/extensions/amethystora-*@arsslensoft.github.io/metadata.json; do
    jq -e '(.version | type == "number") and (.["version-name"] | type == "string" and length > 0)' "${metadata}"
done

# Every extension ships its licence, in its own folder or in /usr/share/licenses/<uuid>: the GPL asks
# for the text to travel with the program. One installed from a Fedora package carries it in the package's
# licence folder, or, like those of gnome-shell-extensions, in that of a sibling from the same source
# package (gnome-shell-extension-common)
for extension in /usr/share/gnome-shell/extensions/*/; do
    uuid="$(basename "${extension}")"
    srpm="$(rpm -qf --qf '%{SOURCERPM}' "${extension%/}" 2>/dev/null)" || srpm=""
    compgen -G "${extension}[Ll][Ii][Cc][Ee][Nn][Ss][Ee]*" >/dev/null ||
        compgen -G "${extension}COPYING*" >/dev/null ||
        compgen -G "/usr/share/licenses/${uuid}/*" >/dev/null ||
        { [[ -n ${srpm} ]] && [[ -n "$(rpm -qa --qf '%{SOURCERPM} %{NAME}\n' |
            awk -v s="${srpm}" '$1 == s { print $2 }' | xargs -r rpm -qL | grep '^/')" ]]; } ||
        { echo "No licence shipped for the extension ${uuid}"; exit 1; }
done

GNOME_MAJOR="$(gnome-shell --version | grep -oE '[0-9]+' | head -n1)"
# Amethystora Transitions, written for this image: enabled, its settings where gsettings finds them,
# and claiming this GNOME, 50 and 51 both. It calls GNOME Shell's paint_to_content, which took a new
# argument in GNOME 50, so read that call again before adding a version to its shell-version.
jq -e --arg shell "${GNOME_MAJOR}" '.["shell-version"] | index($shell)' \
    /usr/share/gnome-shell/extensions/amethystora-transitions@arsslensoft.github.io/metadata.json
jq -e '.["shell-version"] | index("50") and index("51")' \
    /usr/share/gnome-shell/extensions/amethystora-transitions@arsslensoft.github.io/metadata.json
grep -q "'amethystora-transitions@arsslensoft.github.io'" /usr/share/glib-2.0/schemas/zz0-amethystora-modifications.gschema.override
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-transitions style)" == "'grow'" ]]
# Amethystora Widgets, also written for this image: enabled, claiming this GNOME, and every example
# amethystora-widgets new copies passing the same check as amethystora-widgets run. The check runs in a
# home of its own, with settings in memory as there is no session here
WIDGETS_EXTENSION=/usr/share/gnome-shell/extensions/amethystora-widgets@arsslensoft.github.io
jq -e --arg shell "${GNOME_MAJOR}" '.["shell-version"] | index($shell)' "${WIDGETS_EXTENSION}/metadata.json"
grep -q "'amethystora-widgets@arsslensoft.github.io'" /usr/share/glib-2.0/schemas/zz0-amethystora-modifications.gschema.override
test -x /usr/bin/amethystora-widgets
WIDGETS_HOME="$(mktemp -d)"
for example in /usr/share/amethystora/widgets/examples/*/; do
    test -x "${example}widget.sh"
    HOME="${WIDGETS_HOME}" GSETTINGS_BACKEND=memory gjs -m "${WIDGETS_EXTENSION}/check.js" "${example}"
done
# A widget on the desktop (plans/beyond/spec-r4-capture.md, R4-19): the same check passes it, a bar position there fails,
# the bar leaves it to Amethystora UI's deskwidgets module, which runs it with this extension's own runner
DESK_WIDGET="${WIDGETS_HOME}/desk"
mkdir -p "${DESK_WIDGET}"
printf '#!/usr/bin/bash\necho "14:32"\n' >"${DESK_WIDGET}/widget.sh"
chmod +x "${DESK_WIDGET}/widget.sh"
echo '{"name": "Clock", "command": "./widget.sh", "place": "desktop", "position": "top-left", "size": "large"}' \
    >"${DESK_WIDGET}/widget.json"
HOME="${WIDGETS_HOME}" GSETTINGS_BACKEND=memory gjs -m "${WIDGETS_EXTENSION}/check.js" "${DESK_WIDGET}" >/dev/null
echo '{"name": "Clock", "command": "./widget.sh", "place": "desktop", "position": "left"}' >"${DESK_WIDGET}/widget.json"
HOME="${WIDGETS_HOME}" GSETTINGS_BACKEND=memory gjs -m "${WIDGETS_EXTENSION}/check.js" "${DESK_WIDGET}" >/dev/null 2>&1 && false
grep -q "widget.manifest?.place === 'desktop'" "${WIDGETS_EXTENSION}/extension.js"
grep -q "get_child('runner.js')" /usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules/deskwidgets.js
rm -rf "${WIDGETS_HOME}"
# Amethystora Clipboard, Clipboard Indicator's copy that seals its history: enabled, claiming this GNOME, its
# settings where gsettings finds them and Control's command for keep-history there, nothing of Clipboard
# Indicator left, and the sealing tried with a stand-in for the keyring (test-clipboard.sh)
CLIPBOARD_EXTENSION=/usr/share/gnome-shell/extensions/amethystora-clipboard@arsslensoft.github.io
jq -e --arg shell "${GNOME_MAJOR}" '.["shell-version"] | index($shell)' "${CLIPBOARD_EXTENSION}/metadata.json"
grep -q "'amethystora-clipboard@arsslensoft.github.io'" /usr/share/glib-2.0/schemas/zz0-amethystora-modifications.gschema.override
test ! -e /usr/share/gnome-shell/extensions/clipboard-indicator@tudmotu.com
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-clipboard keep-history)" == "'always'" ]]
GSETTINGS_BACKEND=memory gsettings list-keys org.gnome.shell.extensions.amethystora-clipboard | grep -qx cache-only-favorites && false
GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-clipboard excluded-apps | grep -qF "'org.keepassxc.KeePassXC'"
test -x /usr/libexec/amethystora-clipboard-seal
bash /ctx/build_files/shared/test-clipboard.sh
# Amethystora Logo Menu, the gem at the left of the top bar: enabled, claiming this GNOME (the Logo Menu it
# replaced claimed none past 50 and would have gone with no failed build), with its licence. It draws the image's
# own gem; every app it names ships in the image or comes with every machine, and each command is installed.
# Its labels are translated in every language the image has a catalog for, read from the catalog: "Terminal" is
# the same word in three of them.
LOGO_MENU_EXTENSION=/usr/share/gnome-shell/extensions/amethystora-logo-menu@arsslensoft.github.io
jq -e --arg shell "${GNOME_MAJOR}" '.["shell-version"] | index($shell)' "${LOGO_MENU_EXTENSION}/metadata.json"
grep -q "'amethystora-logo-menu@arsslensoft.github.io'" /usr/share/glib-2.0/schemas/zz0-amethystora-modifications.gschema.override
test -s /usr/share/licenses/amethystora-logo-menu@arsslensoft.github.io/LICENSE
test -s "$(grep -oE "^const LOGO = '[^']+'" "${LOGO_MENU_EXTENSION}/extension.js" | cut -d "'" -f 2)"
for app in $(grep -oE "app: '[^']+'" "${LOGO_MENU_EXTENSION}/extension.js" | cut -d "'" -f 2); do
    test -f "/usr/share/applications/${app}" ||
        grep -qxF "[Flatpak Preinstall ${app%.desktop}]" /usr/share/flatpak/preinstall.d/*.preinstall ||
        { echo "The logo menu opens ${app}, which no machine has"; exit 1; }
done
for command in $(grep -oE "argv: \['[^']+'" "${LOGO_MENU_EXTENSION}/extension.js" | cut -d "'" -f 2); do
    command -v "${command}" >/dev/null
done
mapfile -t LOGO_MENU_LABELS < <(grep -oE "_\('[^']+'\)" "${LOGO_MENU_EXTENSION}/extension.js" | cut -d "'" -f 2)
[[ "${#LOGO_MENU_LABELS[@]}" == 11 ]]
for po in /ctx/po/*.po; do
    mo="/usr/share/locale/$(basename "${po}" .po)/LC_MESSAGES/amethystora.mo"
    python3 -c 'import gettext, sys
catalog = gettext.GNUTranslations(open(sys.argv[1], "rb"))._catalog
missing = [label for label in sys.argv[2:] if not catalog.get(label)]
sys.exit(f"{sys.argv[1]} does not translate {missing}" if missing else 0)' "${mo}" "${LOGO_MENU_LABELS[@]}"
done
# Amethystora UI, the desktop's own modules (plans/PLAN-amethystora-ui.md): enabled, under the image's licence,
# claiming this GNOME, and every module it imports is there. The third-party extensions it replaces are gone.
UI_EXTENSION=/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io
jq -e --arg shell "${GNOME_MAJOR}" '.["shell-version"] | index($shell)' "${UI_EXTENSION}/metadata.json"
grep -q "'amethystora-ui@arsslensoft.github.io'" /usr/share/glib-2.0/schemas/zz0-amethystora-modifications.gschema.override
test -s /usr/share/licenses/amethystora-ui@arsslensoft.github.io/LICENSE
for module in $(grep -oE "from './modules/[a-z]+\.js'" "${UI_EXTENSION}/extension.js" | cut -d / -f 3 | cut -d . -f 1); do
    test -s "${UI_EXTENSION}/modules/${module}.js"
done
for retired in just-perfection-desktop@just-perfection space-bar@luchrioh tophat@fflewddur.github.io \
    appindicatorsupport@rgcjonas.gmail.com caffeine@patapon.info search-light@icedman.github.com \
    gnome-fuzzy-app-search@gnome-shell-extensions.Czarlie.gitlab.com gradia-integration@alexandervanhee.github.io \
    bazaar-integration@kolunmi.github.io paperwm@paperwm.github.com blur-my-shell@aunetx \
    dash-to-dock@micxgx.gmail.com gsconnect@andyholmes.github.io; do
    test ! -e "/usr/share/gnome-shell/extensions/${retired}"
    test ! -e "/usr/share/licenses/${retired}"
    grep -q "'${retired}'" /usr/share/glib-2.0/schemas/zz0-amethystora-modifications.gschema.override && false
done
# A module's keys are only the ones this plan names, each schema system-wide for gsettings and ame. The workspace
# row's keys swap a workspace with its neighbour, which collide with nothing GNOME binds, and name a space after its
# main app (auto-names, the motion plan's), and saved sets in its menu (sets, phase 7's).
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys org.gnome.shell.extensions.amethystora-ui.workspaces | sort | paste -sd ' ')" == \
    "auto-names move-workspace-left move-workspace-right resume sets" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.workspaces move-workspace-left)" == \
    "['<Control><Alt><Super>Left']" ]]
# The tiling: a switch (the classic layout is it off) and the settings its preferences window changes, under the
# names scrollable tiling always had them, with the image's values; and its keys, which are the ones ame setup and
# the manual's keybindings page name, 54 bound and the rest left for whoever wants them. The switch is on as the
# image ships.
TILING=org.gnome.shell.extensions.amethystora-ui.tiling
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${TILING}" enabled)" == true ]]
for key in window-gap horizontal-margin vertical-margin vertical-margin-bottom; do
    [[ "$(GSETTINGS_BACKEND=memory gsettings get "${TILING}" "${key}")" == 8 ]]
done
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${TILING}" cycle-width-steps)" == "[0.38195"*", 0.5, 0.61804"*"]" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${TILING}" winprops)" == "@as []" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${TILING}.keybindings" | wc -l)" == 100 ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings list-recursively "${TILING}.keybindings" | grep -vc '@as \[\]$')" == 54 ]]
test -s "${UI_EXTENSION}/prefs.js"
# The colours that follow the theme render whole for every shipped theme, through the theme switcher's own
# render_templates: a colour a palette lacks would leave a {{ key }} in the shell's stylesheet. The dock, the glass
# and the tiling have theirs (their stylesheets keep only shapes and stand-ins), and the glob below renders each
for module in dock glass tiling; do
    test -s "/usr/share/amethystora/themed/shell-${module}.css.tpl"
done
UI_RENDER="$(mktemp -d)"
for theme in /usr/share/amethystora/themes/*/; do
    mkdir -p "${UI_RENDER}/$(basename "${theme}")"
    cp "${theme}colors.toml" "${UI_RENDER}/$(basename "${theme}")/"
    (
        HOME="${UI_RENDER}" XDG_CONFIG_HOME="${UI_RENDER}/config"
        source /usr/lib/amethystora/theme/lib.sh
        render_templates "${UI_RENDER}/$(basename "${theme}")" "theme_name=$(basename "${theme}")"
    )
    for template in /usr/share/amethystora/themed/shell-*.css.tpl; do
        rendered="${UI_RENDER}/$(basename "${theme}")/$(basename "${template}" .tpl)"
        test -s "${rendered}"
        if grep -q '{{' "${rendered}"; then
            echo "${theme} leaves a value of $(basename "${rendered}") unrendered"
            exit 1
        fi
    done
done
rm -rf "${UI_RENDER}"
# The search box is on Super+Space alone, which GNOME's input-source switch must not take (zz0 moves it to
# Shift+Super+Space): on a base without that line the box never opened
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys org.gnome.shell.extensions.amethystora-ui.search | sort | paste -sd ' ')" == \
    "agent answers bookmarks clipboard glance manual meaning notes old-words open remembered reminders settings timers times windows" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.search open)" == "['<Super>space']" ]]
# Search that answers (plans/beyond/spec-search.md): each part on, the emoji picker on Super+. in the yellow tone
for key in answers times reminders timers settings manual windows clipboard agent glance old-words remembered; do
    [[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.search "${key}")" == true ]]
done
EMOJI=org.gnome.shell.extensions.amethystora-ui.emoji
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${EMOJI}" | sort | paste -sd ' ')" == "emoji-picker skin-tone" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${EMOJI}" emoji-picker)" == "['<Super>period']" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${EMOJI}" skin-tone)" == "'none'" ]]
for file in modules/search.js modules/search/calc.js modules/search/providers.js modules/search/glance.js \
    modules/emoji.js modules/emoji/picker.js modules/emoji/paste.js stylesheets/search.css stylesheets/emoji.css \
    icons/amethystora-search-agent-symbolic.svg; do
    test -s "${UI_EXTENSION}/${file}"
done
test -s /usr/share/amethystora/themed/shell-search.css.tpl
test -s /usr/share/amethystora/themed/shell-emoji.css.tpl
# The emoji picker before the glass, which reads its surfaces
[[ "$(grep -oE "^    \['(emoji|glass)'," "${UI_EXTENSION}/extension.js" | paste -sd ' ')" == "    ['emoji',     ['glass'," ]]
for recipe in "search WHAT" "emoji ACTION"; do
    grep -q "^${recipe}=\"\" " /usr/share/amethystora/just/desktop.just
done
# Nothing the search reads is ever run as code
if grep -rnE '\beval\(|new Function|(^|[^.a-zA-Z_])Function\(|\bimport\(' "${UI_EXTENSION}/modules/search" \
    "${UI_EXTENSION}/modules/emoji" "${UI_EXTENSION}/modules/search.js" "${UI_EXTENSION}/modules/emoji.js"; then
    echo "the search or the emoji picker runs text as code"
    exit 1
fi
# The sums and conversions, on the apps stage's Node (gjs runs the same file)
env LD_LIBRARY_PATH=/apps-stage/usr/lib64 NODE_ICU_DATA="$(echo /apps-stage/usr/share/node-*/icudata)" /apps-stage/usr/bin/node /ctx/build_files/shared/test-search-calc.mjs
# Old words and what is remembered (plans/ui-ideas/r7/spec-find.md): the table names no other maker's product, every app it
# names ships in the image or comes with every machine, the rules read what they should, and the index is asked only
# through bound values, with the words on standard input
env LD_LIBRARY_PATH=/apps-stage/usr/lib64 NODE_ICU_DATA="$(echo /apps-stage/usr/share/node-*/icudata)" /apps-stage/usr/bin/node /ctx/build_files/shared/test-search-words.mjs
for app in $(grep -oE "'[A-Za-z0-9._-]+\.desktop'" "${UI_EXTENSION}/modules/search/words.js" | tr -d "'"); do
    test -f "/usr/share/applications/${app}" ||
        grep -qxF "[Flatpak Preinstall ${app%.desktop}]" /usr/share/flatpak/preinstall.d/*.preinstall ||
        { echo "The old words open ${app}, which no machine has"; exit 1; }
done
test -x /usr/libexec/amethystora-file-find
bash /ctx/build_files/shared/test-file-find.sh
# GNOME's file index never reads the Private folder (zz1): its names and text would sit in the index's plain database
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.freedesktop.Tracker3.Miner.Files ignored-directories)" == *"'Private'"* ]]
# Opening a file or a place from the search does not wait on the compositor's thread, and logs no file name
for file in wordsProvider rememberProvider meaning; do
    grep -q "launch_default_for_uri_async" "${UI_EXTENSION}/modules/search/${file}.js"
    [[ -z "$(grep -n 'launch_default_for_uri(' "${UI_EXTENSION}/modules/search/${file}.js")" ]]
done
[[ -z "$(grep -n 'e\.message' "${UI_EXTENSION}/modules/search/rememberProvider.js" "${UI_EXTENSION}/modules/search/meaning.js")" ]]
for file in words wordsProvider remember rememberProvider; do
    [[ -z "$(grep -nE '_sync\(|file_get_contents' "${UI_EXTENSION}/modules/search/${file}.js")" ]]
done
grep -q "output(\[HELPER\], cancellable, JSON.stringify(question))" "${UI_EXTENSION}/modules/search/rememberProvider.js"
# Round 7's devices (plans/ui-ideas/r7/spec-devices.md): printing asks CUPS for states and pages and never for a document's
# name, the sound rows keep no more than three facts of a stream, get-through's mark holds up only the account's own timed
# runs on battery (never longer than a week, and a local backup never), the programs they run are in the image, and none of their files blocks the shell
env LD_LIBRARY_PATH=/apps-stage/usr/lib64 NODE_ICU_DATA="$(echo /apps-stage/usr/share/node-*/icudata)" /apps-stage/usr/bin/node /ctx/build_files/shared/test-devices.mjs
bash /ctx/build_files/shared/test-get-through.sh /usr/libexec/amethystora-metered
for program in ipptool cancel cupsenable lp pactl flatpak; do command -v "${program}" >/dev/null; done
grep -qE 'job-name|job-originating|printer-state-message|notify-text' /usr/share/amethystora/print/jobs-state.test && false
test -s /usr/share/amethystora/print/jobs-state.test
test ! -e /usr/share/amethystora/print/job-state.test
for file in pill/printing.js pill/printwords.js pill/through.js quick/sounding.js quick/tv.js; do
    test -s "${UI_EXTENSION}/modules/${file}"
    grep -nE '[a-z_]_sync\(|file_get_contents' "${UI_EXTENSION}/modules/${file}" && false
done
# The tile only ever opens the app, and only a person's yes in a terminal fetches it
grep -q 'flatpak' "${UI_EXTENSION}/modules/quick/tv.js" && false
env LD_LIBRARY_PATH=/apps-stage/usr/lib64 NODE_ICU_DATA="$(echo /apps-stage/usr/share/node-*/icudata)" /apps-stage/usr/bin/node /ctx/build_files/shared/test-your-year.mjs
# Text shortcuts (plans/beyond/spec-phase7.md, part B): off as the image ships, since they look at what is typed, the
# module after the emoji picker, the helper the only writer of the file, its rules and the shell's one list of cases (the
# installed helper, on the apps stage's Node), its two commands, and neither the switch nor a text in the setup file
TYPING=org.gnome.shell.extensions.amethystora-ui.typing
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${TYPING}" | sort | paste -sd ' ')" == "accents expand" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${TYPING}" expand)" == false ]]
# Hold a key for accents (plans/ui-ideas/spec-keys.md): offered and off, in the same module, never in the setup file
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${TYPING}" accents)" == false ]]
test -s "${UI_EXTENSION}/modules/typing/accents.js"
test -s "${UI_EXTENSION}/stylesheets/typing.css"
grep -q '^accents STATE="":' /usr/share/amethystora/just/desktop.just
for file in modules/typing.js modules/typing/match.js modules/typing/expand.js; do
    test -s "${UI_EXTENSION}/${file}"
done
[[ "$(grep -oE "^    \['(emoji|typing)'," "${UI_EXTENSION}/extension.js" | paste -sd ' ')" == "    ['emoji',     ['typing'," ]]
test -x /usr/libexec/amethystora-text-shortcuts
/usr/libexec/amethystora-text-shortcuts check /ctx/build_files/shared/fixtures/text-shortcuts-good.json
if /usr/libexec/amethystora-text-shortcuts check /ctx/build_files/shared/fixtures/text-shortcuts-bad.json 2>/dev/null; then
    echo "amethystora-text-shortcuts takes a text with an escape sequence in it"
    exit 1
fi
env LD_LIBRARY_PATH=/apps-stage/usr/lib64 NODE_ICU_DATA="$(echo /apps-stage/usr/share/node-*/icudata)" /apps-stage/usr/bin/node /ctx/build_files/shared/test-text-shortcuts.mjs \
    /usr/libexec/amethystora-text-shortcuts
grep -q '^text-shortcuts STATE="":' /usr/share/amethystora/just/desktop.just
grep -q '^text-shortcut \*ARGS:' /usr/share/amethystora/just/desktop.just
if grep -q "amethystora-ui.typing\|text-shortcuts" /usr/libexec/amethystora-setup; then
    echo "the setup file carries the text shortcuts, which are the account's own writing"
    exit 1
fi
# The module wraps GNOME's input method on its one object, never its class, writes nothing but the folder it reads,
# and never puts what was typed in the log
if grep -nE 'prototype\.notify_key_event|InputMethod\.prototype|eval\(|new Function|file_set_contents|replace_contents|console\.[a-z]+\([^)]*\$\{[^}]*(buffer|text|abbr|shortcut|ch)\b' \
    "${UI_EXTENSION}/modules/typing.js" "${UI_EXTENSION}/modules/typing"/*.js; then
    echo "the text shortcuts module touches GNOME's input method class, writes a file or logs what was typed"
    exit 1
fi
# Keys (plans/ui-ideas/spec-keys.md): paste as plain text on Super+Alt+V and what Super alone opens, the module after
# typing; every shortcut in one list (Control's Keyboard page), Super+Alt+V in it once and no shortcut of the image
# clashing with another as the image ships them; a bare letter never taken as a shortcut; the keyboard's recipes
KEYS_SCHEMA=org.gnome.shell.extensions.amethystora-ui.keys
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${KEYS_SCHEMA}" | sort | paste -sd ' ')" == "plain-paste super-alone" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${KEYS_SCHEMA}" plain-paste)" == "['<Super><Alt>v']" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${KEYS_SCHEMA}" super-alone)" == "'overview'" ]]
test -s "${UI_EXTENSION}/modules/keys.js"
[[ "$(grep -oE "^    \['(typing|keys)'," "${UI_EXTENSION}/extension.js" | paste -sd ' ')" == "    ['typing',     ['keys'," ]]
test -x /usr/libexec/amethystora-keybindings
KEYBINDINGS_JSON="$(GSETTINGS_BACKEND=memory /usr/libexec/amethystora-keybindings list --json)"
jq -e '[.bindings[] | select(.owner == "tiling")] | length == 100' <<<"${KEYBINDINGS_JSON}" >/dev/null
jq -e '[.bindings[] | select(.owner == "gnome")] | length > 100' <<<"${KEYBINDINGS_JSON}" >/dev/null
jq -e '[.bindings[] | select(.schema == "org.gnome.shell.extensions.amethystora-ui.keys" and .key == "plain-paste")] | length == 1' \
    <<<"${KEYBINDINGS_JSON}" >/dev/null
SUPER_ALT_V="$(jq -r '[.bindings[] | .value[] | ascii_downcase | select(test("^(<(super|alt)>){2}v$"))
    | select(contains("<super>") and contains("<alt>"))] | length' <<<"${KEYBINDINGS_JSON}")"
[[ "${SUPER_ALT_V}" == 1 ]] || { echo "Super+Alt+V is bound ${SUPER_ALT_V} times"; exit 1; }
[[ "$(jq '[.bindings[] | select(.clash != [])] | length' <<<"${KEYBINDINGS_JSON}")" == 0 ]]
if GSETTINGS_BACKEND=memory /usr/libexec/amethystora-keybindings set org.gnome.desktop.wm.keybindings close a 2>/dev/null; then
    echo "amethystora-keybindings takes a bare letter as a shortcut"
    exit 1
fi
for recipe in 'shortcut \*ARGS:' 'keyboard WHAT="" VALUE="":'; do
    grep -q "^${recipe}" /usr/share/amethystora/just/desktop.just
done
# The emoji data, made by 09-emoji.sh from Unicode's pinned files: every tab, every character named in English, the sizes
# the picker reads in one go when it opens, and the licence
python3 -I - <<'PYEOF'
import json, os
d = "/usr/share/amethystora/emoji"
core = json.load(open(f"{d}/emoji.json", encoding="utf-8"))
names = json.load(open(f"{d}/names-en.json", encoding="utf-8"))
tabs = ["smileys", "people", "nature", "food", "places", "activities", "objects", "symbols", "flags",
        "arrows", "maths", "money", "punctuation"]
assert all(core["tabs"].get(tab) for tab in tabs), "an emoji tab is empty"
emoji = sum(len(core["tabs"][tab]) for tab in tabs[:9])
assert emoji >= 1800, f"only {emoji} emoji"
missing = [c for tab in tabs for c in core["tabs"][tab] if c not in names]
assert not missing, f"no English name for {missing[:10]}"
assert os.path.getsize(f"{d}/emoji.json") <= 64 * 1024, "emoji.json is over 64 KB"
for language in ["en", "fr", "de", "es", "ar"]:
    json.load(open(f"{d}/names-{language}.json", encoding="utf-8"))
    assert os.path.getsize(f"{d}/names-{language}.json") <= 256 * 1024, f"names-{language}.json is over 256 KB"
PYEOF
test -s /usr/share/licenses/unicode/LICENSE
grep -q "emoji-test.txt" /usr/share/licenses/amethystora/NOTICE
grep -q "CLDR" /usr/share/licenses/amethystora/NOTICE
# The manual search answers without the agentic features (as nobody's account), from the manual alone
MANUAL_HITS="$(/usr/libexec/amethystora-agent-tools manual-search -- rolling back)"
jq -e 'length > 0 and all(.[]; .id | startswith("manual/")) and any(.[]; .id == "manual/updates#rolling-back")' \
    <<<"${MANUAL_HITS}" >/dev/null
[[ "$(/usr/libexec/amethystora-agent-tools manual-search -- )" == "[]" ]]
# The clipboard extension gives the search its read-only view, and says so in its first comment
CLIPBOARD_JS=/usr/share/gnome-shell/extensions/amethystora-clipboard@arsslensoft.github.io/extension.js
grep -q "^    history () {" "${CLIPBOARD_JS}"
grep -q "^    content (id) {" "${CLIPBOARD_JS}"
head -3 "${CLIPBOARD_JS}" | grep -q "search"
# Super+. is the picker's alone: Smile keeps Ctrl+Alt+Space, and its old second key is gone
if grep -rqi "<Super>period" /etc/dconf/db/distro.d/; then
    echo "a system keybinding still takes Super+., the emoji picker's"
    exit 1
fi
grep -A1 -h "custom-keybindings/custom5\]" /etc/dconf/db/distro.d/02-amethystora-keybindings | grep -qx "binding='<Control><Alt>space'"
test -s /usr/share/amethystora/user-setup.hooks.d/23-emoji-key.sh
# Ask the agent hands the question as one argument after --, and refuses one that reads as an option
grep -q 'exec xdg-terminal-exec "$0" ask -- "$@"' /usr/bin/amethystora-agent
grep -q 'a question cannot be empty or start with' /usr/bin/amethystora-agent
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.desktop.wm.keybindings switch-input-source)" == "['<Shift><Super>space']" ]]
# App search keeps Parental Controls' filter, as GNOME's own does
grep -q "shouldShowApp" "${UI_EXTENSION}/modules/fuzzy.js"
# The live pill (plans/beyond/spec-pill.md): its seven keys with their defaults, its files, its place among the modules
# (after the phone link, whose calls and links it hears, and before the dock and the glass), pw-dump for the camera's
# apps, its three recipes, and quiet holding banners in memory only: nothing of it writes GNOME's show-banners
PILL=org.gnome.shell.extensions.amethystora-ui.pill
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${PILL}" | sort | paste -sd ' ')" == \
    "digest digest-through digest-times digest-wait drop-targets get-through glow hello later later-times live meeting-mirror meetings moving music-bars music-see printing privacy-dots quiet quiet-night removable-writes saving-data second-zone stay-awake timer-feel two-at-once weight whats-new" ]]
for key in live privacy-dots glow quiet later whats-new meetings printing get-through; do
    [[ "$(GSETTINGS_BACKEND=memory gsettings get "${PILL}" "${key}")" == true ]]
done
for key in quiet-night digest; do
    [[ "$(GSETTINGS_BACKEND=memory gsettings get "${PILL}" "${key}")" == false ]]
done
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${PILL}" digest-times)" == "['12:00', '18:00']" ]]
# The digest's two lists (R5-33), empty as the image ships
for key in digest-through digest-wait; do
    [[ "$(GSETTINGS_BACKEND=memory gsettings get "${PILL}" "${key}")" == "@as []" ]]
done
grep -q "_forDigest(notification)" "${UI_EXTENSION}/modules/pill/hold.js"
test -s "${UI_EXTENSION}/modules/pill.js"
test -s "${UI_EXTENSION}/stylesheets/pill.css"
test -s /usr/share/amethystora/themed/shell-pill.css.tpl
[[ "$(grep -oE "^    \['(connect|pill|dock)'," "${UI_EXTENSION}/extension.js" | paste -sd ' ')" == \
    "    ['connect',     ['pill',     ['dock'," ]]
test -x /usr/bin/pw-dump
# Round 7's pill (plans/ui-ideas/r7/spec-pill.md): its keys, on and offered; its sums, under the apps stage's Node; no blocking call in its
# files; its sound meter neither writes nor takes the microphone; a code from the phone is never written or logged, and off as shipped
for key in two-at-once weight timer-feel moving stay-awake hello drop-targets music-see; do
    [[ "$(GSETTINGS_BACKEND=memory gsettings get "${PILL}" "${key}")" == true ]]
done
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.amethystora.Connect codes)" == false ]]
# The live bars are off until a GPU shows they cost under 1% of a core (R7-PILL-41); only the maintainer turns them on
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${PILL}" music-bars)" == false ]]
env LD_LIBRARY_PATH=/apps-stage/usr/lib64 NODE_ICU_DATA="$(echo /apps-stage/usr/share/node-*/icudata)" /apps-stage/usr/bin/node /ctx/build_files/shared/test-pill-logic.mjs
PILL_R7=(spring ring banners moving movinglist subscribe hello drop tint levels code)
for file in "${PILL_R7[@]}"; do
    test -s "${UI_EXTENSION}/modules/pill/${file}.js"
    if grep -nE "[a-z]_sync\(|file_get_contents|file_test|query_exists|communicate_utf8\(|\.spawn_sync|wait_check\(" "${UI_EXTENSION}/modules/pill/${file}.js"; then
        echo "the pill's ${file}.js blocks the shell"
        exit 1
    fi
done
test -x /usr/bin/pw-record
test -x /usr/bin/systemd-run
grep -q "application.id=org.gnome.VolumeControl" "${UI_EXTENSION}/modules/pill/levels.js"
# ...and it takes no stream but the one it is given: without dont-fallback a stream that is gone links to the microphone
grep -q "node.dont-fallback=true" "${UI_EXTENSION}/modules/pill/levels.js"
bash /ctx/build_files/shared/test-levels.sh "${UI_EXTENSION}/modules/pill/levels.js"
# The shell never reads a text: a code comes as the digits alone, from the phone link's own program
if grep -rnE "ListMessages|MessagesChanged|ListConversations|lastMessages|watchMessages" "${UI_EXTENSION}/modules"; then
    echo "the shell's modules read the phone's texts"
    exit 1
fi
if grep -nE "replace_contents|file_set_contents|create_readwrite|\.replace\(null|filesink|\.append_to" "${UI_EXTENSION}/modules/pill/levels.js" "${UI_EXTENSION}/modules/pill/code.js"; then
    echo "the pill's sound meter or its code writes something"
    exit 1
fi
if grep -nE "console\.[a-z]+\(.*(code|body|message\.)" "${UI_EXTENSION}/modules/pill/code.js"; then
    echo "the pill's code reaches a log"
    exit 1
fi
# ...and the clipboard's history and the phone link's sending both refuse the code the pill copies
for file in "${UI_EXTENSION}/modules/pill/code.js" "${UI_EXTENSION}/modules/connect/clipboard.js" \
    /usr/share/gnome-shell/extensions/amethystora-clipboard@arsslensoft.github.io/extension.js; do
    grep -q "Symbol.for('amethystora.clipboard.secret')" "${file}"
done
for recipe in pill quiet digest; do
    grep -q "^${recipe} \*ARGS:" /usr/share/amethystora/just/desktop.just
done
if grep -rn "show-banners" "${UI_EXTENSION}/modules/pill.js" "${UI_EXTENSION}/modules/pill"; then
    echo "the pill writes GNOME's Do Not Disturb"
    exit 1
fi
# Later and mute, and What's new (plans/beyond/spec-r4-notify.md): the times' default, the recipes, nothing that waits
# ever written (a notification's text stays in memory), and nothing of either shown as markup. What's new's page is
# in the Manual and its card's file is the build's, with no item when the build brought nothing
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${PILL}" later-times)" == "['18:00', '09:00']" ]]
for recipe in notifications whats-new; do
    grep -q "^${recipe} \*ARGS:" /usr/share/amethystora/just/desktop.just
done
if grep -nE "file_set_contents|replace_contents|create_readwrite|\.write\(" "${UI_EXTENSION}/modules/pill/later.js"; then
    echo "Later writes what waits somewhere"
    exit 1
fi
[[ -z "$(grep -nE 'use_markup = true|use_markup: true|set_markup\(' "${UI_EXTENSION}/modules/pill/later.js" \
    "${UI_EXTENSION}/modules/pill/whatsnew.js")" ]]
jq -e '(.version | type == "string") and (.releases | type == "array" and length <= 6)
    and all(.releases[]; (.items | length <= 12) and all(.items[]; (.title | type == "string")
        and (.area | IN("desktop", "security", "control", "apps", "updates", "system"))))' \
    /usr/share/amethystora/whats-new.json >/dev/null
[[ "$(jq -r .version /usr/share/amethystora/whats-new.json)" == "$(. /usr/lib/os-release && echo "${IMAGE_VERSION}")" ]]
jq -e '[.[].pages[]] | index("whats-new.md")' /usr/share/amethystora/manual/pages.json >/dev/null
grep -q "^# What's new$" /usr/share/amethystora/manual/whats-new.md
# Its notes never keep a subject the image's own checks refuse (a command, a path, an upstream name), so that no commit
# can stop a stream from building
bash /ctx/build_files/shared/test-whats-new.sh
# The page is made by each build: a translation of it would stand in for it, for ever saying what one build brought
if compgen -G "/usr/share/amethystora/manual/*/whats-new.md" >/dev/null; then
    echo "whats-new.md is made by the build and must not be translated"
    exit 1
fi
# Power you can see: four switches, all on (the devices' batteries, round 6's R6-26, and the flat battery, R7-54, among
# them), the module between today's and the dock, its recipe; it quits an app only by asking it (request_quit), and runs
# nothing. The flat battery's own files block nothing (its rule is checked by node, with the saved sets, below).
POWER=org.gnome.shell.extensions.amethystora-ui.power
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${POWER}" | sort | paste -sd ' ')" == "battery-details busy-app device-batteries flat-battery flat-hold sleep-report" ]]
# Not yet from the keyboard, bound only while the flat battery's card shows: a key nobody else binds
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${POWER}" flat-hold)" == "['<Super><Shift>n']" ]]
for key in battery-details busy-app device-batteries flat-battery sleep-report; do
    [[ "$(GSETTINGS_BACKEND=memory gsettings get "${POWER}" "${key}")" == true ]]
done
for file in modules/power.js modules/power/apps.js modules/power/battery.js modules/power/busy.js modules/power/devices.js     modules/power/flat.js modules/power/flatdata.js modules/pill/flat.js stylesheets/power.css; do
    test -s "${UI_EXTENSION}/${file}"
done
grep -qE "gi://|resource://" "${UI_EXTENSION}/modules/power/flatdata.js" && false
[[ -z "$(grep -nE '_sync\(|file_get_contents' "${UI_EXTENSION}/modules/power/flat.js" "${UI_EXTENSION}/modules/pill/flat.js")" ]]
test -s /usr/share/amethystora/themed/shell-power.css.tpl
[[ "$(grep -oE "^    \['(today|power|dock)'," "${UI_EXTENSION}/extension.js" | paste -sd ' ')" == \
    "    ['today',     ['power',     ['dock'," ]]
grep -q '^power \*ARGS:' /usr/share/amethystora/just/desktop.just
[[ -z "$(grep -rnE --exclude=slept.js 'Subprocess|Util\.spawn|spawn_command_line|GLib\.spawn|kill\(|SIGKILL|force_exit' \
    "${UI_EXTENSION}/modules/power.js" "${UI_EXTENSION}/modules/power")" ]]
grep -q "request_quit()" "${UI_EXTENSION}/modules/power/busy.js"
[[ -z "$(grep -rnE 'use_markup = true|use_markup: true|set_markup\(' "${UI_EXTENSION}/modules/power.js" \
    "${UI_EXTENSION}/modules/power")" ]]
# Quick settings as live tiles (plans/beyond/spec-quick.md): its two keys, empty, its files, its place among the
# modules (after the phone link, whose phones its tile shows, and before the glass, which draws behind its tiles), its
# recipe and Control's two entries. Nothing in it reads text as markup, the theme block runs `ame desktop theme` and
# nothing else, and the battery's limit is UPower's own method, never a file of the kernel's.
QUICK=org.gnome.shell.extensions.amethystora-ui.quick
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${QUICK}" | sort | paste -sd ' ')" == "send-to-tv sounding-apps tile-hidden tile-order" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${QUICK}" sounding-apps)" == true ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${QUICK}" send-to-tv)" == false ]]
for key in tile-order tile-hidden; do
    [[ "$(GSETTINGS_BACKEND=memory gsettings get "${QUICK}" "${key}")" == "@as []" ]]
done
for file in quick.js quick/tiles.js quick/battery.js quick/theme.js quick/volume.js quick/arrange.js; do
    test -s "${UI_EXTENSION}/modules/${file}"
done
test -s "${UI_EXTENSION}/stylesheets/quick.css"
test -s /usr/share/amethystora/themed/shell-quick.css.tpl
[[ "$(grep -oE "^    \['(connect|quick|glass)'," "${UI_EXTENSION}/extension.js" | paste -sd ' ')" == \
    "    ['connect',     ['quick',     ['glass'," ]]
grep -q '^quick-settings ACTION="" VALUE="":$' /usr/share/amethystora/just/desktop.just
[[ -z "$(grep -rnE 'use_markup = true|use_markup: true|set_markup\(' "${UI_EXTENSION}/modules/quick.js" \
    "${UI_EXTENSION}/modules/quick")" ]]
[[ "$(grep -rnE "Subprocess\.new|Util\.spawn|spawn_command_line|GLib\.spawn" "${UI_EXTENSION}/modules/quick.js" \
    "${UI_EXTENSION}/modules/quick" | grep -c .)" == 5 ]]
grep -q "Util.spawn(\[PHONE_APP\])" "${UI_EXTENSION}/modules/quick/tiles.js"
grep -q "Gio.Subprocess.new(\[AME, 'desktop', 'theme', slug\]," "${UI_EXTENSION}/modules/quick/theme.js"
[[ -z "$(grep -rn "charge_control\|/sys/" "${UI_EXTENSION}/modules/quick.js" "${UI_EXTENSION}/modules/quick")" ]]
grep -q "'EnableChargeThreshold'" "${UI_EXTENSION}/modules/quick/battery.js"
# A top bar on every screen and over fullscreen, and Today in the calendar menu (plans/beyond/spec-quick.md): their
# keys with their defaults (the weather off: it sends the place), their files, their place among the modules, their
# recipes, and nothing of theirs drawn as markup, since what they show is the person's text (task titles) or GNOME's
BARS=org.gnome.shell.extensions.amethystora-ui.bars
TODAY=org.gnome.shell.extensions.amethystora-ui.today
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${BARS}" | sort | paste -sd ' ')" == "autohide every-screen layout peek position size" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${BARS}" every-screen)" == false ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${BARS}" peek)" == true ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${TODAY}" | sort | paste -sd ' ')" == "getting-set tasks weather your-year your-year-seen" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${TODAY}" tasks)" == true ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${TODAY}" getting-set)" == true ]]
# Getting set runs two of the image's helpers, as the account, and nothing else; the lines it opens are the image's apps
test -s "${UI_EXTENSION}/modules/today/getting.js"
grep -q '^getting-set \*ARGS:' /usr/share/amethystora/just/desktop.just
[[ "$(grep -cE "Subprocess\.new|Util\.spawn" "${UI_EXTENSION}/modules/today/getting.js")" == 2 ]]
grep -q "^const RESTORE = '/usr/libexec/amethystora-restore';$" "${UI_EXTENSION}/modules/today/getting.js"
grep -q "^const REPORT = '/usr/libexec/amethystora-security-status';$" "${UI_EXTENSION}/modules/today/getting.js"
for program in amethystora-control amethystora-update amethystora-backups amethystora-security amethystora-phone; do
    test -x "/usr/bin/${program}"
done
if grep -niE "duress" "${UI_EXTENSION}/modules/today/getting.js"; then
    echo "Getting set names the duress feature"
    exit 1
fi
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${TODAY}" weather)" == false ]]
for file in modules/bars.js modules/today.js modules/today/tasks.js modules/today/weather.js stylesheets/bars.css \
    stylesheets/today.css; do
    test -s "${UI_EXTENSION}/${file}"
done
test -s /usr/share/amethystora/themed/shell-today.css.tpl
[[ "$(grep -oE "^    \['(pill|bars|today|dock)'," "${UI_EXTENSION}/extension.js" | paste -sd ' ')" == \
    "    ['pill',     ['bars',     ['today',     ['dock'," ]]
for recipe in top-bar today weather breaks; do
    grep -q "^${recipe} \*ARGS:" /usr/share/amethystora/just/desktop.just
done
[[ -z "$(grep -rnE 'use_markup = true|use_markup: true|set_markup\(' "${UI_EXTENSION}/modules/bars.js" \
    "${UI_EXTENSION}/modules/today.js" "${UI_EXTENSION}/modules/today")" ]]
# The bar your way, a calm desktop and the clock (plans/ui-ideas/spec-bar.md): the bar's arrangement, size and edge
# (offered: at the bottom and hiding until touched, both off), the animation speed, how windows open and close (GNOME's
# own until chosen), the second time zone (none), the light desktop (off), their recipes and the helper Control and the
# recipe share. The helper's items are the module's; nothing here runs a command or polls; the zone is text.
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${BARS}" layout)" == "@a{sas} {}" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${BARS}" position)" == "'top'" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${BARS}" autohide)" == false ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${BARS}" size)" == "'regular'" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.motion speed)" == "'normal'" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.motion open-close)" == "'gnome'" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.pill second-zone)" == "''" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys org.gnome.shell.extensions.amethystora-ui.comfort)" == light ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.comfort light)" == false ]]
for file in modules/bars/arrange.js modules/bars/edge.js modules/comfort/slow.js modules/motion/arrive.js modules/pill/zone.js; do
    test -s "${UI_EXTENSION}/${file}"
done
test -x /usr/libexec/amethystora-bar-layout
for recipe in clock; do
    grep -q "^${recipe} \*ARGS:" /usr/share/amethystora/just/desktop.just
done
for recipe in animation-speed text-size light-desktop window-motion; do
    grep -q "^${recipe} [A-Z]*=\"\":" /usr/share/amethystora/just/desktop.just
done
BAR_ITEMS="$(sed -n "s/^DEFAULT_[A-Z]*=(\(.*\))\$/\1/p" /usr/libexec/amethystora-bar-layout | tr ' ' '\n' | sort | paste -sd ' ')"
[[ "${BAR_ITEMS}" == "$(sed -n '/^export const ITEMS = {$/,/^};$/s/^    \([a-z0-9]*\): .*/\1/p' \
    "${UI_EXTENSION}/modules/bars/arrange.js" | sort | paste -sd ' ')" ]] || { echo "the bar's items differ"; exit 1; }
[[ -z "$(grep -E 'Util\.spawn|Subprocess|spawn_command_line|use_markup *= *true|setInterval|setTimeout' \
    "${UI_EXTENSION}/modules/bars/arrange.js" "${UI_EXTENSION}/modules/pill/zone.js" "${UI_EXTENSION}/modules/comfort/slow.js")" ]]
[[ -z "$(grep -E 'timeout_add' "${UI_EXTENSION}/modules/bars/arrange.js" "${UI_EXTENSION}/modules/pill/zone.js")" ]]
# The light desktop is offered once, from what GNOME itself reads of the renderer, never asked again for the same machine
grep -qF "is_rendering_hardware_accelerated()" "${UI_EXTENSION}/modules/comfort/slow.js"
grep -qF "inhibit_animations()" "${UI_EXTENSION}/modules/tweaks.js"
# Focus profiles (plans/beyond/spec-focus.md): the schema's 38 keys, nothing on and both triggers off for a new account,
# the three profiles as the board ships them, the module last among the modules (it lays itself over the others and stops
# first), its files and recipe. It never writes GNOME's Do Not Disturb, shows nothing as markup and starts nothing but
# Control; Sleep's Night Light is GNOME Settings' warmest; and the power daemon the image has keeps holds
FOCUS=org.gnome.shell.extensions.amethystora-ui.focus
FOCUS_LISTED="ends off-at profile trigger-games trigger-sleep"
for profile in games sleep work; do
    for field in accent apps banners dock grey lighter night-light power set sounds wallpaper; do
        FOCUS_LISTED+=" ${profile}-${field}"
    done
done
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${FOCUS}" | sort | paste -sd ' ')" == \
    "$(tr ' ' '\n' <<<"${FOCUS_LISTED}" | sort | paste -sd ' ')" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${FOCUS}" profile)" == "'off'" ]]
for key in trigger-games trigger-sleep work-sounds games-sounds sleep-sounds work-grey games-grey sleep-grey \
    work-lighter games-lighter sleep-lighter; do
    [[ "$(GSETTINGS_BACKEND=memory gsettings get "${FOCUS}" "${key}")" == false ]]
done
for pair in work-banners:apps games-banners:held sleep-banners:held games-dock:touch games-night-light:paused \
    sleep-night-light:warmest work-power:as-set games-power:performance sleep-power:power-saver sleep-accent:amber \
    sleep-wallpaper:night; do
    [[ "$(GSETTINGS_BACKEND=memory gsettings get "${FOCUS}" "${pair%%:*}")" == "'${pair#*:}'" ]]
done
[[ "$(grep -oE "^    \['[a-z]+'," "${UI_EXTENSION}/extension.js" | tail -n1)" == "    ['focus'," ]]
# Present mode and widgets on the desktop (R4-02, R4-19): offered and off, their keys only these; present starts after
# the pill, whose hold it asks, and widgets on the desktop after the glass, which draws their cards
PRESENT_SCHEMA=org.gnome.shell.extensions.amethystora-ui.present
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${PRESENT_SCHEMA}" | sort | paste -sd ' ')" == \
    "ask-on-screen enabled pointer-mark scribble" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${PRESENT_SCHEMA}" enabled)" == false ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${PRESENT_SCHEMA}" pointer-mark)" == "'halo'" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${PRESENT_SCHEMA}" ask-on-screen)" == true ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${PRESENT_SCHEMA}" scribble)" == "['<Super><Alt>d']" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys org.gnome.shell.extensions.amethystora-ui.deskwidgets)" == enabled ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.deskwidgets enabled)" == false ]]
# In order, not side by side: other groups' modules may come between (focus is last, checked with Focus profiles)
[[ "$(grep -oE "^    \['(pill|glass|tiling|present|deskwidgets|focus)'," "${UI_EXTENSION}/extension.js" | paste -sd ' ')" == \
    "    ['pill',     ['glass',     ['tiling',     ['present',     ['deskwidgets',     ['focus'," ]]
# No other maker's names in what they show or are called
[[ -z "$(grep -rniE 'macos|mac os|tahoe|genie|mission control|spotlight|launchpad|quick look|stage manager|exposé' \
    "${UI_EXTENSION}/modules/present.js" "${UI_EXTENSION}/modules/present" "${UI_EXTENSION}/modules/deskwidgets.js" \
    "${UI_EXTENSION}/modules/screenshot" "${UI_EXTENSION}/schemas/${PRESENT_SCHEMA}.gschema.xml" \
    /usr/share/amethystora/themed/shell-present.css.tpl)" ]] || { echo "present names another maker's feature"; exit 1; }
for file in modules/present.js modules/present/mark.js modules/present/scribble.js modules/deskwidgets.js \
    modules/screenshot/pins.js modules/screenshot/picture.js stylesheets/present.css stylesheets/deskwidgets.css; do
    test -s "${UI_EXTENSION}/${file}"
done
# What the boards drew and rounds 5 and 6 finish (plans/ui-ideas/spec-finish.md, suite 68): the overview's named spaces,
# the Downloads stack's count, the gallery of widgets, the resting screen, living wallpapers and trimming a recording
for file in modules/workspaces/overview.js modules/rest.js modules/living.js modules/living/living.glsl \
    modules/screenshot/trim.js stylesheets/rest.css stylesheets/living.css; do
    test -s "${UI_EXTENSION}/${file}"
done
test -s /usr/share/amethystora/themed/shell-living.css.tpl
grep -q "ThumbnailsBox.prototype.addThumbnails" "${UI_EXTENSION}/modules/workspaces/overview.js"
grep -q "^const COUNTED = 'xdg:DOWNLOAD';$" "${UI_EXTENSION}/modules/dock/stacks.js"
# The resting screen and living wallpapers are offered and off; each module after the pill and comfort it asks
REST_SCHEMA=org.gnome.shell.extensions.amethystora-ui.rest
LIVING_SCHEMA=org.gnome.shell.extensions.amethystora-ui.living
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${REST_SCHEMA}" | sort | paste -sd ' ')" == "after enabled" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${REST_SCHEMA}" enabled)" == false ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${REST_SCHEMA}" after)" == 120 ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${LIVING_SCHEMA}")" == wallpaper ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${LIVING_SCHEMA}" wallpaper)" == "'off'" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings range "${LIVING_SCHEMA}" wallpaper | sed -n '2,$p' | tr -d "'" | paste -sd ' ')" == \
    "off aurora facets rain dusk" ]]
[[ "$(grep -oE "^    \['(pill|comfort|rest|living|focus)'," "${UI_EXTENSION}/extension.js" | tr -d " [',")" == \
    "$(printf '%s\n' comfort pill rest living focus)" ]]
grep -q "^resting-screen \*ARGS:" /usr/share/amethystora/just/desktop.just
grep -q '^living-wallpaper KIND="":' /usr/share/amethystora/just/desktop.just
# The living wallpapers' shader is in what a Cogl snippet takes on every GPU (GLSL ES 1.0 too): no version or precision
# line, no loops, and no pow() of what can be negative; they draw at most FPS frames a second, only while seen
[[ -z "$(sed 's|//.*||' "${UI_EXTENSION}/modules/living/living.glsl" | grep -nE '^#version|precision |\b(for|while) *\(|\bpow\(')" ]]
grep -q "^const FPS = 15;$" "${UI_EXTENSION}/modules/living.js"
# Trim a recording (R6-19): offered and off, ffmpeg-free's encoders the window asks for, and the window itself
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.screenshot recording-trim)" == false ]]
# Read whole first: grep -q stopping early would end ffmpeg with SIGPIPE, which pipefail counts as a failure
ENCODERS="$(ffmpeg -hide_banner -encoders 2>/dev/null)"
FILTERS="$(ffmpeg -hide_banner -filters 2>/dev/null)"
for encoder in gif libvpx-vp9; do
    grep -qE "^ V[.A-Z]{5} ${encoder} " <<<"${ENCODERS}" || { echo "ffmpeg has no ${encoder}"; exit 1; }
done
for filter in palettegen paletteuse tile; do
    grep -qE " ${filter} " <<<"${FILTERS}" || { echo "ffmpeg has no ${filter}"; exit 1; }
done
test -x /usr/bin/ffprobe
grep -q "^const TRIM = '/usr/bin/amethystora-trim';$" "${UI_EXTENSION}/modules/screenshot/trim.js"
grep -q "^NoDisplay=true" /usr/share/applications/amethystora-trim.desktop
# The gallery of widgets (R6-29): every widget it offers is an example that ships, and none is on for an account
for example in clock calendar now-playing system pomodoro do-not-disturb; do
    test -d "/usr/share/amethystora/widgets/examples/${example}"
    grep -q "^gallery ${example} " /usr/libexec/amethystora-control-list
done
grep -q '^    add) add_widget "${2:-}" "${3:-}" ;;$' /usr/bin/amethystora-widgets
[[ ! -e /etc/skel/.config/amethystora/widgets ]]
# Nothing they show is markup, nothing they run is a shell: the present module spawns nothing, the desktop's widgets
# run only through the widgets extension's runner
grep -rqE "use_markup = true|set_use_markup\(true\)" "${UI_EXTENSION}/modules/present.js" "${UI_EXTENSION}/modules/present" \
    "${UI_EXTENSION}/modules/deskwidgets.js" "${UI_EXTENSION}/modules/screenshot/pins.js" && false
grep -rqE "Util\.spawn|Subprocess|spawn_command_line" "${UI_EXTENSION}/modules/present.js" "${UI_EXTENSION}/modules/present" \
    "${UI_EXTENSION}/modules/deskwidgets.js" && false
for file in modules/focus.js modules/focus/icons.js modules/focus/mark.js modules/focus/night.js \
    modules/focus/picture.js modules/focus/power.js modules/focus/profiles.js modules/focus/state.js \
    modules/focus/tile.js modules/focus/triggers.js stylesheets/focus.css; do
    test -s "${UI_EXTENSION}/${file}"
done
test -s /usr/share/amethystora/themed/shell-focus.css.tpl
grep -q '^focus \*ARGS:' /usr/share/amethystora/just/desktop.just
if grep -rn "show-banners\|do-not-disturb" "${UI_EXTENSION}/modules/focus.js" "${UI_EXTENSION}/modules/focus"; then
    echo "focus profiles write GNOME's Do Not Disturb"
    exit 1
fi
[[ -z "$(grep -rnE 'use_markup = true|use_markup: true|set_markup\(' "${UI_EXTENSION}/modules/focus.js" \
    "${UI_EXTENSION}/modules/focus")" ]]
[[ "$(grep -rnE "Subprocess|Util\.spawn|spawn_command_line|GLib\.spawn" "${UI_EXTENSION}/modules/focus.js" \
    "${UI_EXTENSION}/modules/focus" | grep -c .)" == 2 ]]
[[ "$(grep -rcE "Util\.spawn\(\[CONTROL, 'desktop', 'focus'\]\)|Util\.spawn\(\['amethystora-control', 'desktop', 'focus'\]\)" \
    "${UI_EXTENSION}/modules/focus" | awk -F: '{ n += $2 } END { print n }')" == 2 ]]
grep -q "^export const WARMEST = 1700;" "${UI_EXTENSION}/modules/focus/night.js"
GSETTINGS_BACKEND=memory gsettings range org.gnome.settings-daemon.plugins.color night-light-temperature | grep -qx 'type u'
grep -q "def HoldProfile" "$(rpm -ql tuned | grep '/tuned/ppd/controller\.py$')"
# Live captions (plans/beyond/spec-r4-later.md, part B): off, one key; the recognizer is in the image (GStreamer's
# vosk element) and its helper downloads only the pinned model, never writes or sends the sound, and is the only
# program the module starts but pactl, which it asks whether the players are paused; nothing it hears is markup. The
# helper's own test runs without the network
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys org.gnome.shell.extensions.amethystora-ui.captions | sort | paste -sd ' ')" == \
    "dictate dictation enabled language" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.captions enabled)" == false ]]
rpm -q gst-vosk >/dev/null
gst-inspect-1.0 vosk >/dev/null
test -x /usr/libexec/amethystora-captions
grep -q "^MODEL_URL = 'https://alphacephei.com/vosk/models/{}.zip'$" /usr/libexec/amethystora-captions
grep -q "^    'en': ('vosk-model-small-en-us-0.15', '30f26242c4eb449f948e42cb302dd7a686cb29a3423a8367f99ff41780942498',$" \
    /usr/libexec/amethystora-captions
grep -q "'--proto-redir', protocols" /usr/libexec/amethystora-captions
# While words show, a full-screen window is composited, as GNOME's banners and OSDs have it, and given back after
[[ "$(grep -cE "global\.compositor\.(disable|enable)_unredirect\(\)" "${UI_EXTENSION}/modules/captions.js")" == 2 ]]
[[ -z "$(grep -nE "filesink|wavenc|tcpclientsink|udpsink|souphttp" /usr/libexec/amethystora-captions)" ]]
test -s "${UI_EXTENSION}/modules/captions.js"
test -s "${UI_EXTENSION}/stylesheets/captions.css"
[[ "$(grep -cE "Subprocess\.new|Util\.spawn|spawn_command_line|GLib\.spawn|Shell\.App" "${UI_EXTENSION}/modules/captions.js")" == 2 ]]
grep -q "Gio.Subprocess.new(\[HELPER, 'listen', this.language\]," "${UI_EXTENSION}/modules/captions.js"
grep -q "Gio.Subprocess.new(\['pactl', '--format=json', 'list', 'sink-inputs'\]," "${UI_EXTENSION}/modules/captions.js"
[[ -z "$(grep -nE 'use_markup = true|use_markup: true|set_markup\(' "${UI_EXTENSION}/modules/captions.js")" ]]
grep -q '^captions \*ARGS:' /usr/share/amethystora/just/desktop.just
bash /ctx/build_files/shared/test-captions.sh
# The agent on the desktop and force quit (plans/beyond/spec-r4-agent.md): their keys with their defaults, their files,
# their place among the modules (after the pill, before focus, the agent before force quit, whose dialog asks it)
AGENT_UI=org.gnome.shell.extensions.amethystora-ui.agent
QUIT_UI=org.gnome.shell.extensions.amethystora-ui.quit
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${AGENT_UI}" | sort | paste -sd ' ')" == "ask-selection pill" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${AGENT_UI}" ask-selection)" == "['<Super><Shift>a']" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${AGENT_UI}" pill)" == true ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${QUIT_UI}" | sort | paste -sd ' ')" == "force-quit frozen-dialog" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${QUIT_UI}" force-quit)" == "['<Super><Control><Shift>Escape']" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${QUIT_UI}" frozen-dialog)" == true ]]
for file in modules/agent.js modules/agent/status.js modules/agent/selection.js modules/agent/changes.js stylesheets/agent.css \
    modules/quit.js modules/quit/list.js modules/quit/usage.js modules/quit/frozen.js stylesheets/quit.css; do
    test -s "${UI_EXTENSION}/${file}"
done
test -s /usr/share/amethystora/themed/shell-agent.css.tpl
test -s /usr/share/amethystora/themed/shell-quit.css.tpl
# In order, not side by side: other groups' modules may come between (focus is last, checked with Focus profiles)
[[ "$(grep -oE "^    \['(bezel|agent|quit|focus)'," "${UI_EXTENSION}/extension.js" | paste -sd ' ')" == \
    "    ['bezel',     ['agent',     ['quit',     ['focus'," ]]
grep -q "'agent', 'waiting'" "${UI_EXTENSION}/modules/pill/activities.js"
[[ -z "$(grep -rnE 'use_markup = true|use_markup: true|set_markup\(' "${UI_EXTENSION}/modules/agent.js" "${UI_EXTENSION}/modules/agent" \
    "${UI_EXTENSION}/modules/quit.js" "${UI_EXTENSION}/modules/quit")" ]]
# The selection reaches the agent only as the one argument after --, from a read with a size, never the clipboard's
# whole text; the agent's undo and stop are its own commands, and force quit ends windows' processes only as GNOME does
grep -q "argv.push('--', this._text)" "${UI_EXTENSION}/modules/agent/selection.js"
grep -q "transfer_async(Meta.SelectionType.SELECTION_PRIMARY, type, READ," "${UI_EXTENSION}/modules/agent/selection.js"
grep -q "\[AGENT, 'undo', id\]" "${UI_EXTENSION}/modules/agent/changes.js"
grep -q "\[AGENT, 'stop'\]" "${UI_EXTENSION}/modules/agent/changes.js"
[[ -z "$(grep -rnE "\bkill\(|SIGKILL|'kill'" "${UI_EXTENSION}/modules/quit" | grep -v 'window\.kill()\|^[^:]*:[0-9]*: *//')" ]]
grep -q '^selection ACTION="":' /usr/share/amethystora/just/agent.just
grep -q '^pill ACTION="":' /usr/share/amethystora/just/agent.just
grep -q '^undo ID:' /usr/share/amethystora/just/agent.just
grep -q '^stop:' /usr/share/amethystora/just/agent.just
grep -q '^force-quit STATE="":' /usr/share/amethystora/just/desktop.just
grep -q '^frozen-dialog STATE="":' /usr/share/amethystora/just/desktop.just
# The quoted ask: the selected text after --, each line behind a bar, at most 4000 characters, never a question of its own
grep -q 'ask_quoted "${question}" "$2"' /usr/bin/amethystora-agent
grep -q 'sed '"'"'s/^/│ /'"'"'' /usr/bin/amethystora-agent
grep -q '${text:0:4000}' /usr/bin/amethystora-agent
# The image's own failure notices offer the agent only to an account that has it on, by a unit or a finding's id; a
# failed scan's notice is waited for in that account's own session, so the alert ends at once and a repeat is told
grep -q -- 'agent=(--agent-unit "${UNIT}")' /usr/libexec/amethystora-scan-alert
grep -q TimeoutStartSec /usr/lib/systemd/system/amethystora-scan-alert@.service && false
test -x /usr/libexec/amethystora-agent-notice
grep -q -- '--agent "agent=$(N_ '"'"'Ask the agent'"'"')"' /usr/libexec/amethystora-driver-status
grep -q '/usr/bin/amethystora-agent status' /usr/libexec/amethystora-notify-users
# What reaches the agent from others stays data: a frozen app's fields, a selection, a finding, with every kind of line break
bash /ctx/build_files/shared/test-agent-text.sh
# Round 4's privacy module (plans/beyond/spec-r4-privacy.md): its five keys, the share guard and the shield on, network
# by app off; the module after connect, pill, quick and meters, which it reads, and before focus; its files and recipes; no
# markup; it starts nothing but Security and asks the security settings' one reader. Walk-away lock is a security
# switch, off, in Security's logins, never in the setup file; the agent may turn it on and never off
PRIVACY=org.gnome.shell.extensions.amethystora-ui.privacy
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${PRIVACY}" | sort | paste -sd ' ')" == \
    "network-apps share-guard share-guard-apps share-guard-private shield zone-notice" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${PRIVACY}" share-guard)" == "'blur'" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${PRIVACY}" shield)" == "'tile'" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${PRIVACY}" share-guard-private)" == true ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${PRIVACY}" network-apps)" == false ]]
for app in amethystora-notes amethystora-security amethystora-backups amethystora-help amethystora-vault; do
    GSETTINGS_BACKEND=memory gsettings get "${PRIVACY}" share-guard-apps | grep -qF "'${app}.desktop'"
    test -f "/usr/share/applications/${app}.desktop"
done
# Its place by relative order only: after the modules it reads, before focus, which is last
MODULE_ORDER="$(grep -oE "^    \['[a-z]+'," "${UI_EXTENSION}/extension.js" | tr -d " [',")"
module_at() { grep -nx "$1" <<<"${MODULE_ORDER}" | cut -d: -f1; }
for before in connect pill quick meters; do
    (($(module_at "${before}") < $(module_at privacy)))
done
(($(module_at privacy) < $(module_at focus)))
[[ "$(tail -n1 <<<"${MODULE_ORDER}")" == focus ]]
# The shield never says anything of the duress feature: report.json's duress checks are left out where it reads them
grep -q "^const HIDDEN = /^duress-/;" "${UI_EXTENSION}/modules/privacy/readings.js"
[[ -z "$(grep -rniE 'duress' "${UI_EXTENSION}/modules/privacy" | grep -v 'HIDDEN\|duress feature\|duress rule')" ]]
for file in modules/privacy.js modules/privacy/away.js modules/privacy/connections.js modules/privacy/guard.js \
    modules/privacy/readings.js modules/privacy/shield.js modules/privacy/streams.js stylesheets/privacy.css; do
    test -s "${UI_EXTENSION}/${file}"
done
test -s /usr/share/amethystora/themed/shell-privacy.css.tpl
grep -q '^share-guard \*ARGS:' /usr/share/amethystora/just/desktop.just
grep -q '^shield STATE="":' /usr/share/amethystora/just/desktop.just
grep -q '^network-apps STATE="":' /usr/share/amethystora/just/desktop.just
grep -q '^walk-away ACTION="":' /usr/share/amethystora/just/security.just
[[ -z "$(grep -rnE 'use_markup = true|use_markup: true|set_markup\(' "${UI_EXTENSION}/modules/privacy.js" \
    "${UI_EXTENSION}/modules/privacy")" ]]
[[ "$(grep -rhoE "Util\.spawn\([^)]*\)|Subprocess\.new\(\[[A-Z_]+" "${UI_EXTENSION}/modules/privacy" | LC_ALL=C sort -u | paste -sd ' ')" == \
    "Subprocess.new([CONFIG Util.spawn([SECURITY, 'apps']) Util.spawn([SECURITY, 'settings/network']) Util.spawn(page ? [SECURITY, page] : [SECURITY])" ]]
grep -q "^const SECURITY = '/usr/bin/amethystora-security';" "${UI_EXTENSION}/modules/privacy/shield.js"
grep -q "^const CONFIG = '/usr/libexec/amethystora-security-config';" "${UI_EXTENSION}/modules/privacy/away.js"
[[ "$(/usr/libexec/amethystora-security-config WALK_AWAY)" == off ]]
[[ "$(/usr/libexec/amethystora-security-config --list --json | jq -r '.settings[] | select(.key == "WALK_AWAY") | .group')" == logins ]]
# GNOME's own rest reminders are what ame desktop breaks writes: the key and its two kinds of break are there
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.desktop.break-reminders selected-breaks)" == "@as []" ]]
GSETTINGS_BACKEND=memory gsettings list-keys org.gnome.desktop.break-reminders.eyesight | grep -qx interval-seconds
GSETTINGS_BACKEND=memory gsettings list-keys org.gnome.desktop.break-reminders.movement | grep -qx interval-seconds
# Notes' quick note: its launcher's action, and the help that names it
grep -qx "Exec=amethystora-notes capture" /usr/share/applications/amethystora-notes.desktop
grep -q "amethystora-notes capture" /usr/bin/amethystora-notes
test -s "${UI_EXTENSION}/modules/fuzzy/matcher.js"
# Copying text out of a screenshot is Gradia's, reached through the screenshot module's Extract Text button
# and Ctrl+E in GNOME's screenshot UI (manual: desktop.md#copying-text-from-the-screen): a change that drops
# the button, its key or the hand-off to Gradia must fail here instead of leaving the manual wrong
grep -q "_('Extract Text')" "${UI_EXTENSION}/modules/screenshot/session.js"
grep -q "Clutter.KEY_e" "${UI_EXTENSION}/modules/screenshot.js"
grep -q -- "'flatpak', 'run', GRADIA, \`--ocr-file=" "${UI_EXTENSION}/modules/screenshot.js"
grep -q "^export const GRADIA = 'be.alexandervanhee.gradia';$" "${UI_EXTENSION}/modules/screenshot/capture.js"
test -s "${UI_EXTENSION}/icons/screenshot/extract-symbolic.svg"
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys org.gnome.shell.extensions.amethystora-ui.screenshot | sort | paste -sd ' ')" == \
    "capture-pin capture-thumbnail hide-private recording-trim tool-settings" ]]
# Pin a capture (R4-17): the thumbnail and pinning on as the image ships, the bar's Pin button and Ctrl+P, the pin's two
# icons drawn for the extension, at most six pinned, and a pinned picture never saved by itself
for key in capture-thumbnail capture-pin; do
    [[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.screenshot "${key}")" == true ]]
done
grep -q "_('Pin on top')" "${UI_EXTENSION}/modules/screenshot/session.js"
grep -q "Clutter.KEY_p" "${UI_EXTENSION}/modules/screenshot.js"
grep -q "^export const MOST = 6;$" "${UI_EXTENSION}/modules/screenshot/pins.js"
test -s "${UI_EXTENSION}/icons/screenshot/pin-symbolic.svg"
test -s "${UI_EXTENSION}/icons/screenshot/through-symbolic.svg"
grep -qE "saveToFolder|replace_contents" "${UI_EXTENSION}/modules/screenshot/pins.js" && false
# If it's lost (R7-71): off until written, no key in the image, GDM's banner the one on the login screen
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.found line)" == "''" ]]
grep -q "system-db:gdm" /usr/share/dconf/profile/gdm /etc/dconf/profile/gdm 2>/dev/null
test ! -e /etc/dconf/db/gdm.d/70-amethystora-if-found
bash /ctx/build_files/shared/test-if-found.sh
# Your year (R7-30): never kept. How do I look and sound? (R7-44): frames and sound go nowhere
bash /ctx/build_files/shared/test-your-year.sh
bash /ctx/build_files/shared/test-mirror.sh
# What the people group (R7-64, 65, 67, 71, 30) added to the shell never waits for the disk or a process. The pattern wants
# a letter before "_sync(", since several of these files have a method of their own with that name
for file in found.js today/year.js today/yearwindow.js windows/handover.js windows/simple.js screenshot/cover.js; do
    if grep -nE '[a-z0-9]_sync\(|file_get_contents|mkdir_with_parents|\.communicate\(|\.communicate_utf8\(|\.wait_check\(' "${UI_EXTENSION}/modules/${file}"; then
        echo "${file} waits for the disk or a process"
        exit 1
    fi
done
# Hide what's private (R7-67): on as the image ships, its helper's patterns, solid boxes, a pipe and no file
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.screenshot hide-private)" == true ]]
test -s "${UI_EXTENSION}/icons/screenshot/cover-symbolic.svg"
test -x /usr/libexec/amethystora-cover-private
command -v tesseract >/dev/null
bash /ctx/build_files/shared/test-cover-private.sh
# Uninstalling from an app's menu: Flatpak's own command, as the account, and Bazaar for the details
grep -qF "['uninstall', user ? '--user' : (extra ? installation : '--system'), '-y', id]" "${UI_EXTENSION}/modules/bazaar.js"
grep -qF 'appstream:' "${UI_EXTENSION}/modules/bazaar.js"
# The tweaks: animations at 0.6 of their time through the shell's slow-down factor alone. GNOME's own switch for
# animations is the person's (Settings, Accessibility), and nothing here may turn it back on.
grep -qF "export const SPEEDS = {snappy: 0.4, normal: 0.6, relaxed: 0.9};" "${UI_EXTENSION}/modules/tweaks.js"
grep -rqE "set_boolean\(['\"]enable-animations|enable_animations = " "${UI_EXTENSION}" && false
# The trash empties itself of what was deleted more than 30 days ago, and ame desktop privacy changes that: a
# number of days or off, nothing else. Settings in memory, as there is no session here
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.desktop.privacy remove-old-trash-files)" == "true" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.desktop.privacy old-files-age)" == "uint32 30" ]]
GSETTINGS_BACKEND=memory ame desktop privacy trash 7 >/dev/null
GSETTINGS_BACKEND=memory ame desktop privacy trash off >/dev/null
for wrong in 0 3651 -1 7x; do
    GSETTINGS_BACKEND=memory ame desktop privacy trash "${wrong}" >/dev/null 2>&1 && false
done
GSETTINGS_BACKEND=memory ame desktop privacy clipboard 7 >/dev/null 2>&1 && false
test ! -e /usr/share/gnome-shell/extensions/tactile@lundal.io
# GNOME Shell's getSettings() reads an extension's own schemas folder before the system's, where an
# override does not reach by itself: every key zz0 and zz1 set for an extension reads the same through
# its folder as system-wide (build-gnome-extensions.sh compiles them in)
for schemas in /usr/share/gnome-shell/extensions/*/schemas; do
    for schema in $(cat "${schemas}"/*.gschema.xml | tr -s '[:space:]' ' ' | grep -oE '<schema [^>]*id="[^"]+"' |
        sed -E 's/.*id="([^"]+)".*/\1/'); do
        for key in $(awk -v section="[${schema}]" '/^\[/ { on = ($0 == section); next } on && /^[a-z]/ { sub(/ *=.*/, ""); print }' \
            /usr/share/glib-2.0/schemas/zz0-amethystora-modifications.gschema.override \
            /usr/share/glib-2.0/schemas/zz1-amethystora-modifications.gschema.override); do
            [[ "$(GSETTINGS_BACKEND=memory gsettings --schemadir "${schemas}" get "${schema}" "${key}")" == \
                "$(GSETTINGS_BACKEND=memory gsettings get "${schema}" "${key}")" ]]
        done
    done
done
# None of the clipboard history's shortcuts take the Ctrl+F keys from applications
grep -qi "<Control>F" <<<"$(GSETTINGS_BACKEND=memory gsettings list-recursively org.gnome.shell.extensions.amethystora-clipboard)" && false
# The tiling takes a colliding GNOME binding for itself while it is on. It cannot see custom keybindings
# or other extensions' shortcuts, and the GNOME ones listed here are kept on purpose, so no tiling
# binding may share a key with any of them. Modifiers are sorted and lower-cased before comparing.
keycombos() {
    grep -oE "'[^']+'" | tr -d "'" | tr '[:upper:]' '[:lower:]' | sed 's/<ctrl>/<control>/g; s/<primary>/<control>/g' |
        while read -r combo; do
            echo "$(grep -o '<[a-z]*>' <<<"${combo}" | LC_ALL=C sort | tr -d '\n')${combo##*>}"
        done | LC_ALL=C sort -u
}
TILING_COMBOS="$(GSETTINGS_BACKEND=memory gsettings list-recursively org.gnome.shell.extensions.amethystora-ui.tiling.keybindings | keycombos)"
grep -qx "<super>left" <<<"${TILING_COMBOS}"
KEPT_COMBOS="$(
    {
        grep -h -d skip -e "^binding=" -e "^shortcut-search=" /etc/dconf/db/distro.d/*
        GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-clipboard toggle-menu
        for key in close begin-resize switch-applications switch-windows switch-group switch-panels; do
            GSETTINGS_BACKEND=memory gsettings get org.gnome.desktop.wm.keybindings "${key}"
        done
        GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.keybindings toggle-message-tray
        GSETTINGS_BACKEND=memory gsettings get org.gnome.mutter.wayland.keybindings restore-shortcuts
        GSETTINGS_BACKEND=memory gsettings get org.gnome.mutter.keybindings cancel-input-capture
        GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.windows snap-chooser
        # The drop-down terminal's, keep on top's and reopen's (Super+Alt+Return, P and T)
        for key in drop-down-key keep-on-top-key reopen-key; do
            GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.windows "${key}"
        done
        # The reading ruler's, reading aloud's and the pen's tools' (Super+Ctrl+R, S and P)
        GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.access ruler-key
        GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.access read-aloud-key
        GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.touch pen-ring-key
    } | keycombos
)"
grep -qx "<super>return" <<<"${KEPT_COMBOS}"
# The quick note's Super+Alt+N (custom30) is the image's: no tiling key takes it
grep -qx "<alt><super>n" <<<"${KEPT_COMBOS}"
[[ -z "$(LC_ALL=C comm -12 <(echo "${TILING_COMBOS}") <(echo "${KEPT_COMBOS}"))" ]]
# Super+Alt+D is Scribble's (the present module's own key, R4-02): neither the tiling nor a kept binding has it
if grep -qx "<alt><super>d" <<<"${TILING_COMBOS}"$'\n'"${KEPT_COMBOS}"; then
    echo "Super+Alt+D is bound by the tiling or a kept binding, and is Scribble's"
    exit 1
fi
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.present scribble | keycombos)" == \
    "<alt><super>d" ]]
# Super+. is the emoji picker's: neither the tiling nor a kept binding has it
if grep -qx "<super>period" <<<"${TILING_COMBOS}"$'\n'"${KEPT_COMBOS}"; then
    echo "Super+. is bound by the tiling or a kept binding, and is the emoji picker's"
    exit 1
fi
# Super+Shift+A is asking the agent about a selection and Super+Ctrl+Shift+Escape force quit's (Super+Ctrl+Escape is the
# tiling's): neither the tiling, a kept binding nor any of GNOME's own keybinding schemas has either
GNOME_COMBOS="$(for schema in org.gnome.desktop.wm.keybindings org.gnome.shell.keybindings org.gnome.mutter.keybindings \
    org.gnome.mutter.wayland.keybindings org.gnome.settings-daemon.plugins.media-keys; do
    GSETTINGS_BACKEND=memory gsettings list-recursively "${schema}"
done | keycombos)"
# Super+Alt+Return, P and T are the windows module's (the drop-down terminal, keep on top, reopen): neither the tiling
# nor any of GNOME's own keybinding schemas has them
for combo in "<alt><super>return" "<alt><super>p" "<alt><super>t"; do
    grep -qx "${combo}" <<<"${KEPT_COMBOS}" || { echo "${combo} is not the windows module's"; exit 1; }
    if grep -qx "${combo}" <<<"${TILING_COMBOS}"$'\n'"${GNOME_COMBOS}"; then
        echo "${combo} is bound by the tiling or GNOME, and is the windows module's"
        exit 1
    fi
done
for combo in "<shift><super>a" "<control><shift><super>escape"; do
    if grep -qx "${combo}" <<<"${TILING_COMBOS}"$'\n'"${KEPT_COMBOS}"$'\n'"${GNOME_COMBOS}"; then
        echo "${combo} is bound elsewhere, and is the agent's or force quit's"
        exit 1
    fi
done

# Tools at hand (plans/ui-ideas/spec-tools.md): the tools module's three keys and their defaults, between privacy and
# focus; its two keys, Super+Alt+C and Super+Alt+M, bound nowhere else; the QR codes' qrencode, handed its text on stdin
# only; the helpers of What is using this? and Convert, which read argv only and run no shell; no markup and no shell in
# the module or the clipboard's text actions; Convert offered and off; What is using this? in Files' Scripts menu
TOOLS=org.gnome.shell.extensions.amethystora-ui.tools
[[ "$(GSETTINGS_BACKEND=memory gsettings list-recursively "${TOOLS}" | cut -d' ' -f2- | LC_ALL=C sort | paste -sd ';')" == \
    "colour-picker ['<Super><Alt>c'];convert-in-files false;earlier-versions true;mute-microphone ['<Super><Alt>m'];network-reach true;offer-room true;open-anything true;uninstall-takes-data true" ]] ||
    { echo "the tools module's keys or defaults are not the spec's"; exit 1; }
TOOLS_ORDER="$(grep -oE "^    \['[a-z]+'," "${UI_EXTENSION}/extension.js" | tr -d " [',")"
tools_at() { grep -nx "$1" <<<"${TOOLS_ORDER}" | cut -d: -f1; }
(($(tools_at privacy) < $(tools_at tools) && $(tools_at tools) < $(tools_at focus)))
for file in modules/tools.js modules/tools/microphone.js modules/tools/picker.js modules/tools/qr.js modules/tools/run.js \
    modules/tools/users.js modules/tools/wifi.js stylesheets/tools.css; do
    test -s "${UI_EXTENSION}/${file}"
done
test -s /usr/share/amethystora/themed/shell-tools.css.tpl
test -s /usr/share/gnome-shell/extensions/amethystora-clipboard@arsslensoft.github.io/textActions.js
[[ "$(GSETTINGS_BACKEND=memory gsettings list-recursively "${TOOLS}" | grep -E ' (colour-picker|mute-microphone) ' | keycombos |
    paste -sd ' ')" == "<alt><super>c <alt><super>m" ]]
for combo in "<alt><super>c" "<alt><super>m"; do
    if grep -qx "${combo}" <<<"${TILING_COMBOS}"$'\n'"${KEPT_COMBOS}"$'\n'"${GNOME_COMBOS}"; then
        echo "${combo} is bound elsewhere, and is the tools module's"
        exit 1
    fi
done
command -v qrencode >/dev/null
[[ "$(printf 'https://amethystora.org' | qrencode -t ASCII -m 0 -l M -8 | head -c 14)" == "##############" ]]
grep -qF "run([QRENCODE, '-t', 'ASCII', '-m', '0', '-l', 'M', '-8']," "${UI_EXTENSION}/modules/tools/qr.js"
[[ -z "$(grep -rnE 'use_markup = true|use_markup: true|set_markup\(|spawn_command_line|GLib\.spawn_async|Util\.spawn' \
    "${UI_EXTENSION}/modules/tools.js" "${UI_EXTENSION}/modules/tools" \
    /usr/share/gnome-shell/extensions/amethystora-clipboard@arsslensoft.github.io/textActions.js)" ]]
for helper in amethystora-what-uses amethystora-convert; do
    test -x "/usr/libexec/${helper}"
    python3 -I -c 'import ast, sys; ast.parse(open(sys.argv[1], encoding="utf-8").read())' "/usr/libexec/${helper}"
    grep -vh '^ *#' "/usr/libexec/${helper}" | grep -qE 'NAUTILUS_SCRIPT_|shell=True|os\.system|bash -c|sh -c' && false
    [[ "$("/usr/libexec/${helper}" >/dev/null 2>&1; echo $?)" == 64 ]]
done
[[ "$(/usr/libexec/amethystora-what-uses list --json -- /nonexistent | jq -c .)" == '{"targets":[],"holders":[],"truncated":false}' ]]
grep -q '^files-convert STATE="":' /usr/share/amethystora/just/desktop.just
grep -q 'ame desktop files-convert' /usr/libexec/amethystora-control-list
grep -q '"What is using this?")" /usr/libexec/amethystora-what-uses show$' /usr/share/amethystora/user-setup.hooks.d/24-files-menu.sh

# Round 7, the files group (plans/ui-ideas/r7/spec-files.md). Every switch has its command and its Control entry
for recipe in earlier-versions open-anything uninstall-data low-space; do
    grep -q "^${recipe} STATE=\"\":" /usr/share/amethystora/just/desktop.just
    grep -q "ame desktop ${recipe}" /usr/libexec/amethystora-control-list
done
grep -q '"Earlier versions")" /usr/libexec/amethystora-back-in-time --versions$' /usr/share/amethystora/user-setup.hooks.d/24-files-menu.sh
for helper in amethystora-open-anything amethystora-free-space amethystora-uninstall amethystora-flatpak-ask; do
    test -x "/usr/libexec/${helper}"
    python3 -I -c 'import ast, sys; ast.parse(open(sys.argv[1], encoding="utf-8").read())' "/usr/libexec/${helper}"
    grep -vh '^ *#' "/usr/libexec/${helper}" | grep -qE 'shell=True|os\.system|bash -c|sh -c' && false
    # Flatpak's -y and --noninteractive both call off polkit's question, which a person who pressed a button must be able to answer
    sed -n '/^"""$/,$p' "/usr/libexec/${helper}" | grep -vE '^ *#' | grep -qE '"--noninteractive"|"-y"' && false
    [[ "$("/usr/libexec/${helper}" >/dev/null 2>&1; echo $?)" == 64 ]]
done
# New shell code blocks nothing, and writes nothing that is not markup-free
[[ -z "$(grep -nE '_sync\(|file_get_contents|use_markup = true' "${UI_EXTENSION}/modules/tools/opener.js" "${UI_EXTENSION}/modules/tools/room.js")" ]]
# Never out of room never takes a container's volume, a named image or the trash's last 30 days
grep -vh '^ *#' /usr/libexec/amethystora-free-space | grep -qE 'volume prune|prune.*-a|-af\b' && false
# ...and a check connects to nothing: Homebrew's dry run would fetch its list of formulae
grep -q '"--dry-run"' /usr/libexec/amethystora-free-space && false
# Uninstall goes through the trash: what it may touch is the app's own folder, and it never deletes it
sed -n '/^"""$/,$p' /usr/libexec/amethystora-uninstall | grep -vE '^ *#' | grep -qE 'rmtree|--delete-data' && false
# Anything opens: its launcher stays out of the app grid, claims what the image does not open, and the index GIO reads has it. An
# image's own launcher never loses a kind to it, text and folders are never claimed, and a claim outranks only nothing: Flatpak's
# launchers come first, and in its own folder, a package added later included, its name sorts last (test-files-group.sh)
OPENER=/usr/share/applications/zz-amethystora-open-anything.desktop
grep -qx 'NoDisplay=true' "${OPENER}"
grep -qx 'Exec=/usr/libexec/amethystora-open-anything %f' "${OPENER}"
grep -q '^application/octet-stream=.*zz-amethystora-open-anything.desktop' /usr/share/applications/mimeinfo.cache
[[ "$(sed -n 's/^MimeType=//p' "${OPENER}" | tr ';' '\n' | grep -c .)" -ge 100 ]]
[[ -z "$(sed -n 's/^MimeType=//p' "${OPENER}" | tr ';' '\n' | grep -E '^(text|inode|x-content|x-scheme-handler)/')" ]]
[[ -z "$(for launcher in /usr/share/applications/*.desktop; do
    [[ "${launcher}" == "${OPENER}" ]] || sed -n 's/^MimeType=//p' "${launcher}"
done | tr ';' '\n' | grep . | sort -u | comm -12 - <(sed -n 's/^MimeType=//p' "${OPENER}" | tr ';' '\n' | grep . | sort -u))" ]]
# Look's copy from a backup goes after a day with nothing new running: the account's own tmpfiles timer, on as Fedora ships it
grep -qx 'e %C/amethystora-restore/look - - - 1d' /usr/share/user-tmpfiles.d/amethystora-restore.conf
[[ "$(systemctl --global is-enabled systemd-tmpfiles-clean.timer)" == enabled ]]
# Space to look: GNOME's previewer stays on the default list, since nothing else answers Files' Space
grep -q '^\[Flatpak Preinstall org.gnome.NautilusPreviewer\]$' /usr/share/flatpak/preinstall.d/amethystora.preinstall
bash /ctx/build_files/shared/test-files-group.sh

# The glass and the dock are Amethystora UI's own modules, in place of Blur my Shell and Dash to Dock. The glass
# is a stylesheet and a shader (modules/glass/lens.glsl) with no setting of its own, and no Shell.GLSLEffect,
# which GNOME 51 no longer has. Neither extension nor the glass's old dconf defaults are in the image.
test -s "${UI_EXTENSION}/modules/glass/lens.glsl"
test -s "${UI_EXTENSION}/stylesheets/glass.css"
test -s "${UI_EXTENSION}/stylesheets/dock.css"
# No extension of the image, ours or another's, may draw with Shell.GLSLEffect: it fails to load on GNOME 51
[[ -z "$(grep -rn --include='*.js' GLSLEffect /usr/share/gnome-shell/extensions | grep -v ':[0-9]*: *//')" ]]
# Every module of our extensions parses: one syntax error keeps the whole extension from loading, and nothing else
# here would notice
find /usr/share/gnome-shell/extensions/amethystora-*@arsslensoft.github.io -name '*.js' -print0 |
    while IFS= read -r -d '' js; do
        env LD_LIBRARY_PATH=/apps-stage/usr/lib64 /apps-stage/usr/bin/node --check --input-type=module <"${js}" ||
            { echo "${js} does not parse" >&2; exit 1; }
    done
# Every relative import in our extensions names a file the image has. One missing file keeps the whole extension from
# loading, and parsing each file alone does not notice (dock/flatpak.js, once left out by a .gitignore rule)
find /usr/share/gnome-shell/extensions/amethystora-*@arsslensoft.github.io -name '*.js' -print0 |
    while IFS= read -r -d '' js; do
        { grep -oE "(from|import\() *['\"]\.\.?/[^'\"]+['\"]" "${js}" || true; } |
            sed -E "s/.*['\"](\.\.?\/[^'\"]+)['\"]/\1/" |
            while IFS= read -r rel; do
                test -f "$(dirname "${js}")/${rel}" || { echo "${js} imports ${rel}, which is missing" >&2; exit 1; }
            done || exit 1
    done
# Round 7's help (plans/ui-ideas/r7/spec-help.md): Show me's key and its guides, each step held to what exists (R7-61); what
# the Help window's helpers do, against stand-ins (R7-36, R7-32); nothing of theirs blocks the shell or keeps a record
GUIDE_SCHEMA=org.gnome.shell.extensions.amethystora-ui.guide
[[ "$(GSETTINGS_BACKEND=memory gsettings list-recursively "${GUIDE_SCHEMA}" | cut -d' ' -f2-)" == "show-me true" ]]
test -s /usr/share/amethystora/themed/shell-guide.css.tpl
[[ -z "$(grep -lE '_sync\(|file_get_contents' "${UI_EXTENSION}/modules/guide.js" "${UI_EXTENSION}"/modules/guide/*.js)" ]]
grep -q '^show-me STATE="":' /usr/share/amethystora/just/desktop.just
env LD_LIBRARY_PATH=/apps-stage/usr/lib64 NODE_ICU_DATA="$(echo /apps-stage/usr/share/node-*/icudata)" /apps-stage/usr/bin/node \
    /ctx/build_files/shared/check-guides.mjs "${UI_EXTENSION}" /usr/share/amethystora/manual /usr/lib /usr/share/applications
bash /ctx/build_files/shared/test-wrong.sh
bash /ctx/build_files/shared/test-changes.sh
# What changed with the real helper it hands an app's Undo to (R7-41's amethystora-uninstall, of the files group)
bash /ctx/build_files/shared/test-changes-uninstall.sh
# Every Gio.Settings the shell code of our extensions makes is kept and run_dispose()d on the main thread at disable,
# never left to the collector: GJS sweeping one while dconf's worker dispatches a change to it deadlocks the shell
env LD_LIBRARY_PATH=/apps-stage/usr/lib64 NODE_ICU_DATA="$(echo /apps-stage/usr/share/node-*/icudata)" /apps-stage/usr/bin/node /ctx/build_files/shared/test-settings-disposed.mjs \
    /usr/share/gnome-shell/extensions/amethystora-*@arsslensoft.github.io
# The glass's two keys (ame desktop glass, ame desktop comfort reduce-transparency): Prism by default, the three
# variants the schema offers each a table of materials.js
GLASS=org.gnome.shell.extensions.amethystora-ui.glass
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${GLASS}" | sort | paste -sd ' ')" == \
    "blur-clear blur-frost blur-prism material solid tint-clear tint-frost tint-prism" ]]
# Each glass's blur and tint (R6-28): its own at 1, from half to one and a half
for key in blur-clear blur-frost blur-prism tint-clear tint-frost tint-prism; do
    [[ "$(GSETTINGS_BACKEND=memory gsettings get "${GLASS}" "${key}")" == 1.0 ]]
    [[ "$(GSETTINGS_BACKEND=memory gsettings range "${GLASS}" "${key}" | xargs)" == "range d 0.5 1.5" ]]
done
grep -q "^    materialFor(name) {" "${UI_EXTENSION}/modules/glass.js"
grep -q "^glass-strength \*ARGS:" /usr/share/amethystora/just/desktop.just
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${GLASS}" material)" == "'prism'" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${GLASS}" solid)" == false ]]
[[ "$(sed -n 's/.*<choice value="\([a-z]*\)"\/>.*/\1/p' "${UI_EXTENSION}/schemas/${GLASS}.gschema.xml" | paste -sd ' ')" == \
    "frost prism clear" ]]
[[ "$(grep -c '^    \(frost\|prism\|clear\): {$' "${UI_EXTENSION}/modules/glass/materials.js")" == 3 ]]
# The shader declares what the effect sets, in what a Cogl snippet takes on every GPU (GLSL ES 1.0 too): no version
# or precision line, no explicit level of detail, no loops
for uniform in u_corner u_tone u_rim u_veil u_edge u_lift u_light u_ring u_glow u_glowshape u_sideglow u_sheen u_solid; do
    grep -qE "^uniform (float|vec[234]) ${uniform};" "${UI_EXTENSION}/modules/glass/lens.glsl"
done
[[ -z "$(grep -E '#version|precision|texture2DLod|textureLod|for \(|while \(' "${UI_EXTENSION}/modules/glass/lens.glsl")" ]]
# Comfort follows GNOME's own Reduced Motion and High Contrast and power-profiles-daemon's profile, on signals
# alone, and starts before every module that moves something
grep -q 'org.freedesktop.UPower.PowerProfiles' "${UI_EXTENSION}/modules/comfort.js"
[[ -z "$(grep -E 'timeout_add|setInterval|setTimeout' "${UI_EXTENSION}/modules/comfort.js")" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings range org.gnome.desktop.a11y.interface reduced-motion | xargs)" == \
    "enum no-preference reduce" ]]
MODULE_ORDER="$(sed -n "s/^    \['\([a-z]*\)', [A-Za-z]*\],$/\1/p" "${UI_EXTENSION}/extension.js" | paste -sd ' ')"
for module in tweaks workspaces dock glass motion windows; do
    [[ " ${MODULE_ORDER} " != *" ${module} "* || " ${MODULE_ORDER%%" ${module}"*} " == *" comfort "* ]] ||
        { echo "comfort does not start before ${module}"; exit 1; }
done
[[ " ${MODULE_ORDER} " == *" style comfort "* ]]
# Solid and High Contrast in the glass's colours, from keys every theme's palette has (the render loop above)
for word in amethystora-glass-solid amethystora-glass-contrast -amethystora-glass-ground -amethystora-glass-accent \
    '{{ popover_bg }}'; do
    grep -qF -- "${word}" /usr/share/amethystora/themed/shell-glass.css.tpl
done
test ! -e /etc/dconf/db/distro.d/05-blur-my-shell-extension
# The windows module (snap layouts, the switcher, the Super card, hot corners, show desktop, gestures, places, a copy
# kept in view, dimming, and rounds 5 and 6's drop-down terminal and scratchpad, keep on top, see-through windows, the
# frame, sizes for recording and reopening, plans/ui-ideas/spec-windows.md): its twenty-seven keys and their defaults,
# after the tiling, whose layout it reads, and ame desktop windows, which refuses what it does not know. Super+Z,
# Super+Alt+Return, Super+Alt+P and Super+Alt+T are in KEPT_COMBOS above.
WINDOWS=org.gnome.shell.extensions.amethystora-ui.windows
[[ "$(GSETTINGS_BACKEND=memory gsettings list-recursively "${WINDOWS}" | cut -d' ' -f2- | LC_ALL=C sort | paste -sd ';')" == \
    "corner-bottom-left 'none';corner-bottom-right 'none';corner-top-left 'none';corner-top-right 'none';dim false;drop-down true;drop-down-key ['<Super><Alt>Return'];frame false;gesture-down true;gesture-pinch true;gesture-spread true;gesture-today true;hand-over true;hold-super true;hold-super-delay 1000;keep-on-top true;keep-on-top-key ['<Super><Alt>p'];places true;recording-sizes true;reopen true;reopen-key ['<Super><Alt>t'];scratchpad true;see-through false;see-through-apps @a{si} {};show-desktop false;simple-desktop false;simple-desktop-leave ['<Super><Alt>Home'];simple-tiles @as [];snap true;snap-chooser ['<Super>z'];switcher true" ]] ||
    { echo "the windows module's keys or defaults are not the spec's"; exit 1; }
[[ " ${MODULE_ORDER} " == *" tiling windows "* ]] || { echo "the windows module does not start right after the tiling"; exit 1; }
# A simple desktop (R7-65): every key it sets exists, and takes what it is set to; Super+Alt+Home is free. Hand it over
# (R7-64) says what it is, and ends at the lock
for pair in tiling:enabled keys:super-alone dock:icon-size bars:layout; do
    grep -q "UI}.${pair%:*}\`, '${pair#*:}'" "${UI_EXTENSION}/modules/windows/simple.js"
    GSETTINGS_BACKEND=memory gsettings get "org.gnome.shell.extensions.amethystora-ui.${pair%:*}" "${pair#*:}" >/dev/null
done
grep -qx "<alt><super>home" <<<"${TILING_COMBOS}"$'\n'"${KEPT_COMBOS}" && { echo "Super+Alt+Home is bound elsewhere, and leaves the simple desktop"; exit 1; }
grep -q "This is a courtesy, not a lock" "${UI_EXTENSION}/modules/windows/handover.js"
grep -q "filter-keybinding" "${UI_EXTENSION}/modules/windows/handover.js"
test -s "${UI_EXTENSION}/stylesheets/windows.css"
test -s /usr/share/amethystora/themed/shell-windows.css.tpl
GSETTINGS_BACKEND=memory ame desktop windows >/dev/null
for wrong in "snap maybe" "hold-super 100" "corner middle overview" "corner top-left explode" "gesture swipe on" "places keep" \
    "drop-down maybe" "frame" "see-through-app kitty.desktop 20" "see-through-app kitty.desktop 101" \
    "see-through-app ../kitty.desktop 90" "see-through-app kitty 90"; do
    # shellcheck disable=SC2086
    GSETTINGS_BACKEND=memory ame desktop windows ${wrong} >/dev/null 2>&1 && { echo "ame desktop windows ${wrong} was taken"; exit 1; }
done
# An app's see-through level is kept beside the others' and taken away alone
# (A keyfile of its own: the memory backend keeps nothing from one command to the next)
env GSETTINGS_BACKEND=keyfile XDG_CONFIG_HOME="$(mktemp -d)" bash -c 'ame desktop windows see-through-app kitty.desktop 92 >/dev/null &&
    ame desktop windows see-through-app org.gnome.Nautilus.desktop 80 >/dev/null &&
    ame desktop windows see-through-app kitty.desktop off >/dev/null &&
    [[ "$(gsettings get org.gnome.shell.extensions.amethystora-ui.windows see-through-apps)" == "{'"'"'org.gnome.Nautilus.desktop'"'"': 80}" ]]' ||
    { echo "ame desktop windows see-through-app does not keep each app's level"; exit 1; }
# GNOME's window settings (ame desktop window-settings, R5-12): listed, each set as GNOME's own key, the unknown refused,
# and dialogs left to the tiling while it runs
GSETTINGS_BACKEND=memory ame desktop window-settings >/dev/null
for wrong in "drag-key ctrl" "double-click explode" "focus-follows-pointer maybe" "button-layout close:" "centre-new"; do
    # shellcheck disable=SC2086
    GSETTINGS_BACKEND=memory ame desktop window-settings ${wrong} >/dev/null 2>&1 &&
        { echo "ame desktop window-settings ${wrong} was taken"; exit 1; }
done
env GSETTINGS_BACKEND=keyfile XDG_CONFIG_HOME="$(mktemp -d)" bash -c 'ame desktop window-settings focus-follows-pointer on >/dev/null &&
    ame desktop window-settings drag-key alt >/dev/null && ame desktop window-settings middle-click lower >/dev/null &&
    [[ "$(gsettings get org.gnome.desktop.wm.preferences focus-mode)" == "'"'"'sloppy'"'"'" &&
        "$(gsettings get org.gnome.desktop.wm.preferences mouse-button-modifier)" == "'"'"'<Alt>'"'"'" &&
        "$(gsettings get org.gnome.desktop.wm.preferences action-middle-click-titlebar)" == "'"'"'lower'"'"'" ]] &&
    ! ame desktop window-settings attach-dialogs on >/dev/null 2>&1' ||
    { echo "ame desktop window-settings does not set GNOME's keys, or changes dialogs under the tiling"; exit 1; }
# The window menu's lines go through one hook (windows/menu.js), and the drop-down terminal is kitty started with an
# argument vector of the shell's own
grep -q "kitty', '--class', DROP_DOWN_CLASS" "${UI_EXTENSION}/modules/windows/scratch.js"
[[ "$(grep -rl "showWindowMenuForWindow" "${UI_EXTENSION}/modules/windows" | xargs -n1 basename)" == menu.js ]]
# A window's title is never markup in it, and it runs nothing through a shell
[[ -z "$(grep -rnE 'use_markup = true|spawn_command_line|GLib\.spawn_async' "${UI_EXTENSION}/modules/windows.js" \
    "${UI_EXTENSION}/modules/windows")" ]]
# The dock (plans/beyond/spec-dock.md): its fourteen keys and their defaults; the choices of each that the schema,
# ame desktop dock and Control give alike; ame desktop dock refusing what it does not know; and what it may never do:
# take a name for markup, send files to a phone itself (only the Phone app may), or carry another maker's names
DOCK=org.gnome.shell.extensions.amethystora-ui.dock
[[ "$(GSETTINGS_BACKEND=memory gsettings list-recursively "${DOCK}" | cut -d' ' -f2- | LC_ALL=C sort | paste -sd ';')" == \
    "autohide 'off';edge 'bottom';folders @a{sas} {};icon-size 48;indicator 'bar';launch 'halo';magnify 'swell';magnify-amount 1.6000000000000001;previews true;recent true;screens 'primary';stacks ['xdg:DOWNLOAD'];updating true;workspace-apps false" ]] ||
    { echo "the dock's keys or defaults are not the spec's"; exit 1; }
for key in magnify launch indicator edge screens autohide; do
    schema="$(GSETTINGS_BACKEND=memory gsettings range "${DOCK}" "${key}" | sed -n '2,$p' | tr -d "'" | LC_ALL=C sort | xargs)"
    recipe="$(sed -n "s/^ *${key}) values=(\(.*\)) ;;\$/\1/p" /usr/share/amethystora/just/desktop.just | tr ' ' '\n' |
        LC_ALL=C sort | xargs)"
    control="$(grep -o "ame desktop dock ${key} [a-z]*" /usr/libexec/amethystora-control-list | awk '{print $5}' |
        LC_ALL=C sort -u | xargs)"
    [[ -n "${schema}" && "${schema}" == "${recipe}" && "${schema}" == "${control}" ]] ||
        { echo "the dock's ${key}: the schema offers ${schema}, ame desktop dock ${recipe}, Control ${control}"; exit 1; }
done
[[ "$(GSETTINGS_BACKEND=memory ame desktop dock | wc -l)" == 14 ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings range "${DOCK}" icon-size | xargs)" == "range i 40 64" ]]
for wrong in "size 30" "size 65" "size big" "magnify wobble" "amount 300" "edge top" "recent maybe" "stack add /nonexistent" "folder" "sideways"; do
    # shellcheck disable=SC2086
    GSETTINGS_BACKEND=memory ame desktop dock ${wrong} >/dev/null 2>&1 && { echo "ame desktop dock ${wrong} was taken"; exit 1; }
done
test -s "${UI_EXTENSION}/schemas/${DOCK}.gschema.xml"
for class in amethystora-dock-bar amethystora-dock-glow amethystora-dock-halo amethystora-dock-number; do
    grep -q "\.${class}\b" /usr/share/amethystora/themed/shell-dock.css.tpl
done
[[ -z "$(grep -rnE 'use_markup = true|use_markup: true|set_markup\(' "${UI_EXTENSION}/modules/dock.js" \
    "${UI_EXTENSION}/modules/dock")" ]] || { echo "the dock takes text for markup"; exit 1; }
[[ -z "$(grep -rn SendFiles "${UI_EXTENSION}/modules/dock.js" "${UI_EXTENSION}/modules/dock" \
    "${UI_EXTENSION}/modules/connect.js" "${UI_EXTENSION}/modules/connect")" ]] || { echo "the shell sends files itself"; exit 1; }
[[ -z "$(grep -rniE 'macos|mac os|tahoe|genie|mission control|spotlight|launchpad|quick look|stage manager|exposé' \
    "${UI_EXTENSION}/modules/dock.js" "${UI_EXTENSION}/modules/dock" "${UI_EXTENSION}/stylesheets/dock.css" \
    /usr/share/amethystora/themed/shell-dock.css.tpl "${UI_EXTENSION}/schemas/${DOCK}.gschema.xml")" ]] ||
    { echo "the dock names another maker's desktop"; exit 1; }
# The Phone app sends a drop at once with --now, which its own tests cover in the apps stage
grep -qa -- '--now' /usr/lib/amethystora-phone/amethystora-phone
# An account that holds a list of its own never sees the default list of extensions, so the user-setup hook
# puts it back after every update; the default it reads has to be the one the image compiled
test -x /usr/share/amethystora/user-setup.hooks.d/15-extensions.sh
GSETTINGS_BACKEND=memory gsettings get org.gnome.shell enabled-extensions | grep -q "'amethystora-ui@arsslensoft.github.io'"
# Xwayland only when an X11 app needs it, ibus-x11 with it (17-cleanup.sh), and no idle input engine
test ! -e /etc/xdg/autostart/ibus-mozc-launch-xwayland.desktop
test -x /etc/xdg/Xwayland-session.d/10-ibus-x11
if rpm -q ibus-typing-booster >/dev/null; then
    echo "ibus-typing-booster is back: IBus starts it in every session"
    exit 1
fi
# Six fixed workspaces on Super+N, which moves the dash to Alt+N
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.desktop.wm.preferences num-workspaces)" == "6" ]]
# As a GNOME session reads it: GNOME Shell's override has a section for that desktop alone
[[ "$(XDG_CURRENT_DESKTOP=GNOME GSETTINGS_BACKEND=memory gsettings get org.gnome.mutter dynamic-workspaces)" == "false" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.desktop.wm.keybindings switch-to-workspace-6)" == "['<Super>6']" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.keybindings switch-to-application-1)" == "['<Alt>1']" ]]
# custom-keybindings in zz0-amethystora-modifications lists every custom keybinding GNOME reads, the
# Amethystora ones in 06-amethystora-keybindings included
CUSTOM_KEYBINDINGS="$(GSETTINGS_BACKEND=memory gsettings get org.gnome.settings-daemon.plugins.media-keys custom-keybindings)"
grep -q "custom0/'" <<<"${CUSTOM_KEYBINDINGS}"
for index in 20 21 22 23 24 25 26 27 28 29 30; do
    grep -q "custom${index}/'" <<<"${CUSTOM_KEYBINDINGS}"
    grep -q "^\[org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom${index}\]$" \
        /etc/dconf/db/distro.d/06-amethystora-keybindings
done
# Super+Alt+N is the quick note's
grep -A2 -x "\[org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom30\]" \
    /etc/dconf/db/distro.d/06-amethystora-keybindings | paste -sd ' ' |
    grep -qF "binding='<Super><Alt>n' command='amethystora-notes capture'"

# Theme switching: the CLI, its library, the templates every theme renders, and the themes themselves
test -x /usr/bin/amethystora-theme
test -x /usr/bin/amethystora-menu

# Web apps: a launcher that reopens the site through amethystora-webapp, in Firefox by default, with
# the window class GNOME needs to show it under its own name; anything but http(s) is refused. Firefox
# has no app window, so the launcher opens the site in a profile of its own and takes that window's
# class, and removing the web app removes the profile. With a Chromium-based default browser the
# launcher opens the browser's app window instead and takes that one's class
test -x /usr/bin/amethystora-webapp
WEBAPP_HOME="$(mktemp -d)"
WEBAPP_CLASS=amethystora-webapp-example.com_app
HOME="${WEBAPP_HOME}" amethystora-webapp install "Test App" example.com/app web-browser
WEBAPP_DESKTOP="${WEBAPP_HOME}/.local/share/applications/amethystora-webapp-test-app.desktop"
grep -qx 'Exec=amethystora-webapp "https://example.com/app"' "${WEBAPP_DESKTOP}"
grep -qx "StartupWMClass=${WEBAPP_CLASS}" "${WEBAPP_DESKTOP}"
HOME="${WEBAPP_HOME}" amethystora-webapp install Bad "javascript:alert(1)" web-browser 2>/dev/null && false
mkdir "${WEBAPP_HOME}/bin"
# Opens the web app from its launcher with the browser named as the default one, a stand-in that
# prints what it was started with. GLib names the launcher and the process it started; exec keeps the
# shell's PID for the script
webapp_open() {
    printf '#!/usr/bin/bash\necho %s.desktop\n' "$1" >"${WEBAPP_HOME}/bin/xdg-settings"
    chmod +x "${WEBAPP_HOME}/bin/xdg-settings"
    printf '[Desktop Entry]\nExec=echo %%u\n' >"${WEBAPP_HOME}/.local/share/applications/$1.desktop"
    HOME="${WEBAPP_HOME}" PATH="${WEBAPP_HOME}/bin:${PATH}" GIO_LAUNCHED_DESKTOP_FILE="${WEBAPP_DESKTOP}" \
        bash -c 'export GIO_LAUNCHED_DESKTOP_FILE_PID=$$; exec amethystora-webapp example.com/app'
}
test "$(webapp_open chromium-browser)" = "--app=https://example.com/app"
grep -qx 'StartupWMClass=chrome-example.com__app-Default' "${WEBAPP_DESKTOP}"
WEBAPP_PROFILE="${WEBAPP_HOME}/.local/share/amethystora-webapp/${WEBAPP_CLASS}"
test "$(webapp_open firefox)" = \
    "--name ${WEBAPP_CLASS} --class ${WEBAPP_CLASS} --profile ${WEBAPP_PROFILE} https://example.com/app"
grep -qx "StartupWMClass=${WEBAPP_CLASS}" "${WEBAPP_DESKTOP}"
grep -q legacyUserProfileCustomizations "${WEBAPP_PROFILE}/user.js"
test -s "${WEBAPP_PROFILE}/chrome/userChrome.css"
HOME="${WEBAPP_HOME}" amethystora-webapp remove "Test App" >/dev/null
test ! -e "${WEBAPP_DESKTOP}"
test ! -e "${WEBAPP_PROFILE}"
rm -rf "${WEBAPP_HOME}"

# Packages from other distributions (amethystora-pkg): the command and its short name, which answers to
# the name it was typed as, and `ame pkg`
test -x /usr/bin/amethystora-pkg
[[ "$(readlink /usr/bin/amepkg)" == amethystora-pkg ]]
amepkg --help | grep -q "^Usage: amepkg "
ame pkg --help | grep -q "^Usage: amethystora-pkg "
# The daily upgrade, started by its timer, waits for the network with nm-online: its user unit cannot wait for
# network-online.target
test -x /usr/bin/nm-online
grep -q "nm-online -q" /usr/bin/amethystora-pkg
for completion in bash-completion/completions/amethystora-pkg bash-completion/completions/amepkg \
    fish/vendor_completions.d/amethystora-pkg.fish fish/vendor_completions.d/amepkg.fish zsh/site-functions/_amethystora-pkg; do
    test -s "/usr/share/${completion}"
done
grep -q "^#compdef amethystora-pkg amepkg$" /usr/share/zsh/site-functions/_amethystora-pkg
# Every list --json parses, the containers one through a stand-in for podman with one container in it
PKG_HOME="$(mktemp -d)"
mkdir -p "${PKG_HOME}/bin" "${PKG_HOME}/.local/state/amethystora/pkg/containers"
cat >"${PKG_HOME}/bin/podman" <<'EOF'
#!/usr/bin/bash
echo '[{"Names": ["pkg-test"], "State": "exited", "Labels": {"manager": "distrobox",
    "amethystora.pkg.manager": "amethystora-pkg", "amethystora.pkg.name": "test", "amethystora.pkg.template": "debian",
    "amethystora.pkg.pkgmanager": "apt", "amethystora.pkg.image": "quay.io/x/y:1", "amethystora.pkg.init": "0"}}]'
EOF
chmod +x "${PKG_HOME}/bin/podman"
HOME="${PKG_HOME}" PATH="${PKG_HOME}/bin:${PATH}" amethystora-pkg containers list --json |
    jq -e 'length == 1 and (.[0] | .name == "test" and .manager == "apt" and .status == "exited"
        and has("home") and has("init") and has("exported_apps") and has("last_upgrade"))' >/dev/null
HOME="${PKG_HOME}" amethystora-pkg templates list --json |
    jq -e 'length >= 7 and all(has("name") and has("base") and has("manager") and has("packages") and .built_in and .verified)' >/dev/null
HOME="${PKG_HOME}" amethystora-pkg managers list --json |
    jq -e 'length >= 5 and all(.built_in and (.install | length > 0) and (.clean | length > 0) and has("need_sudo"))' >/dev/null
# The AUR's manager and template are there only while ame apps aur has turned it on
HOME="${PKG_HOME}" amethystora-pkg managers show paru >/dev/null 2>&1 && false
mkdir -p "${PKG_HOME}/.config/amethystora" && touch "${PKG_HOME}/.config/amethystora/aur"
HOME="${PKG_HOME}" amethystora-pkg templates show arch-aur | jq -e '.manager == "paru" and .own_home' >/dev/null
# A container's home folder reaches distrobox's eval: one holding a character a shell reads is refused
# before anything is made (amethystora-restore-setup passes one read from a backup)
PKG_OUT="$(HOME="${PKG_HOME}" PATH="${PKG_HOME}/bin:${PATH}" amethystora-pkg containers new --template debian \
    --name hometest --home "${PKG_HOME}/a;touch ${PKG_HOME}/pwned" --no-prompt --yes 2>&1 || true)"
grep -q "letters, digits" <<<"${PKG_OUT}"
test ! -e "${PKG_HOME}/pwned"
rm -rf "${PKG_HOME}"
ame --show apps::aur >/dev/null
# Every built-in template's image is pinned by digest, and the pins ship in the signed image, which is
# what makes a container made from one start from exactly that image. Each names a built-in manager.
for template in /usr/share/amethystora/pkg/templates/*.json /usr/share/amethystora/pkg/aur/templates/*.json; do
    jq -e '.base | test("^[a-z0-9.-]+/[^@ ]+@sha256:[0-9a-f]{64}$")' "${template}" >/dev/null
    manager="$(jq -r .manager "${template}")"
    test -f "/usr/share/amethystora/pkg/managers/${manager}.json" -o -f "/usr/share/amethystora/pkg/aur/managers/${manager}.json"
done
# A .deb or an .rpm opens in it, in a terminal, scanned and explained first
PKG_DESKTOP=/usr/share/applications/amethystora-pkg-install.desktop
command -v desktop-file-validate >/dev/null && desktop-file-validate "${PKG_DESKTOP}"
grep -qx "MimeType=application/vnd.debian.binary-package;application/x-rpm;" "${PKG_DESKTOP}"
grep -q "^Exec=/usr/libexec/amethystora-in-terminal amethystora-pkg install %f$" "${PKG_DESKTOP}"
for type in application/vnd.debian.binary-package application/x-rpm; do
    grep -qx "${type}=amethystora-pkg-install.desktop" /etc/xdg/mimeapps.list
done
# A Windows program opens in a handler of its own, in a terminal, for every type shared-mime-info gives one: scanned,
# its signer said, and the same app for Linux, its web version or an alternative offered before Bottles
WIN_DESKTOP=/usr/share/applications/amethystora-windows-program.desktop
test -x /usr/libexec/amethystora-windows-program
command -v osslsigncode >/dev/null
command -v desktop-file-validate >/dev/null && desktop-file-validate "${WIN_DESKTOP}"
grep -q "^Exec=/usr/libexec/amethystora-in-terminal /usr/libexec/amethystora-windows-program %f$" "${WIN_DESKTOP}"
for type in vnd.microsoft.portable-executable x-msdownload x-ms-dos-executable x-dosexec x-ms-ne-executable x-msi \
    x-ms-shortcut msix appx msixbundle appxbundle x-bat; do
    grep -q "^MimeType=\(.*;\)\?application/${type};" "${WIN_DESKTOP}"
    grep -qx "application/${type}=amethystora-windows-program.desktop" /etc/xdg/mimeapps.list
done
# Its map: known fields only, every pattern compiles, every Flatpak ID has an ID's shape, and no entry names one twice
WIN_MAP=/usr/share/amethystora/windows-programs.json
jq -e 'length >= 30 and all(.[]; .match as $m |
    (.name | type == "string") and (keys - ["name", "match", "flatpak", "web", "launcher", "ame", "alternatives", "wine", "driver"] == [])
    and ("x" | test($m; "i") | type == "boolean")
    and ([.flatpak // empty, .alternatives[]?.flatpak] | all(test("^[A-Za-z][A-Za-z0-9_-]*(\\.[A-Za-z0-9_-]+)+$")) and length == (unique | length))
    and ((.web // "https://") | startswith("https://")))' "${WIN_MAP}" >/dev/null
for launcher in $(jq -r '.[].launcher // empty' "${WIN_MAP}"); do
    test -f "/usr/share/applications/${launcher}"
done
jq -r '.[].ame // empty' "${WIN_MAP}" | while read -r first second; do
    ame --show "${first}::${second}" >/dev/null
done
[[ "$(/usr/libexec/amethystora-windows-program --match 'SpotifySetup (1).exe' | jq -r .flatpak)" == com.spotify.Client ]]
[[ "$(/usr/libexec/amethystora-windows-program --match 'Firefox Installer.exe' | jq -r .launcher)" == org.mozilla.firefox.desktop ]]
[[ -z "$(/usr/libexec/amethystora-windows-program --match 'something-else.exe')" ]]
# DistroShelf, the window onto the containers, comes with every machine, as Bazaar and Flatseal do
for preinstall in bazaar flatseal distroshelf; do
    test -f "/usr/share/flatpak/preinstall.d/${preinstall}.preinstall"
done
grep -q "^\[Flatpak Preinstall com.ranfdev.DistroShelf\]$" /usr/share/flatpak/preinstall.d/distroshelf.preinstall
grep -q "^\[Flatpak Preinstall com.ranfdev.DistroShelf\]$" "${DEFAULT_FLATPAKS}" && false
# So does Extension Manager, for extensions.gnome.org. The logo menu's "Extensions" opens GNOME's own app,
# since Extension Manager shows no details of an extension extensions.gnome.org lacks, the image's own
grep -q "^\[Flatpak Preinstall com.mattjakeman.ExtensionManager\]$" "${DEFAULT_FLATPAKS}"
grep -q "app: 'org.gnome.Extensions.desktop'" "${LOGO_MENU_EXTENSION}/extension.js"
# Its containers are upgraded by a timer of each account's own. uupd's module for containers stays off:
# it upgraded every container of everyone, and made manual updates slow.
test -L /etc/systemd/user/timers.target.wants/amethystora-pkg-upgrade.timer
grep -q "^ExecStart=/usr/bin/amethystora-pkg upgrade-all$" /usr/lib/systemd/user/amethystora-pkg-upgrade.service
jq -e '.modules.distrobox.disable == true' /etc/uupd/config.json >/dev/null
# Nothing this adds is named after the tools it does the work of. The manual alone may say, once, that
# importing reads their file format; the agent's index leaves that sentence out. The embedding model under
# agents/knowledge/model is a third party's vocabulary, not text of Amethystora's, and is left out of the search.
PKG_NAMES='\b(vanilla[ -]?os|apx|vso|sideload)\b'
grep -rliE --exclude-dir=model "${PKG_NAMES}" /usr/bin/amethystora-pkg /usr/share/amethystora/pkg /usr/share/bash-completion/completions/amepkg \
    /usr/share/bash-completion/completions/amethystora-pkg /usr/share/fish/vendor_completions.d/amepkg.fish \
    /usr/share/fish/vendor_completions.d/amethystora-pkg.fish /usr/share/zsh/site-functions/_amethystora-pkg "${PKG_DESKTOP}" \
    /usr/lib/systemd/user/amethystora-pkg-upgrade.service /usr/lib/systemd/user/amethystora-pkg-upgrade.timer \
    /usr/libexec/amethystora-gpu-check /usr/libexec/amethystora-update-alert /usr/libexec/amethystora-setup-manifest \
    /usr/libexec/amethystora-restore-setup /usr/share/amethystora/agents /usr/share/amethystora/just \
    /usr/share/amethystora/user-setup.hooks.d && false
# A translation of that page (manual/<language>/software.md) may say the same, once, in its language.
SOFTWARE_PAGES='^/usr/share/amethystora/manual/([a-z]{2,3}(-[A-Za-z0-9]{2,8})*/)?software\.md$'
[[ -z "$(grep -rliE "${PKG_NAMES}" /usr/share/amethystora/manual | grep -vE "${SOFTWARE_PAGES}")" ]]
for page in /usr/share/amethystora/manual/software.md /usr/share/amethystora/manual/*/software.md; do
    [[ -e "${page}" ]] || continue
    [[ "$(grep -ciE "${PKG_NAMES}" "${page}")" -le 1 ]]
done

# A setup to make again on a new machine: every backup stores one beside the files, and `ame
# restore-setup` reads it back. Neither may delete anything from the repository.
test -x /usr/libexec/amethystora-setup-manifest
test -x /usr/libexec/amethystora-restore-setup
grep -q "/usr/libexec/amethystora-setup-manifest" /usr/libexec/amethystora-backup
grep -q "^restore-setup SNAPSHOT=" /usr/share/amethystora/just/backup.just
grep -vE "^[[:space:]]*#" /usr/libexec/amethystora-restore-setup | grep -qE "restic +(forget|prune)" && false
# The same setup in a file of the account's own (ame setup save, diff, apply), which names things and
# never carries an address or a command, tried against stand-ins for flatpak, Homebrew and GNOME
test -x /usr/libexec/amethystora-setup
ame --show setup::apply >/dev/null
bash /ctx/build_files/shared/test-setup.sh
# The image switch it makes goes through the rebase helper, which takes signed images only and offers the
# image without NVIDIA's driver too
grep -q "^rebase IMAGE=\"\" STREAM=\"\":" /usr/share/amethystora/just/system.just
jq -e '.images == ["amethystora", "amethystora-dx", "amethystora-nvidia-open", "amethystora-dx-nvidia-open",
    "amethystora-nvidia-lts", "amethystora-dx-nvidia-lts"]' <<<"$(/usr/bin/amethystora-rollback-helper --choices)" >/dev/null
# ...and, with Secure Boot on, switches to an NVIDIA image only once the key NVIDIA's driver is signed with is
# enrolled: before, the NVIDIA image would start to a black screen
grep -q 'not-enrolled .\*/akmods-amethystora' /usr/bin/amethystora-rollback-helper

# The NVIDIA check, once per machine, through the rebase helper, never switching by itself
test -x /usr/libexec/amethystora-gpu-check
grep -q "^ExecCondition=/usr/libexec/amethystora-gpu-check pending$" /usr/lib/systemd/system/amethystora-gpu-check.service
grep -q "ame system rebase" /usr/libexec/amethystora-gpu-check
grep -q "kernel lockdown" /usr/libexec/amethystora-gpu-check
# ...offering the image whose driver drives the card, by its PCI device ID: the open driver from Turing (0x1E00)
# on, the 580 branch's closed one from Maxwell (0x1340) to Volta, and nouveau, on the image without NVIDIA's
# driver, for anything older. Tried against stand-in cards and images.
GPU_TEST="$(mktemp -d)"
gpu_offer() {
    local flavor="$1" card
    rm -rf "${GPU_TEST}/pci"
    shift
    for card in "$@"; do
        mkdir -p "${GPU_TEST}/pci/${card}"
        echo 0x030000 >"${GPU_TEST}/pci/${card}/class"
        echo 0x10de >"${GPU_TEST}/pci/${card}/vendor"
        echo "${card}" >"${GPU_TEST}/pci/${card}/device"
    done
    mkdir -p "${GPU_TEST}/pci"
    jq -n --arg flavor "${flavor}" '{"image-name": ("amethystora-dx" + (if $flavor == "main" then "" else "-" + $flavor end)),
        "image-flavor": $flavor, "image-tag": "stable"}' >"${GPU_TEST}/info.json"
    AMETHYSTORA_PCI_DEVICES="${GPU_TEST}/pci" AMETHYSTORA_IMAGE_INFO="${GPU_TEST}/info.json" /usr/libexec/amethystora-gpu-check offer
}
[[ "$(gpu_offer main 0x2684)" == "nvidia-card amethystora-dx-nvidia-open" ]]
[[ "$(gpu_offer main 0x1b80)" == "nvidia-lts-card amethystora-dx-nvidia-lts" ]]
[[ "$(gpu_offer nvidia-open 0x1b80)" == "nvidia-lts-card amethystora-dx-nvidia-lts" ]]
[[ "$(gpu_offer nvidia-open 0x1e02 0x1b80)" == "nvidia-lts-card amethystora-dx-nvidia-lts" ]]
[[ -z "$(gpu_offer main 0x1180)" ]]
[[ "$(gpu_offer nvidia-lts 0x1180)" == "old-nvidia-card amethystora-dx" ]]
[[ "$(gpu_offer nvidia-open)" == "no-nvidia-card amethystora-dx" ]]
[[ -z "$(gpu_offer nvidia-lts 0x2684)" && -z "$(gpu_offer nvidia-lts 0x1340)" && -z "$(gpu_offer nvidia-open 0x1e00)" ]]
rm -rf "${GPU_TEST}"
# The image knows its own flavor, which the check and the doctor read: the last part of its name, or main
jq -e --arg name "${IMAGE_NAME}" '."image-flavor" == ($name | capture("-(?<f>nvidia-(open|lts))$").f // "main")' \
    /usr/share/amethystora/image-info.json >/dev/null
grep -q -- "--action" /usr/libexec/amethystora-notify-users

# Resetting settings, the step before starting over: /etc put back as the image ships it, less what the
# machine knows and everything Security sets, saved first and undone byte for byte, tried on a stand-in /etc
test -x /usr/libexec/amethystora-reset-settings
ame --show system::reset-settings >/dev/null
bash /ctx/build_files/shared/test-reset-settings.sh

# Pools of disks (ame system raid): one helper, which erases nothing without a terminal and ERASE typed,
# whose status Control and its own hourly check read, tried against a stand-in machine since the build
# has no disks to spare. Its two timers run on every image and skip a machine with nothing to watch. The
# TPM unlock and the report's tile take the system's own LUKS from it, never the first one lsblk lists.
test -x /usr/libexec/amethystora-raid
bash -n /usr/libexec/amethystora-raid
ame --show system::raid >/dev/null
jq -e '.version == 1 and (.spare | type) == "array" and ([.pools[] | select(.managed)] == [])' \
    <<<"$(/usr/libexec/amethystora-raid status --json)" >/dev/null
bash /ctx/build_files/shared/test-raid.sh
# It tells you the truth (plans/ui-ideas/r7/spec-truth.md): where a connection breaks, a removable disk still being written
# to, what woke the computer, and the warning before the system's disk fails. The new shell files block nothing and hold no
# markup; the check contacts only the router and resolved, and resolved never from its cache; a failing disk says "may";
# the wake setting is one udev line of a fixed shape, which the watcher accepts and nothing more
TRUTH_FILES=(modules/tools/reach.js modules/tools/reachlogic.js modules/pill/stick.js modules/pill/stickmodel.js
    modules/power/slept.js modules/power/sleptlogic.js)
for file in "${TRUTH_FILES[@]}"; do
    test -s "${UI_EXTENSION}/${file}"
    if grep -nE '_sync\b|file_get_contents|use_markup = true|use_markup: true|set_markup\(' "${UI_EXTENSION}/${file}"; then
        echo "${file} blocks the shell or takes markup"
        exit 1
    fi
done
env LD_LIBRARY_PATH=/apps-stage/usr/lib64 NODE_ICU_DATA="$(echo /apps-stage/usr/share/node-*/icudata)" /apps-stage/usr/bin/node /ctx/build_files/shared/test-truth.mjs
bash /ctx/build_files/shared/test-sleep-report.sh
bash /ctx/build_files/shared/test-disk-health.sh
# The name looked up is the one NetworkManager's own connectivity check asks, read from its address, and none while that check is off
grep -q 'connectivity_check_enabled' "${UI_EXTENSION}/modules/tools/reach.js"
grep -q 'connectivity_check_uri' "${UI_EXTENSION}/modules/tools/reach.js"
[[ -z "$(grep -n 'fedoraproject' "${UI_EXTENSION}/modules/tools/reach.js" "${UI_EXTENSION}/modules/tools/reachlogic.js")" ]]
grep -q "row network-reach " /usr/libexec/amethystora-sends
[[ "$(grep -c "'stick'" "${UI_EXTENSION}/modules/pill/activities.js")" == 1 ]]
systemd-analyze verify --man=no --recursive-errors=no /usr/lib/systemd/system/amethystora-disk-health.{service,timer}
grep -q "may" <<<"$(sed -n '/^cmd_notify/,/^}/p' /usr/libexec/amethystora-disk-health)"
# What woke the computer is the one thing the power module runs: its own helper, and pkexec of it
[[ "$(grep -oE "Subprocess\.new\(\[[A-Za-z']+" "${UI_EXTENSION}/modules/power/slept.js" | LC_ALL=C sort -u | paste -sd ' ')" == \
    "Subprocess.new([HELPER Subprocess.new([PKEXEC" ]]
grep -q "^const PKEXEC = '/usr/bin/pkexec';" "${UI_EXTENSION}/modules/power/slept.js"
[[ -z "$(grep -nE 'Util\.spawn|spawn_command_line|GLib\.spawn|kill\(|SIGKILL|force_exit' "${UI_EXTENSION}/modules/power/slept.js")" ]]
# No ProtectClock on the disk reading: it may shut smartctl out of the disks it reads, and nothing would say so
[[ -z "$(grep '^ProtectClock' /usr/lib/systemd/system/amethystora-disk-health.service)" ]]
grep -q 'udev/rules.d/90-amethystora-wake.rules' /usr/libexec/amethystora-security-watch
for unit in amethystora-raid-check amethystora-raid-scrub; do
    grep -q "^ExecCondition=/usr/libexec/amethystora-raid present$" "/usr/lib/systemd/system/${unit}.service"
done
grep -q "^ExecCondition=/usr/libexec/amethystora-raid present-system$" /usr/lib/systemd/system/amethystora-raid-boot.service
# An update that does not start goes back by itself (R7-52; manual: updates.md#if-an-update-does-not-start): off as the
# image ships, both units conditional on the owner's mark and never on a sealed image; the helper against stand-ins, GRUB's
# script checked; the automatic update asks and Update now does not; it names no home folder, and its way back is by
# id: by position the second entry can be the firmware settings
test -x /usr/libexec/amethystora-boot-guard
test ! -e /var/lib/amethystora/boot-guard/on
for unit in amethystora-boot-guard amethystora-boot-guard-good; do
    # The owner's mark, or the one a turn-off leaves when /boot would not take it (either); never on a sealed image. A
    # skipped unit must pull in nothing: ordered after /boot's mount, never requiring it, and no ProtectClock, which
    # closes the device list grub2-editenv needs for the partition under /boot
    grep -qxF 'ConditionPathExists=|/var/lib/amethystora/boot-guard/on' "/usr/lib/systemd/system/${unit}.service"
    grep -qxF 'ConditionPathExists=|/var/lib/amethystora/boot-guard/cleanup' "/usr/lib/systemd/system/${unit}.service"
    grep -qxF 'ConditionPathExists=!/usr/share/amethystora/sealed' "/usr/lib/systemd/system/${unit}.service"
    grep -q '^After=local-fs.target boot.mount$' "/usr/lib/systemd/system/${unit}.service"
    grep -qE '^(RequiresMountsFor|ProtectClock)=' "/usr/lib/systemd/system/${unit}.service" && false
done
grep -q '^Before=ostree-finalize-staged.service$' /usr/lib/systemd/system/amethystora-boot-guard.service
# The disks' unit and the guard both write custom.cfg: one stops before the other, and a command typed meanwhile takes the
# same lock
grep -q '^After=amethystora-raid-boot.service$' /usr/lib/systemd/system/amethystora-boot-guard.service
# A good start is the display manager and a session on the seat, not graphical.target: a stray oneshot that never ends holds
# that target for good, and a slow greeter is waited for as long as the start lasts
grep -qE '^After=.*graphical.target' /usr/lib/systemd/system/amethystora-boot-guard-good.service && false
grep -q '^After=display-manager.service ' /usr/lib/systemd/system/amethystora-boot-guard-good.service
grep -q '^TimeoutStartSec=infinity$' /usr/lib/systemd/system/amethystora-boot-guard-good.service
# What the guard writes to /boot is flushed to the disk (GRUB reads ext4 without its journal); the disks' unit does the same
# only while the guard is on, and is as it was otherwise
grep -qF 'fsfreeze -f /boot' /usr/libexec/amethystora-boot-guard
grep -qF '[[ -e /var/lib/amethystora/boot-guard/on ]] && command -v fsfreeze' /usr/libexec/amethystora-raid
grep -qF '/run/amethystora/boot-cfg.lock' /usr/libexec/amethystora-raid
grep -qF 'LOCK="${R}/run/amethystora/boot-cfg.lock"' /usr/libexec/amethystora-boot-guard
grep -q '^ExecCondition=/usr/libexec/amethystora-boot-guard update-ok$' /usr/lib/systemd/system/uupd.service.d/51-amethystora-boot-guard.conf
systemctl cat uupd-manual.service 2>/dev/null | grep -q boot-guard && false
grep -qE '/var/home|\$\{?HOME|/home/|default=1' /usr/libexec/amethystora-boot-guard && false
# bootupd's static configuration must still read custom.cfg after its entries, which is the only way in
grep -qF 'custom.cfg' /usr/lib/bootupd/grub2-static/configs.d/41_custom.cfg
bash /ctx/build_files/shared/test-boot-guard.sh
systemd-analyze verify --man=no --recursive-errors=no /usr/lib/systemd/system/amethystora-boot-guard{,-good}.service
systemd-analyze verify --man=no --recursive-errors=no /usr/lib/systemd/system/amethystora-raid-{check,scrub}.{service,timer} \
    /usr/lib/systemd/system/amethystora-raid-{boot,mirror}.service
# A mirror's first disk's /boot joins it while nothing holds /boot: before boot.mount, and only when one waits
grep -q "^Before=boot.mount" /usr/lib/systemd/system/amethystora-raid-mirror.service
grep -q "^ConditionPathExists=/var/lib/amethystora/raid/mirror.json$" /usr/lib/systemd/system/amethystora-raid-mirror.service
# A system over several disks starts with one missing from the boot menu: the initramfs lets its btrfs mount
# degraded when the kernel command line asks for it (and only then), which the entry amethystora-raid writes does
grep -q "usr/lib/udev/rules.d/65-amethystora-btrfs-degraded.rules" <<<"${INITRAMFS_FILES}"
# ...and dracut then waits no longer for the missing disk's LUKS (its initqueue's 90-crypt.sh)
grep -q "dracut/hooks/cmdline/99-amethystora-degraded-crypt.sh" <<<"${INITRAMFS_FILES}"
grep -qF 'ENV{rootflags}!="*degraded*", GOTO' /usr/lib/dracut/modules.d/90amethystora-degraded/65-amethystora-btrfs-degraded.rules
[[ ! -e /usr/lib/udev/rules.d/65-amethystora-btrfs-degraded.rules ]]
grep -q "/usr/libexec/amethystora-raid root-luks" /usr/share/amethystora/just/security.just
grep -q "/usr/libexec/amethystora-raid root-luks" /usr/libexec/amethystora-security-status
grep -q "Open Control" /usr/libexec/amethystora-raid
# A machine started from an older version than its default one is told so at login, and System Updates
# offers to keep it or to restart into the newest
test -x /usr/libexec/amethystora-update-alert
test -L /etc/systemd/user/graphical-session.target.wants/amethystora-update-alert.service
grep -qF '"pkexec", &["/usr/bin/bootc", "rollback"]' /ctx/apps/update/src/backend.rs
# The manual's welcome page, once, at an account's first login
test -x /usr/share/amethystora/user-setup.hooks.d/17-welcome.sh
grep -q "^version-script welcome user " /usr/share/amethystora/user-setup.hooks.d/17-welcome.sh
grep -q "amethystora-manual welcome$" /usr/share/amethystora/user-setup.hooks.d/17-welcome.sh
# ...with the offer of classic windows a little after it, which only asks: `ame desktop layout classic` acts
# on it by turning the tiling off in its own key, which an update leaves as it is
test -x /usr/libexec/amethystora-layout-offer
grep -q "/usr/libexec/amethystora-layout-offer$" /usr/share/amethystora/user-setup.hooks.d/17-welcome.sh
grep -q "^    schema=org.gnome.shell.extensions.amethystora-ui.tiling$" /usr/share/amethystora/just/desktop.just
grep -q "paperwm" /usr/share/amethystora/just/desktop.just && false
grep -q "paperwm@paperwm.github.com" /usr/share/amethystora/user-setup.hooks.d/15-extensions.sh && false

# The Amethystora apps (13-apps.sh): one program, which each app's own path is a hard link to. It starts as
# each of them and says which it is, which also proves every library it links against is on the image, and
# each launcher's window class is the id the program gives that app's window, by which GNOME finds the
# launcher's name and icon. Nothing of the Electron windows is left beside it but their catalogs, and the
# licences of the crates built into it are beside the list of them (build-apps.sh). The stone of the
# wordmarks, which the program carries, is the logo's own, cropped the way branding/generate.mjs crops it.
APPS_PROGRAM=/usr/lib/amethystora-apps/amethystora-apps
test -x "${APPS_PROGRAM}"
for app in manual security update notes logs backups vault phone control shelf trim help; do
    test -x "/usr/bin/amethystora-${app}"
    [[ "/usr/lib/amethystora-${app}/amethystora-${app}" -ef "${APPS_PROGRAM}" ]]
    [[ "$(env -u DISPLAY -u WAYLAND_DISPLAY "/usr/lib/amethystora-${app}/amethystora-${app}" --version)" == "amethystora-${app} "* ]]
    [[ -z "$(find "/usr/lib/amethystora-${app}" -mindepth 1 -not -name "amethystora-${app}" -not -path '*/resources' \
        -not -path '*/resources/app' -not -path '*/resources/app/locale' -not -path '*/resources/app/locale/*.json')" ]]
    APP_DESKTOP="/usr/share/applications/amethystora-${app}.desktop"
    command -v desktop-file-validate >/dev/null && desktop-file-validate "${APP_DESKTOP}"
    APP_CLASS="$(sed -n 's/^StartupWMClass=//p' "${APP_DESKTOP}")"
    [[ "${APP_CLASS}" =~ ^org\.amethystora\.[A-Z][a-z]+$ ]]
    grep -qaF "${APP_CLASS}" "${APPS_PROGRAM}"
done
test -s /usr/lib/amethystora-apps/fonts/QuicksandVariable.ttf
test -f /usr/share/licenses/amethystora-apps/Quicksand-OFL.txt
APPS_CRATES=/usr/share/licenses/amethystora-apps/crates/CRATES
grep -q $'^gtk4\t' "${APPS_CRATES}"
grep -q $'^libadwaita\t' "${APPS_CRATES}"
grep -v '^#' "${APPS_CRATES}" | while IFS=$'\t' read -r name version _; do
    compgen -G "/usr/share/licenses/amethystora-apps/crates/${name}-${version}/*" >/dev/null
done
cmp -s <(sed 1d /usr/share/icons/hicolor/scalable/apps/amethystora-logo.svg) <(sed 1d /ctx/apps/common/assets/gem.svg)
# The Chromium profiles the Electron windows left in each account's ~/.config go at its next login
test -x /usr/share/amethystora/user-setup.hooks.d/20-retire-electron-profiles.sh
# The file-watch sensor (plans/done/PLAN-audit-cost.md): a binary of its own, built by the apps stage; the
# keys it can cover are exactly the watcher's audit keys; its unit runs only while it is wanted; and its
# record writer gives the watcher the same sentences auditd would, which ausearch reads without any kernel.
test -x /usr/libexec/amethystora-sensor
test -f /usr/share/licenses/amethystora-sensor/COPYING
grep -q $'^libbpf-rs\t' "${APPS_CRATES}"
diff <(/usr/libexec/amethystora-sensor --keys | paste -sd'|' -) <(/usr/libexec/amethystora-security-watch --keys)
grep -q '^ExecCondition=/usr/libexec/amethystora-sensor --check$' /usr/lib/systemd/system/amethystora-sensor.service
# Nobody logs in before it is watching, and it sees the real /tmp and /var/tmp, which it watches programs run from
grep -q '^Type=notify$' /usr/lib/systemd/system/amethystora-sensor.service
grep -q '^Before=systemd-user-sessions\.service' /usr/lib/systemd/system/amethystora-sensor.service
[[ -z "$(grep '^PrivateTmp=' /usr/lib/systemd/system/amethystora-sensor.service)" ]]
systemd-analyze verify --man=no --recursive-errors=no /usr/lib/systemd/system/amethystora-sensor.service
bash /ctx/build_files/shared/test-sensor-records.sh
# The checks written in JavaScript run on the apps stage's Node, mounted at /apps-stage with its libraries,
# so that the image carries no Node of its own
NODE=(env LD_LIBRARY_PATH=/apps-stage/usr/lib64 NODE_ICU_DATA="$(echo /apps-stage/usr/share/node-*/icudata)" /apps-stage/usr/bin/node)

# The manual: every page in pages.json exists and every link between pages lands on a page and a heading
# that exist, as the pages are in the image and as they were written, before 07-debrand.sh renamed
# upstream wherever no copyright mark keeps it. It replaces upstream's Documentation launcher, and Super+F1
# opens it.
"${NODE[@]}" /ctx/build_files/shared/check-manual.js
"${NODE[@]}" /ctx/build_files/shared/check-manual.js /ctx/system_files/shared/usr/share/amethystora/manual
# The top bar's peek leaves the way up to a screen above open
"${NODE[@]}" /ctx/build_files/shared/test-bars-edge.mjs
test ! -e /usr/share/applications/documentation.desktop
grep -q "^command='amethystora-manual'$" /etc/dconf/db/distro.d/06-amethystora-keybindings
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.settings-daemon.plugins.media-keys help)" == "@as []" ]]

# Translations (apps/common/src/i18n.rs): every message the apps' code has is in its catalog's English, and
# every translation parses and names only the values its English names; the same for the shell programs'
# gettext catalogs (po/), compiled where gettext looks for them. Each launcher carries its translated name.
"${NODE[@]}" /ctx/build_files/shared/check-translations.js --po /ctx/po --apps /ctx/apps /
command -v gettext ngettext envsubst >/dev/null
test -f /usr/bin/gettext.sh
# The notifications are translated per account by the helper, and the recipes by the TEXTDOMAIN `ame` exports
grep -q '^source gettext.sh' /usr/libexec/amethystora-notify-users
grep -q '^export TEXTDOMAIN=amethystora$' /usr/bin/ame
# gettext reads LANGUAGE only outside the C locale, so the helper needs the account's own locale to be there
locale -a | grep -qix 'fr_FR.utf-\?8'
for po in /ctx/po/*.po; do
    [[ -e "${po}" ]] || continue
    test -s "/usr/share/locale/$(basename "${po}" .po)/LC_MESSAGES/amethystora.mo"
done
for catalog in /usr/lib/amethystora-*/resources/app/locale/*.json; do
    language="$(basename "${catalog}" .json)"
    [[ "${language}" == en ]] && continue
    app="$(basename "$(dirname "$(dirname "$(dirname "$(dirname "${catalog}")")")")")"
    name="$(sed -n 's/^Name=//p' "/usr/share/applications/${app}.desktop" | head -n1)"
    if [[ -n "$(jq -r --arg name "${name}" '.[$name] // ""' "${catalog}")" ]]; then
        grep -q "^Name\[${language//-/_}\]=" "/usr/share/applications/${app}.desktop"
    fi
done

# The Security app runs the commands it offers in a terminal, and draws its shield from the icon theme's
# own file (GPL-3.0, NOTICE) when it runs, rather than carrying a copy
test -x /usr/libexec/amethystora-in-terminal
test -s /usr/share/icons/hicolor/scalable/apps/amethystora-security-status.svg
# What each Flatpak can reach beyond its sandbox, for the report's check, the Apps page and `ame security
# apps`: one helper, run as the user, whose JSON the three read
test -x /usr/libexec/amethystora-app-permissions
grep -q 'amethystora-app-permissions --json' /usr/libexec/amethystora-security-status
# ...tried against a stand-in flatpak. An override that loads a library into an app, and lets it change the
# account's other apps (whose exports run outside every sandbox), is granted; a manifest that sets a plugin
# path asks for it; an empty variable or a read-only look at the installation reaches nothing.
PERMISSIONS_TEST="$(mktemp -d)"
cat >"${PERMISSIONS_TEST}/flatpak" <<'EOF'
#!/usr/bin/bash
app="${*: -1}"
case "$*" in
    "list --app --columns=application,installation,name") printf 'org.example.Loaded\tuser\tLoaded\norg.example.Plugins\tsystem\tPlugins\n' ;;
    *--show-permissions*org.example.Loaded) printf '[Context]\nfilesystems=xdg-data/flatpak;\n\n[Environment]\nLD_PRELOAD=/var/home/u/x.so\nPYTHONPATH=\n' ;;
    *--show-permissions*org.example.Plugins) printf '[Context]\nfilesystems=~/.local/share/flatpak:ro;\n\n[Environment]\nGST_PLUGIN_PATH=/app/lib/gst\n' ;;
    *--show-metadata*org.example.Plugins) printf '[Application]\nname=%s\n\n[Environment]\nGST_PLUGIN_PATH=/app/lib/gst\n' "${app}" ;;
    *--show-metadata*) printf '[Application]\nname=%s\n' "${app}" ;;
    "override --user --show org.example.Loaded") printf '[Context]\nfilesystems=xdg-data/flatpak;\n\n[Environment]\nLD_PRELOAD=/var/home/u/x.so\n' ;;
esac
EOF
chmod +x "${PERMISSIONS_TEST}/flatpak"
jq -e 'map({(.app): [.reaches[] | "\(.id) \(.granted)"]}) | add == {
    "org.example.Loaded": ["installations true", "code true"], "org.example.Plugins": ["code false"]}' \
    <<<"$(PATH="${PERMISSIONS_TEST}:${PATH}" /usr/libexec/amethystora-app-permissions --json)"
rm -rf "${PERMISSIONS_TEST}"

# The Updates app replaces upstream's System Update launcher, while `ame update` stays for the terminal
test ! -e /usr/share/applications/system-update.desktop
grep -rqx 'update ACTION="" VALUE="":' /usr/share/amethystora/just/
# After a restart (plans/ui-ideas/spec-restart.md): the open apps kept for the first login after an update's restart
# (the workspaces module's resume, asked for by the soft reboot, Updates' full restart and the night restart), the
# night restart's helper, saving data on a connection that counts it (each automatic updater's own condition, never
# Update now's), and a new network's zone (the privacy module's, which opens Security and switches nothing)
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.workspaces resume)" == true ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.pill saving-data)" == true ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.privacy zone-notice)" == true ]]
for file in /usr/libexec/amethystora-soft-reboot /usr/libexec/amethystora-night-restart /ctx/apps/update/src/backend.rs; do
    grep -q "org.amethystora.UI.Resume.Save" "${file}"
done
grep -q "'/org/amethystora/UI/Resume'" "${UI_EXTENSION}/modules/workspaces/resume.js"
grep -q "module.beforeDisable?.()" "${UI_EXTENSION}/extension.js"
test -x /usr/libexec/amethystora-night-restart
test -x /usr/libexec/amethystora-metered
NIGHT_HOME="$(mktemp -d)"
night() { XDG_CONFIG_HOME="${NIGHT_HOME}" XDG_STATE_HOME="${NIGHT_HOME}" /usr/libexec/amethystora-night-restart "$@"; }
night set 06:00 2>/dev/null && false
night set 3:00 2>/dev/null && false
night set 03:00
[[ "$(night get)" == 03:00 ]]
night set 05:00
[[ "$(night get)" == 05:00 ]]
night set off
[[ -z "$(night get)" ]]
rm -rf "${NIGHT_HOME}"
unset -f night
systemctl cat uupd.service | grep -qx "ExecCondition=/usr/libexec/amethystora-metered wait system"
systemctl cat uupd.service | grep -qx "ExecStartPost=/usr/libexec/amethystora-metered done system"
systemctl cat uupd-manual.service | grep -q amethystora-metered && false
grep -qx "ExecCondition=/usr/libexec/amethystora-metered wait containers" /usr/lib/systemd/user/amethystora-pkg-upgrade.service
grep -qx "ExecCondition=/usr/libexec/amethystora-metered wait backup" /usr/lib/systemd/user/amethystora-backup.service
# A run a person starts always runs, and nothing is metered here
env -u TRIGGER_UNIT /usr/libexec/amethystora-metered wait system
/usr/libexec/amethystora-metered metered && false
for recipe in resume saving-data zone-notice; do
    grep -q "^${recipe} VALUE=\"\":" /usr/share/amethystora/just/desktop.just
done
[[ -z "$(grep -nE 'use_markup = true|use_markup: true|set_markup\(|useBodyMarkup' "${UI_EXTENSION}/modules/privacy/zone.js" \
    "${UI_EXTENSION}/modules/pill/saving.js" "${UI_EXTENSION}/modules/pill/resume.js")" ]]
# The zone notice only reads: nothing in it changes a connection or the firewall
if grep -nE "changeZone|addInterface|removeInterface|Update2|AddAndActivate|ame security" "${UI_EXTENSION}/modules/privacy/zone.js"; then
    echo "the zone notice switches something"
    exit 1
fi
# "Update now" starts uupd's manual unit, which the rule the uupd package ships lets anyone start
# without a password. Without the rule it would still start, after asking for one. The ublue-os/packages
# build this image installs puts the rule in /etc, uupd's own spec in /usr/share: either will do.
test -f /usr/lib/systemd/system/uupd-manual.service
grep -qs '"uupd-manual.service"' /etc/polkit-1/rules.d/uupd.rules /usr/share/polkit-1/rules.d/uupd.rules
# ...for anyone, and to every other verb too (stop, kill, set-property): the image's own rule is read first
# and leaves the person at the machine starting an update, and everything else to an administrator
UUPD_RULE=/usr/share/polkit-1/rules.d/40-amethystora-uupd.rules
grep -q 'action.lookup("verb") == "start" && subject.local && subject.active' "${UUPD_RULE}"
grep -q "polkit.Result.AUTH_ADMIN" "${UUPD_RULE}"
[[ "$(printf '%s\n' "${UUPD_RULE##*/}" uupd.rules | LC_ALL=C sort | head -n1)" == "${UUPD_RULE##*/}" ]]
# Its Firmware section asks fwupd, which only knows what LVFS has once fwupd-refresh.timer has fetched it:
# Fedora leaves that to GNOME Software, which this image does not have
command -v fwupdmgr >/dev/null
[[ "$(systemctl is-enabled fwupd-refresh.timer 2>/dev/null)" == "enabled" ]]
# An update that leaves the kernel, the initramfs and the kernel arguments alone finishes with a soft reboot
# (amethystora-soft-reboot, Restart now and `ame update restart`). bootc says when, and only does when two
# builds of the same packages make the same initramfs (19-initramfs.sh): nothing of the build's version in its
# os-release, and no out-of-tree driver but NVIDIA's, which akmods builds anew for every image...
test -x /usr/libexec/amethystora-soft-reboot
bash -n /usr/libexec/amethystora-soft-reboot
test -f /usr/lib/systemd/system/soft-reboot.target
ame --show update >/dev/null
INITRD_RELEASE="$(lsinitrd -f usr/lib/initrd-release /lib/modules/*/initramfs.img)"
grep -q '^ID=amethystora' <<<"${INITRD_RELEASE}"
grep -E '^(OSTREE_VERSION|IMAGE_VERSION|BUILD_ID)=' <<<"${INITRD_RELEASE}" && false
grep -q "^VERSION=\"$(. /usr/lib/os-release && echo "${VERSION_ID}") dracut-" <<<"${INITRD_RELEASE}"
grep -E 'lib/modules/[^/]+/(extra|updates)/.*\.ko' <<<"${INITRAMFS_FILES}" | grep -v '/nvidia' && false
# The image's kernel settings are the system's to apply, where ame security kernel off can mask them
grep -q 'usr/lib/sysctl.d/60-amethystora-hardening.conf' <<<"${INITRAMFS_FILES}" && false
# ...which the system still has, os-release, modules-load.d and its kernel settings as they were
test -f /usr/lib/sysctl.d/60-amethystora-hardening.conf
cmp /etc/os-release /usr/lib/os-release
if compgen -G "/usr/lib/modules/*/extra/v4l2loopback" >/dev/null; then grep -rqsx v4l2loopback /usr/lib/modules-load.d; fi
# Its status is anyone's to ask, and the update is mounted for the restart only by the person at the machine
SOFT_RULE=/usr/share/polkit-1/rules.d/50-amethystora-soft-reboot.rules
grep -q 'command == "/usr/libexec/amethystora-soft-reboot status") {' "${SOFT_RULE}"
grep -q 'reset") && subject.local && subject.active' "${SOFT_RULE}"

# The Notes app asks a security key for a passkey through libfido2's tools (fido2-tools). Its sealing is the
# program's own, AES-256-GCM under a key from scrypt, which the apps stage's tests hold to the vaults the
# Electron window wrote.
for tool in fido2-token fido2-cred fido2-assert; do
    command -v "${tool}"
done
test -s /usr/share/icons/hicolor/scalable/apps/amethystora-notes.svg
# It takes the place of Joplin and Planify, so no Flatpak list the image carries may bring them back
grep -rqsE '"(net\.cozic\.joplin_desktop|io\.github\.alainm23\.planify)"' /usr/share/amethystora/homebrew /usr/share/flatpak/preinstall.d && false

# The Logs app replaces GNOME Logs, which the upstream Flatpak list installed. The options it hands
# journalctl have to be ones this journalctl knows.
grep -rqF "org.gnome.Logs" /usr/share/flatpak/preinstall.d && false
# ...and a machine that already had it loses it, once, by 21-retire-gnome-logs.sh
grep -q "flatpak uninstall --system .*org.gnome.Logs" /usr/share/amethystora/system-setup.hooks.d/21-retire-gnome-logs.sh
JOURNALCTL_HELP="$(journalctl --help --no-pager)"
for option in --case-sensitive --output-fields --after-cursor --identifier --user-unit; do
    grep -q -- "${option}" <<<"${JOURNALCTL_HELP}"
done

# The Control app, on Super+Ctrl+Shift+C
test -s /usr/share/icons/hicolor/scalable/places/amethystora-control.svg
grep -q "^command='amethystora-control'$" /etc/dconf/db/distro.d/06-amethystora-keybindings
# What it shows is one list, which `ame control` prints too. Nothing is set up on the build machine and no
# session runs there, so entries may be unavailable, but the list parses, its ids are its own, each kind
# is one the window draws, and every command is an `ame` command outside `ame security`. Every manual
# link lands, and a pool of disks is only ever changed in a terminal.
test -x /usr/libexec/amethystora-control-list
# The list resolves its messages with bash 5.3's ${ cmd; } (no process per message), which bash 5.2 cannot parse
[[ "$(bash -c 'echo $((BASH_VERSINFO[0] * 100 + BASH_VERSINFO[1]))')" -ge 503 ]]
bash -n /usr/libexec/amethystora-control-list
ame --show control >/dev/null
CONTROL_LIST="$(GSETTINGS_BACKEND=memory /usr/libexec/amethystora-control-list --json)"
jq -e '.items | length > 20' <<<"${CONTROL_LIST}" >/dev/null
# Quick settings' two entries, each an `ame desktop quick-settings` command that needs no terminal
jq -e '[.items[] | select(.id == "quick-theme" or .id == "quick-tiles")] | length == 2' <<<"${CONTROL_LIST}" >/dev/null
[[ -z "$(jq -c '.items[] | select(.id == "quick-theme" or .id == "quick-tiles") | (.command // empty),
    (.choices // [] | .[].command) | select(.[0:3] != ["ame", "desktop", "quick-settings"])' <<<"${CONTROL_LIST}")" ]]
[[ -z "$(jq -r '.items[].id' <<<"${CONTROL_LIST}" | sort | uniq -d)" ]]
[[ -z "$(jq -r '.items[].kind' <<<"${CONTROL_LIST}" |
    grep -vxE 'switch|choice|range|action|link|themes|theme-maker|wallpapers|widgets|webapps|shortcuts|keybindings|trust-image|setup|disks|rows|bar|graph')" ]]
[[ -z "$(jq -c '.items[] | (.command // empty), (.choices // [] | .[].command), (.commands // {} | .[].command),
    (.disks // [] | .[].actions[].command), (.rows // [] | .[].actions[].command)
    | select(.[0] != "ame" or .[1] == "security")' <<<"${CONTROL_LIST}")" ]]
# The text shortcuts' entries: the switch, and the list, which names abbreviations and line counts and never a text,
# and whose commands take the abbreviation alone as an argument (a text goes on stdin, out of the process list)
jq -e '[.items[] | select(.id == "text-shortcuts" and .kind == "switch")] | length == 1' <<<"${CONTROL_LIST}" >/dev/null
jq -e '.items[] | select(.id == "text-shortcut-list") | .kind == "shortcuts" and
    all(.shortcuts[]; keys == ["abbr", "lines"]) and (.commands | keys == ["add", "remove", "show"]) and
    all(.commands[]; .command[-1] == "{abbr}" and (.command | map(select(test("[{]"))) | length) == 1)' <<<"${CONTROL_LIST}" >/dev/null
[[ -z "$(jq -c '.items[] | select(.kind == "disks") | .disks[].actions[]
    | select(.command[0:3] != ["ame", "system", "raid"] or .terminal != true)' <<<"${CONTROL_LIST}")" ]]
# The same for the Disks group's other cards, of which making a pool is always there
jq -e 'any(.items[]; .id == "raid-create")' <<<"${CONTROL_LIST}" >/dev/null
[[ -z "$(jq -c '.items[] | select(.id | startswith("raid-")) | ., (.choices // [])[] | select(.command != null)
    | select(.command[0:3] != ["ame", "system", "raid"] or .terminal != true)' <<<"${CONTROL_LIST}")" ]]
mapfile -t CONTROL_MANUAL < <(jq -r '.items[].manual // empty' <<<"${CONTROL_LIST}" | sort -u)
"${NODE[@]}" /ctx/build_files/shared/check-manual.js /usr/share/amethystora/manual --links "${CONTROL_MANUAL[@]}"
# The window runs nothing of its own: no shell, no password prompt, nothing of Security's (the commands it
# shows to copy are only text). The repository it lets you trust is checked with the very expression
# amethystora-trust-image checks it with.
grep -rhvE '^[[:space:]]*//' /ctx/apps/control/src /ctx/apps/control-core/src |
    grep -qE '"(sudo|pkexec|sh|bash)"|"-c"|ame security|amethystora-security-config' && false
TRUST_RULE="$(sed -n 's/^\[\[ "\${REPOSITORY}" =~ \(.*\) \]\] ||$/\1/p' /usr/libexec/amethystora-trust-image)"
[[ -n "${TRUST_RULE}" ]]
grep -qF "r\"${TRUST_RULE}\"" /ctx/apps/control-core/src/lib.rs
# The helpers it reads: the images and streams the rebase helper offers, the widgets and the web apps
jq -e '(.images | length) == 6 and (.streams | length) > 0' <<<"$(/usr/bin/amethystora-rollback-helper --choices)" >/dev/null
CONTROL_HOME="$(mktemp -d)"
jq -e '(.bar | type) == "boolean" and .widgets == []' <<<"$(HOME="${CONTROL_HOME}" amethystora-widgets list --json)" >/dev/null
jq -e '. == []' <<<"$(HOME="${CONTROL_HOME}" amethystora-webapp list --json)" >/dev/null
# A switch says what it wants: on or off sets the one thing it names, and running it twice changes nothing
# more. Without an argument it still flips, as it did before Control. One command a line, so that set -e
# stops at the one that fails.
(
    export HOME="${CONTROL_HOME}" XDG_CONFIG_HOME="${CONTROL_HOME}/.config" SHELL=/usr/bin/bash
    FLAGS="${XDG_CONFIG_HOME}/amethystora"
    ame desktop greeting off
    ame desktop greeting off
    test -e "${FLAGS}/no-greeting"
    ame desktop greeting on
    ame desktop greeting on
    test ! -e "${FLAGS}/no-greeting"
    ame desktop greeting
    test -e "${FLAGS}/no-greeting"
    touch "${FLAGS}/aur"
    ame apps aur off
    ame apps aur off
    test ! -e "${FLAGS}/aur"
    # The agentic features are off until the account turns them on
    amethystora-agent status >/dev/null && false
    ame agent toggle on
    ame agent toggle on
    test -e "${FLAGS}/agentic"
    amethystora-agent status >/dev/null
    ame agent toggle off
    ame agent toggle off
    test ! -e "${FLAGS}/agentic"
    touch "${HOME}/.bashrc"
    ame desktop bling on
    ame desktop bling on
    [[ "$(grep -c 'bling.sh source start' "${HOME}/.bashrc")" == 1 ]]
    [[ "$(amethystora-bling status)" == on ]]
    ame desktop bling off
    ame desktop bling off
    grep -q bling "${HOME}/.bashrc" && false
    [[ "$(amethystora-bling status)" == off ]]
)
rm -rf "${CONTROL_HOME}"

# The Backups app
test -s /usr/share/icons/hicolor/scalable/places/amethystora-backups.svg
# It offers itself under Open With on a folder in Files, and Files stays what opens one
grep -qx 'MimeType=inode/directory;' /usr/share/applications/amethystora-backups.desktop
grep -qx 'inode/directory=org.gnome.Nautilus.desktop' /etc/xdg/mimeapps.list
# What it shows is one helper's, which finds nothing set up on the build machine and still answers. The
# window runs no shell, asks for no password and switches nothing of Security's: it only opens Security.
test -x /usr/libexec/amethystora-restore
jq -e '.version == 1 and .backup.set_up == false and (.snapshots.on | type) == "boolean"' \
    <<<"$(/usr/libexec/amethystora-restore status --json)" >/dev/null
jq -e '.points == [] and .backup == "none"' <<<"$(/usr/libexec/amethystora-restore points --json)" >/dev/null
grep -rhvE '^[[:space:]]*//' /ctx/apps/backups/src |
    grep -qE '"(sudo|pkexec|sh|bash)"|"-c"|ame security|amethystora-security-config' && false

# OpenTabletDriver's udev rules come from a pinned release whose tarball is checked before anything goes
# into /etc, and its daemon from the image's own user unit, which no account gets without asking
OTD_RECIPE="$(ame --show apps::opentabletdriver)"
grep -q 'OTD_SHA256="[0-9a-f]\{64\}"' <<<"${OTD_RECIPE}"
grep -q 'sha256sum' <<<"${OTD_RECIPE}"
grep -qE 'releases/latest|raw\.githubusercontent\.com' <<<"${OTD_RECIPE}" && false
test -f /usr/lib/systemd/user/amethystora-opentabletdriver.service
[[ -z "$(find /etc/systemd/user -name amethystora-opentabletdriver.service)" ]]
# Gathering a bug report is `ame report`, which keeps it on the machine: the old name posts nothing
DEVICE_INFO="$(ame --show system::device-info)"
[[ "${DEVICE_INFO}" == *"ame report"* ]]
grep -qiE 'pastebin|fpaste' <<<"${DEVICE_INFO}" && false

test -f /usr/lib/amethystora/theme/lib.sh
test -f /usr/share/amethystora/keybindings.md
test -x /usr/share/amethystora/theme-set.hooks.d/10-vscodium.sh
test -x /usr/share/amethystora/user-setup.hooks.d/11-theme.sh
for template in btop.theme kitty-theme.conf ptyxis.palette starship.toml wallpaper.svg wallpaper-contours.svg wallpaper-ridges.svg wallpaper-waves.svg; do
    test -f "/usr/share/amethystora/themed/${template}.tpl"
done
# A wallpaper template draws with colours from the palette, and one a theme lacks would be left in
# the SVG as "{{ key }}", which librsvg cannot draw
for key in $(grep -oh '{{ [a-z0-9_]* }}' /usr/share/amethystora/themed/wallpaper*.svg.tpl | tr -d '{} ' | sort -u); do
    for theme in /usr/share/amethystora/themes/*/; do
        grep -q "^${key} = \"#" "${theme}colors.toml"
    done
done
# A theme is data shared as dotfiles: its colours are put into the templates as text, never as a program,
# and one that is not a colour refuses the theme. Every shipped theme renders completely.
grep -q "sed -f" /usr/lib/amethystora/theme/lib.sh && false
bash /ctx/build_files/shared/test-theme-render.sh
# ...and every colour the interface takes from a theme (the palette layer, themed/palette-gtk*.css.tpl, the shell's
# status colours and the segments of the top bar) reads at the contrast WCAG asks, on every surface it is drawn on, with
# each of the twelve accents and with the colours of every shipped wallpaper in each of their three ways
bash /ctx/build_files/shared/test-theme-palette.sh
# Make it yours (rounds 5 and 6, plans/ui-ideas/spec-look.md). Themes into files and out of them: colour files from
# elsewhere read for their colours only, a palette from the theme maker, look files read whole or refused whole
test -x /usr/libexec/amethystora-theme-file
bash /ctx/build_files/shared/test-theme-file.sh
# The theme maker's three checks are palette.awk's (checks=1), which apps/control-core/src/contrast.rs is held to
[[ "$(bash -c 'source /usr/lib/amethystora/theme/lib.sh; palette_checks /usr/share/amethystora/themes/nord' | cut -d' ' -f1 | tr '\n' ' ')" == "text label outline " ]]
# Every one of the seven drawn wallpapers is a whole SVG in every shipped theme's colours, and in the first's at each time
# of day
PATTERN_LIST="$(bash -c 'source /usr/lib/amethystora/theme/lib.sh; echo "${PATTERNS[*]}"')"
[[ "${PATTERN_LIST}" == "facets waves contour mesh grain dots bands" ]]
for theme_dir in /usr/share/amethystora/themes/*/; do
    for pattern in ${PATTERN_LIST}; do
        tones=(night)
        [[ "${theme_dir}" == */amethystora/ ]] && tones=(dawn day dusk night)
        for tone in "${tones[@]}"; do
            bash -c 'source /usr/lib/amethystora/theme/lib.sh; pattern_input "$1" "$2" 7 "$3"' _ "${theme_dir%/}" "${pattern}" "${tone}" |
                awk -f /usr/lib/amethystora/theme/pattern.awk >/tmp/pattern.svg
            python3 -I -c 'import sys, xml.etree.ElementTree as ET; r = ET.parse(sys.argv[1]).getroot(); assert r.tag.endswith("svg") and len(r) > 1' /tmp/pattern.svg ||
                { echo "${pattern} in ${theme_dir} at ${tone} is not a whole SVG"; exit 1; }
        done
    done
done
rm -f /tmp/pattern.svg
# A look shared is a type of its own, opened by its handler (which says while opening looks is off), and a colour file
# can be imported from Open With, never by default
grep -q '^application/x-amethystora-look' /usr/share/mime/globs2 2>/dev/null || grep -q 'amethystora-look' /usr/share/mime/globs
grep -qx "application/x-amethystora-look=amethystora-look.desktop" /etc/xdg/mimeapps.list
grep -qx "Exec=/usr/libexec/amethystora-in-terminal /usr/bin/ame desktop theme look %f" /usr/share/applications/amethystora-look.desktop
grep -qx "Exec=/usr/libexec/amethystora-in-terminal /usr/bin/ame desktop theme import %f" /usr/share/applications/amethystora-theme-import.desktop
if grep -q "=amethystora-theme-import.desktop" /etc/xdg/mimeapps.list; then echo "a colour file opens as a theme by default"; exit 1; fi
# Light by day, dark by night: the theme module's keys, off as the image ships; only the two pairs it names have a pair
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.theme day-night)" == false ]]
[[ "$(cat /usr/share/amethystora/themes/*/pair.theme | sort | tr '\n' ' ')" == "amethystora amethystora-light catppuccin catppuccin-latte " ]]
# The twelve accents are one list: the theme library's ACCENTS, which Control offers, the recipe's usage names, the setup
# file takes, the shell's workspaces' menu and the apps' palette know
ACCENT_IDS="$(bash -c 'source /usr/lib/amethystora/theme/lib.sh; for e in "${ACCENTS[@]}"; do echo "${e%% *}"; done')"
[[ "$(wc -l <<<"${ACCENT_IDS}")" == 12 ]]
[[ "$(bash -c 'source /usr/lib/amethystora/theme/lib.sh; printf "%s\n" "${ACCENTS[@]}"')" == "$(sed -nE "s/^ *\['([a-z]+)', '(#[0-9a-f]{6})'\],$/\1 \2/p" \
    "${UI_EXTENSION}/modules/theme/accents.js")" ]]
[[ "$(jq -r '.items[] | select(.id == "accent") | .choices[].value' <<<"${CONTROL_LIST}")" == theme$'\n'"${ACCENT_IDS}" ]]
[[ -z "$(jq -c '.items[] | select(.id == "accent") | .choices[] | select((.color | test("^#[0-9a-f]{6}$") | not)
    or .command != ["ame", "desktop", "accent", .value])' <<<"${CONTROL_LIST}")" ]]
grep -qF "usage: ame desktop accent $(tr '\n' '|' <<<"${ACCENT_IDS}")theme\"" /usr/share/amethystora/just/desktop.just
[[ "$(sed -n "s/^LOOK_ACCENTS='\(.*\)'$/\1/p" /usr/libexec/amethystora-setup | jq -r '.[]')" == "${ACCENT_IDS}"$'\n'theme ]]
[[ "$(tr -d ' \n' <<<"$(sed -n '/^const ACCENTS: &\[&str\] =$/,/;$/p' /ctx/apps/common/src/palette.rs | tail -n +2)")" == \
    "&[$(sed 's/.*/"&"/' <<<"${ACCENT_IDS}" | paste -sd,)];" ]]
# Colours from the wallpaper, Daylight, the accent and an accent per workspace: each its recipe, and Control's entries
for recipe in accent daylight colours accent-per-workspace; do
    grep -qx "desktop::${recipe}" <<<"$(ame --summary | tr ' ' '\n')"
    jq -e --arg id "${recipe}" 'any(.items[]; .id == $id)' <<<"${CONTROL_LIST}" >/dev/null
done
# ...the theme module's keys and their defaults (an accent per workspace off, and the six workspaces' accents)
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.theme accent-per-workspace)" == false ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.theme workspace-accents)" == \
    "$(bash -c 'source /usr/lib/amethystora/theme/lib.sh; printf "%s\n" "${WORKSPACE_ACCENTS[@]}"' | sed "s/.*/'&'/" | paste -sd, | sed 's/,/, /g; s/^/[/; s/$/]/')" ]]
# Daylight: every theme has its four pictures, dawn, day, dusk and night in that order, each a wallpaper drawn at the
# size of the others and named in CREDITS.md, and the slideshow made of them is GNOME's format: nine slides of whole
# seconds, a day in all, every file there. No picture has an alpha channel: Mutter 50.5 blends two such pictures without
# the first one, so a fade would start near black. A theme's day is on its own side of the line between dark and light
# themes, and its night under it: the dark Amethystora theme once took the light one's near-white day. A dark theme's night
# stays under 0.5. A light theme's dusk stays at 0.45 or over and its night under 0.6: Amethystora Light's night is pale
# moonlit stone at 0.56, and it once took the dark theme's dusk (0.40) and night (0.34), which its own have to stay above.
# Every picture is one theme's and one time's, by name and by content
daylight_lightness() {
    python3 -I -c 'import sys, gi
gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf
p = GdkPixbuf.Pixbuf.new_from_file_at_scale(sys.argv[1], 64, 64, False)
d, n, r, t = p.get_pixels(), p.get_n_channels(), p.get_rowstride(), 0
lin = lambda v: v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4
for y in range(64):
    for x in range(64):
        R, G, B = (lin(d[y * r + x * n + c] / 255) for c in range(3))
        t += 0.2104542553 * (0.4122214708 * R + 0.5363325363 * G + 0.0514459929 * B) ** (1 / 3)
        t += 0.793617785 * (0.2119034982 * R + 0.6806995451 * G + 0.1073969566 * B) ** (1 / 3)
        t -= 0.0040720468 * (0.0883024619 * R + 0.2817188376 * G + 0.6299787005 * B) ** (1 / 3)
print("%.4f" % (t / 4096))' "$1"
}
DAYLIGHT_FILES="$(cut -d' ' -f2- /usr/share/amethystora/themes/*/daylight.list)"
[[ "$(wc -l <<<"${DAYLIGHT_FILES}")" == "$((4 * $(find /usr/share/amethystora/themes -name daylight.list | wc -l)))" ]]
[[ -z "$(sort <<<"${DAYLIGHT_FILES}" | uniq -d)" ]]
[[ -z "$(xargs -d '\n' sha256sum <<<"${DAYLIGHT_FILES}" | cut -d' ' -f1 | sort | uniq -d)" ]]
for time in dusk night; do
    awk -v l="$(daylight_lightness "$(awk -v t="${time}" '$1 == t { print $2 }' /usr/share/amethystora/themes/amethystora-light/daylight.list)")" \
        -v d="$(daylight_lightness "$(awk -v t="${time}" '$1 == t { print $2 }' /usr/share/amethystora/themes/amethystora/daylight.list)")" \
        'BEGIN { exit !(l >= d + 0.1) }'
done
for theme in /usr/share/amethystora/themes/*/; do
    DAYLIGHT_DAY="$(daylight_lightness "$(awk '$1 == "day" { print $2 }' "${theme}daylight.list")")"
    DAYLIGHT_NIGHT="$(daylight_lightness "$(awk '$1 == "night" { print $2 }' "${theme}daylight.list")")"
    if [[ -f "${theme}light.mode" ]]; then
        DAYLIGHT_DUSK="$(daylight_lightness "$(awk '$1 == "dusk" { print $2 }' "${theme}daylight.list")")"
        awk -v d="${DAYLIGHT_DAY}" -v k="${DAYLIGHT_DUSK}" -v n="${DAYLIGHT_NIGHT}" 'BEGIN { exit !(d >= 0.77 && k >= 0.45 && n < 0.6) }'
    else
        awk -v d="${DAYLIGHT_DAY}" -v n="${DAYLIGHT_NIGHT}" 'BEGIN { exit !(d < 0.77 && n < 0.5) }'
    fi
    [[ "$(cut -d' ' -f1 "${theme}daylight.list" | paste -sd' ')" == "dawn day dusk night" ]]
    while read -r _ picture; do
        [[ "$(file -b "${picture}")" == "PNG image data, 3840 x 2160, 8-bit/color RGB,"* ]]
        grep -qF "\`${picture#/usr/share/backgrounds/amethystora/}\`" /usr/share/backgrounds/amethystora/CREDITS.md
    done <"${theme}daylight.list"
    DAYLIGHT_XML="$(mktemp)"
    (
        export GSETTINGS_BACKEND=memory
        source /usr/lib/amethystora/theme/lib.sh
        daylight_pictures "${theme}" | daylight_xml 2026 10 27
    ) >"${DAYLIGHT_XML}"
    xmllint --noout "${DAYLIGHT_XML}"
    [[ "$(grep -c '<static>\|<transition ' "${DAYLIGHT_XML}")" == 9 ]]
    [[ "$(grep -o '<duration>[0-9]*</duration>' "${DAYLIGHT_XML}" | tr -dc '0-9\n' | awk '{ s += $1 } END { print s }')" == 86400 ]]
    grep -o '<\(file\|from\|to\)>[^<]*<' "${DAYLIGHT_XML}" | sed 's/^<[a-z]*>//; s/<$//' | xargs -r -n1 test -f
    rm -f "${DAYLIGHT_XML}"
done
# ...a theme of the account's own that names a picture with & < > in its path gets them written as text, not markup
DAYLIGHT_ODD="$(mktemp -d)"
mkdir -p "${DAYLIGHT_ODD}/odd"
for time in dawn day dusk night; do
    cp /usr/share/backgrounds/amethystora/amethystora-facets-d.png "${DAYLIGHT_ODD}/a&b<${time}>.png"
    echo "${time} ${DAYLIGHT_ODD}/a&b<${time}>.png" >>"${DAYLIGHT_ODD}/odd/daylight.list"
done
(
    export GSETTINGS_BACKEND=memory
    source /usr/lib/amethystora/theme/lib.sh
    daylight_pictures "${DAYLIGHT_ODD}/odd" | daylight_xml 2026 10 27
) >"${DAYLIGHT_ODD}/odd.xml"
xmllint --noout "${DAYLIGHT_ODD}/odd.xml"
[[ "$(xmllint --xpath 'string(/background/static[1]/file)' "${DAYLIGHT_ODD}/odd.xml")" == "${DAYLIGHT_ODD}/a&b<night>.png" ]]
rm -rf "${DAYLIGHT_ODD}"
# An accent per workspace: each accent's overlay holds only the rules the accent changes, never a font (a theme's own
# monospace font is the same for every accent), every selector under the accent's class
ACCENT_RENDER="$(mktemp -d)"
for theme in amethystora catppuccin nord; do
    (
        export HOME="${ACCENT_RENDER}/home" XDG_CONFIG_HOME="${ACCENT_RENDER}/home/.config" AMETHYSTORA_ACCENTS=on
        source /usr/lib/amethystora/theme/lib.sh
        mkdir -p "${ACCENT_RENDER}/${theme}"
        cp -r "/usr/share/amethystora/themes/${theme}/." "${ACCENT_RENDER}/${theme}/"
        render_templates "${ACCENT_RENDER}/${theme}" "theme_name=${theme}" 2>/dev/null
        render_accent_sets "${ACCENT_RENDER}/${theme}" "${theme}"
    )
    for overlay in "${ACCENT_RENDER}/${theme}"/accents/*/shell-accent.css; do
        test -s "${overlay}"
        grep -q 'font-family' "${overlay}" && { echo "${overlay} carries a font"; false; }
        id="$(basename "$(dirname "${overlay}")")"
        [[ -z "$(grep -E '\{$|,$' "${overlay}" | grep -vE "^(#uiGroup\.amethystora-accent-${id}|\.amethystora-accent-${id} )")" ]]
    done
done
rm -rf "${ACCENT_RENDER}"
# The helper that works colours out of a wallpaper reads a picture and refuses anything else, root included
WALLPAPER_COLOURS_HOME="$(mktemp -d)"
chown nobody "${WALLPAPER_COLOURS_HOME}"
[[ "$(setpriv --reuid=nobody --regid=nobody --clear-groups env -i HOME="${WALLPAPER_COLOURS_HOME}" PATH=/usr/bin:/usr/sbin /usr/libexec/amethystora-wallpaper-colours \
    --way soft --mode dark --background '#110c18' --foreground '#e8e0f2' --print \
    /usr/share/backgrounds/amethystora/amethystora-facets-d.png 2>/dev/null | grep -cE '^(background|foreground|accent|selection_background) = "#[0-9a-f]{6}"$')" == 4 ]]
echo "not a picture" >"${WALLPAPER_COLOURS_HOME}/x.png"
chmod 0644 "${WALLPAPER_COLOURS_HOME}/x.png"
code=0
setpriv --reuid=nobody --regid=nobody --clear-groups env -i HOME="${WALLPAPER_COLOURS_HOME}" PATH=/usr/bin:/usr/sbin /usr/libexec/amethystora-wallpaper-colours \
    --way soft --mode dark --background '#110c18' --foreground '#e8e0f2' --print "${WALLPAPER_COLOURS_HOME}/x.png" 2>/dev/null || code=$?
[[ "${code}" == 3 ]]
code=0
/usr/libexec/amethystora-wallpaper-colours --way soft --mode dark --background '#110c18' --foreground '#e8e0f2' --print \
    /usr/share/backgrounds/amethystora/amethystora-facets-d.png >/dev/null 2>&1 || code=$?
[[ "${code}" == 2 ]]
rm -rf "${WALLPAPER_COLOURS_HOME}"
# ...and the hooks that write into other programs' settings (kdeglobals, VSCodium) keep the account's own, take only what a theme
# may give, and are the same on a run again
bash /ctx/build_files/shared/test-theme-hooks.sh
# Every theme needs a palette; the default one is what 11-theme.sh applies at first login
test -f /usr/share/amethystora/themes/amethystora/colors.toml
for theme in /usr/share/amethystora/themes/*/; do
    test -f "${theme}colors.toml"
    grep -q "^background = \"#" "${theme}colors.toml"
    grep -q "^color15 = \"#" "${theme}colors.toml"
    # accent.theme has to name an accent GNOME knows, or setting it silently does nothing
    if [[ -f "${theme}accent.theme" ]]; then
        grep -qE "^(blue|teal|green|yellow|orange|red|pink|purple|slate)$" "${theme}accent.theme"
    fi
    # pair.theme has to name a theme that exists, or Super+Ctrl+D dead-ends
    if [[ -f "${theme}pair.theme" ]]; then
        test -d "/usr/share/amethystora/themes/$(cat "${theme}pair.theme")"
    fi
done

# GTK theme (11-gtk-theme.sh): Sweet, rotated onto the amethyst palette and installed under
# Amethystora's own name, because a theme whose colours have been replaced is no longer Sweet
GTK_THEME_DIR=/usr/share/themes/Amethystora
test -f /usr/share/licenses/amethystora-gtk-theme/LICENSE
grep -qx "Name=Amethystora" "${GTK_THEME_DIR}/index.theme"
for stylesheet in gtk-3.0/gtk.css gtk-3.0/gtk-dark.css gtk-4.0/gtk.css gtk-4.0/gtk-dark.css; do
    test -s "${GTK_THEME_DIR}/${stylesheet}"
done
# The stylesheets reach their bitmaps through ../assets, so the two have to stay siblings
test -s "${GTK_THEME_DIR}/assets/checkbox-checked-dark.png"
grep -q '\.\./assets/' "${GTK_THEME_DIR}/gtk-4.0/gtk-dark.css"
# Nothing Sweet coloured may be left: its teal accent by name, and anything else the rotation
# should have moved and did not
grep -qi "00d3a7" "${GTK_THEME_DIR}/gtk-4.0/gtk-dark.css" && false
python3 /ctx/build_files/shared/recolor.py --check "${GTK_THEME_DIR}"/gtk-[34].0/*.css
# The filled half of a switch is the one accent Sweet draws in the warm colours the default band
# spares, and it gets a pass of its own. Amber left here would be the only thing in the theme still
# wearing Sweet's colours, next to a purple everything else.
for switch in switch-on switch-on-insensitive; do
    grep -qiE "ff9200|fadd00" "${GTK_THEME_DIR}/assets/${switch}.svg" && false
done
python3 /ctx/build_files/shared/recolor.py --band 30:56 --check \
    "${GTK_THEME_DIR}"/assets/switch-on.svg "${GTK_THEME_DIR}"/assets/switch-on-insensitive.svg
# Only the two Amethystora palettes ask for it. The rest stay on adw-gtk3, which follows the GNOME
# accent colour, where this one would hold its purple against whatever palette they set.
for theme in amethystora amethystora-light; do
    grep -qx "Amethystora" "/usr/share/amethystora/themes/${theme}/gtk.theme"
done
[[ -z "$(find /usr/share/amethystora/themes -name gtk.theme ! -path '*/amethystora/*' ! -path '*/amethystora-light/*')" ]]
# libadwaita ignores gtk-theme, and a Flatpak cannot see /usr/share/themes at all, so both are
# reached by the theme-set hook: it mirrors the theme into ~/.themes and writes the user stylesheet
# that imports from the mirror. Without it the theme stops at the native GTK3 applications.
THEME_HOOK=/usr/share/amethystora/theme-set.hooks.d/20-gtk-apps.sh
test -x "${THEME_HOOK}"
grep -q 'MIRROR_ROOT="${HOME}/.themes"' "${THEME_HOOK}"
grep -q "gtk-4.0" "${THEME_HOOK}"
# The mirror and the stylesheet are only any use to a Flatpak if the sandbox is let at them
grep -q "^filesystems=~/\.themes:ro;xdg-config/gtk-4\.0:ro;xdg-config/gtk-3\.0/gtk\.css:ro;$" /etc/flatpak/overrides/global
# Window buttons on the right, close last, drawn as the lights of themed/window-controls-gtk*.css.tpl
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.desktop.wm.preferences button-layout)" == "':minimize,maximize,close'" ]]
# ...which every shipped theme renders whole, through the theme switcher's own render_templates: a colour its
# palette lacks would leave a {{ key }} in the CSS
RENDER_HOME="$(mktemp -d)"
for theme in /usr/share/amethystora/themes/*/; do
    mkdir -p "${RENDER_HOME}/$(basename "${theme}")"
    cp "${theme}colors.toml" "${RENDER_HOME}/$(basename "${theme}")/"
    [[ -f "${theme}light.mode" ]] && cp "${theme}light.mode" "${RENDER_HOME}/$(basename "${theme}")/"
    (
        HOME="${RENDER_HOME}" XDG_CONFIG_HOME="${RENDER_HOME}/config"
        source /usr/lib/amethystora/theme/lib.sh
        render_templates "${RENDER_HOME}/$(basename "${theme}")" "theme_name=$(basename "${theme}")"
    )
    for rendered in "${RENDER_HOME}/$(basename "${theme}")"/window-controls-gtk{3,4}.css \
        "${RENDER_HOME}/$(basename "${theme}")"/palette-gtk{3,4}.css \
        "${RENDER_HOME}/$(basename "${theme}")"/style-*.gtk.css; do
        test -s "${rendered}"
        if grep -q '{{' "${rendered}"; then
            echo "${theme} leaves a value of $(basename "${rendered}") unrendered"
            exit 1
        fi
    done
done
rm -rf "${RENDER_HOME}"

# The source of every package is listed (source-manifest.sh), and the written offer it points to ships
diff <(rpm -qa --queryformat '%{NAME}\n' | grep -vx gpg-pubkey | sort) \
    <(awk -F'\t' 'NF == 4 && $1 != "PACKAGE" { print $1 }' /usr/share/licenses/amethystora/SOURCES | sort)
grep -q "^Written offer" /usr/share/licenses/amethystora/NOTICE
# What is not installed from a package has no package to carry its licence: Starship is one binary
# from its release (05-override-install.sh), and the *-gdu.rules are game-devices-udev's
test -x /usr/bin/starship
# Its setup for each shell, written at build time (05-override-install.sh)
grep -q "starship prompt" /usr/share/amethystora/starship/init.bash /usr/share/amethystora/starship/init.zsh \
    /usr/share/amethystora/starship/init.fish
grep -qx 'source /etc/zsh/zshrc' /etc/zshrc
SHELL_HOME="$(mktemp -d)"
HOME="${SHELL_HOME}" zsh -i -c '[[ -n "${_comps[ame]:-}" && "${STARSHIP_SHELL}" == zsh ]]' </dev/null
HOME="${SHELL_HOME}" fish -i -c 'test "$STARSHIP_SHELL" = fish' </dev/null
rm -rf "${SHELL_HOME}"
test -s /usr/share/licenses/starship/LICENSE
compgen -G "/usr/lib/udev/rules.d/71-*-gdu.rules" >/dev/null
test -s /usr/share/licenses/game-devices-udev/LICENSE
# The same for what came with the desktop layer and is somebody else's work under another licence
# (NOTICE names each): Prezto's and zimfw's for the zsh configuration, and the GPL of Realtek's driver for
# its udev rules
for licence in prezto/LICENSE zimfw/LICENSE realtek-r8152/LICENSE; do
    test -s "/usr/share/licenses/${licence}"
done
# The licences Fedora's packages keep among their documentation outlive the documentation
# (05-override-install.sh): zsh and libX11 have theirs nowhere else
test -s /usr/share/licenses/zsh/LICENCE
test -s /usr/share/licenses/libX11-common/COPYING
# The credits in the udev rules taken from Universal Blue still say where the work was done
# (debrand.py leaves them alone)
grep -q "https://github.com/ublue-os/config/pull/45$" /usr/lib/udev/rules.d/70-wooting.rules
# The source packages SOURCES says the image carries are in it. FFmpeg's is one: RPM Fusion's
# libavcodec-freeworld is built from it, and RPM Fusion keeps only its current build
awk -F'\t' '$4 ~ "^/usr/src/amethystora/" { print $4 }' /usr/share/licenses/amethystora/SOURCES |
    sort -u | xargs -r -n1 test -s
compgen -G "/usr/src/amethystora/ffmpeg-*.src.rpm" >/dev/null
rpm -q breeze-icon-theme >/dev/null && false
# The icon packs are the image's own and nothing else is installed as an icon theme: the pack the desktop started
# with is gone, and Firefox keeps the icon in its package (nobody else's logo is redrawn)
[[ ! -e /usr/share/icons/candy-icons ]]
compgen -G "/usr/share/icons/hicolor/*/apps/firefox.*" >/dev/null
# The links an earlier image made in each account to the IDE drawings, which now point at nothing,
# are taken back at login (user-setup.hooks.d/16-retire-jetbrains-icons.sh)
grep -q -- "-xtype l -name 'jetbrains-\*.svg' -delete" /usr/share/amethystora/user-setup.hooks.d/16-retire-jetbrains-icons.sh
# The Security app draws its shield from hicolor as it runs, and the Notes launcher falls back on it under any icon
# theme: both are the Glass pack's drawings (branding/icons.mjs), as the pack has them. glycin decides an icon is SVG
# from its first 256 bytes alone, so the tag has to be within them.
for icon in amethystora-security-status amethystora-notes; do
    head -c 256 "/usr/share/icons/hicolor/scalable/apps/${icon}.svg" | grep -q "<svg"
    cmp -s "/usr/share/icons/hicolor/scalable/apps/${icon}.svg" "/usr/share/icons/amethystora-glass/apps/scalable/${icon}.svg"
done
# The theme is the default before an account applies a theme of its own, and the value
# amethystora-theme falls back to afterwards; the two have to name the same theme
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.desktop.interface icon-theme)" == "'amethystora-glass'" ]]
grep -q "^DEFAULT_ICON_THEME=amethystora-glass$" /usr/lib/amethystora/theme/lib.sh
# An account set up before the Amethystora packs holds an explicit icon-theme that beats the default above,
# so the theme hook has to be past the version that only wrote the desktop's own icons
[[ "$(sed -n 's/^version-script theme user \([0-9]\+\).*/\1/p' \
    /usr/share/amethystora/user-setup.hooks.d/11-theme.sh)" -ge 2 ]]
# ...and past the one that re-applies it after 09-retire-dms.sh, which runs first
[[ "$(sed -n 's/^version-script theme user \([0-9]\+\).*/\1/p' \
    /usr/share/amethystora/user-setup.hooks.d/11-theme.sh)" -ge 5 ]]
# ...and past the one that moved the default from candy-icons to the Amethystora packs
[[ "$(sed -n 's/^version-script theme user \([0-9]\+\).*/\1/p' \
    /usr/share/amethystora/user-setup.hooks.d/11-theme.sh)" -ge 8 ]]
# The eight icon packs branding/icons.mjs draws: Glass is the default and Glass for light follows a light theme, each theme
# names the one that suits its colours and style (icons.theme), and every one is chosen with `ame desktop icons`. They
# draw the same names, no logo of anybody else's and no symbolic icons (the panel keeps Adwaita's), and every launcher
# of this image finds its icon in each of them.
grep -q "^DEFAULT_LIGHT_ICON_THEME=amethystora-glass-light$" /usr/lib/amethystora/theme/lib.sh
GLASS_ICONS="$(find /usr/share/icons/amethystora-glass -name '*.svg' | wc -l)"
((GLASS_ICONS > 600))
# Other makers' apps are drawn by what they do, a maker's logo never: Firefox and Thunderbird keep theirs
[[ -s /usr/share/icons/amethystora-glass/apps/scalable/org.kde.konsole.svg ]]
[[ -z "$(find /usr/share/icons/amethystora-glass -name 'firefox.svg' -o -name 'org.mozilla.*')" ]]
# Extensions and Extension Manager each have a drawing of their own, neither the add-on file type's
EXTENSION_ICONS=(/usr/share/icons/amethystora-glass/{apps/scalable/org.gnome.Extensions,apps/scalable/com.mattjakeman.ExtensionManager,mimetypes/scalable/application-x-addon}.svg)
! cmp -s "${EXTENSION_ICONS[0]}" "${EXTENSION_ICONS[1]}"
! cmp -s "${EXTENSION_ICONS[0]}" "${EXTENSION_ICONS[2]}"
! cmp -s "${EXTENSION_ICONS[1]}" "${EXTENSION_ICONS[2]}"
ICON_PACK_NAMES="$(source /usr/lib/amethystora/theme/lib.sh && echo "${ICON_PACKS[*]}")"
[[ "$(wc -w <<<"${ICON_PACK_NAMES}")" == 8 ]]
# Icon packs are the folders with Directories=; the pointers (amethystora-pointer-*) have none
[[ "$(grep -l '^Directories=' /usr/share/icons/amethystora-*/index.theme | wc -l)" == 8 ]]
for pack in ${ICON_PACK_NAMES}; do
    PACK_DIR="/usr/share/icons/amethystora-${pack}"
    grep -q "^Name=Amethystora " "${PACK_DIR}/index.theme"
    grep -qx "Inherits=Adwaita,hicolor" "${PACK_DIR}/index.theme"
    [[ "$(find "${PACK_DIR}" -name '*.svg' | wc -l)" == "${GLASS_ICONS}" ]]
    [[ -z "$(find "${PACK_DIR}" -name '*-symbolic.svg' -o -type l)" ]]
    for launcher in /usr/share/applications/amethystora-*.desktop; do
        icon="$(sed -n 's/^Icon=//p' "${launcher}" | head -n1)"
        [[ -z "${icon}" || "${icon}" == /* ]] && continue
        [[ -n "$(find "${PACK_DIR}" -name "${icon}.svg")" ]] || {
            echo "${launcher}: ${icon} is not in ${PACK_DIR}"
            false
        }
    done
done
# Which pack is written: the account's own choice, else the theme's, else Glass or, for a light theme, Glass for light
(
    unset XDG_CONFIG_HOME XDG_DATA_HOME
    HOME="$(mktemp -d)"
    # shellcheck source=/dev/null
    source /usr/lib/amethystora/theme/lib.sh
    mkdir -p "${CURRENT_THEME_DIR}"
    [[ "$(icon_theme)" == amethystora-glass ]]
    touch "${CURRENT_THEME_DIR}/light.mode"
    [[ "$(icon_theme)" == amethystora-glass-light ]]
    # A name that is not installed is passed over, wherever it comes from, as candy-icons now is
    echo candy-icons >"${CURRENT_THEME_DIR}/icons.theme"
    [[ "$(icon_theme)" == amethystora-glass-light ]]
    mkdir -p "${XDG_DATA_HOME:-${HOME}/.local/share}/icons/some-pack"
    touch "${XDG_DATA_HOME:-${HOME}/.local/share}/icons/some-pack/index.theme"
    echo some-pack >"${CURRENT_THEME_DIR}/icons.theme"
    [[ "$(icon_theme)" == some-pack ]]
    echo candy-icons >"${CONFIG_DIR}/icons"
    [[ "$(icon_theme)" == some-pack ]]
    echo amethystora-neon >"${CONFIG_DIR}/icons"
    [[ "$(icon_theme)" == amethystora-neon ]]
    rm -rf "${HOME}"
)

# The pointers branding/cursors.mjs draws (plans/beyond/spec-pointer.md): three sets, one Xcursor file per drawing, each
# of five square sizes and the animated ones of 20 frames, and every other name a toolkit asks for a link 10-icons.sh
# made from the set's aliases table, to a drawing's file in the same folder; Halo the default here and in the library
POINTER_SET_NAMES="$(source /usr/lib/amethystora/theme/lib.sh && echo "${POINTER_SETS[*]}")"
[[ "${POINTER_SET_NAMES}" == "halo ink facet" ]]
grep -q "^DEFAULT_POINTER=amethystora-pointer-halo$" /usr/lib/amethystora/theme/lib.sh
grep -q "^DEFAULT_LIGHT_POINTER=amethystora-pointer-ink$" /usr/lib/amethystora/theme/lib.sh
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.desktop.interface cursor-theme)" == "'amethystora-pointer-halo'" ]]
for set in ${POINTER_SET_NAMES}; do
    dir="/usr/share/icons/amethystora-pointer-${set}"
    grep -qx "Name=Amethystora ${set^}" "${dir}/index.theme"
    grep -qx "Inherits=Adwaita" "${dir}/index.theme"
    grep -q '^Directories=' "${dir}/index.theme" && false
    [[ "$(find "${dir}/cursors" -type f | wc -l)" == 22 ]]
    # Every alias a link, relative, to its drawing's own file and folder, every link one the table names, and the table
    # whole: 94 other names, 116 in all
    [[ "$(grep -cv '^#' "${dir}/aliases")" == 94 ]]
    while read -r alias target; do
        [[ -z "${alias}" || "${alias}" == \#* ]] && continue
        [[ -L "${dir}/cursors/${alias}" && "$(readlink "${dir}/cursors/${alias}")" == "${target}" ]] ||
            { echo "${dir}/cursors/${alias} is not a link to ${target}"; false; }
        [[ -f "${dir}/cursors/${target}" && ! -L "${dir}/cursors/${target}" ]]
        [[ -L "${dir}/cursors_scalable/${alias}" && "$(readlink "${dir}/cursors_scalable/${alias}")" == "${target}" ]] ||
            { echo "${dir}/cursors_scalable/${alias} is not a link to ${target}"; false; }
    done <"${dir}/aliases"
    [[ "$(find "${dir}/cursors" -type l | wc -l)" == "$(grep -cv '^#' "${dir}/aliases")" ]]
    [[ "$(find "${dir}/cursors_scalable" -mindepth 1 -maxdepth 1 -type l | wc -l)" == "$(grep -cv '^#' "${dir}/aliases")" ]]
    [[ -z "$(find -L "${dir}/cursors" -type l)" ]]
    for name in default context-menu help pointer progress wait cell crosshair text vertical-text alias copy move \
        no-drop not-allowed grab grabbing e-resize n-resize ne-resize nw-resize s-resize se-resize sw-resize w-resize \
        ew-resize ns-resize nesw-resize nwse-resize col-resize row-resize all-scroll zoom-in zoom-out dnd-ask all-resize \
        left_ptr hand2 xterm watch left_ptr_watch crossed_circle fleur sb_h_double_arrow sb_v_double_arrow size_fdiag \
        size_bdiag openhand closedhand e29285e634086352946a0e7090d73106; do
        [[ -f "${dir}/cursors/${name}" ]] || { echo "${dir} has no ${name}"; false; }
    done
    [[ "$(readlink -f "${dir}/cursors/left_ptr")" == "${dir}/cursors/default" ]]
    [[ "$(readlink -f "${dir}/cursors/hand2")" == "${dir}/cursors/pointer" ]]
    [[ "$(readlink -f "${dir}/cursors/grab")" == "${dir}/cursors/move" ]]
    [[ "$(readlink -f "${dir}/cursors/watch")" == "${dir}/cursors/wait" ]]
    # GNOME Shell 51 reads its own pointers as SVG, cursors_scalable/<name>/metadata.json: every name it asks for (its
    # ClutterCursorType names) is there, through the same links, and lists frames that are there
    [[ "$(find "${dir}/cursors_scalable" -mindepth 1 -maxdepth 1 -type d | wc -l)" == 22 ]]
    [[ -z "$(find -L "${dir}/cursors_scalable" -type l)" ]]
    for name in default context-menu help pointer progress wait cell crosshair text vertical-text alias copy move \
        no-drop not-allowed grab grabbing e-resize n-resize ne-resize nw-resize s-resize se-resize sw-resize w-resize \
        ew-resize ns-resize nesw-resize nwse-resize col-resize row-resize all-scroll zoom-in zoom-out dnd-ask all-resize; do
        meta="${dir}/cursors_scalable/${name}/metadata.json"
        jq -e 'type == "array" and length > 0 and all(.[]; .nominal_size == 24 and (.filename | test("^[a-z-]+(-[0-9]{2})?[.]svg$")))' \
            "${meta}" >/dev/null || { echo "${meta} is not a list of frames"; false; }
        for frame in $(jq -r '.[].filename' "${meta}"); do
            grep -q '<svg xmlns="http://www.w3.org/2000/svg" width="30" height="30"' "$(dirname "${meta}")/${frame}"
        done
    done
    [[ "$(jq length "${dir}/cursors_scalable/wait/metadata.json")" == 20 ]]
    python3 - "${dir}/cursors" <<'PY'
import os, struct, sys
d = sys.argv[1]
seen = set()
total = 0
for name in sorted(os.listdir(d)):
    path = os.path.join(d, name)
    if os.path.islink(path):
        continue
    b = open(path, 'rb').read()
    magic, header, version, ntoc = struct.unpack_from('<4sIII', b, 0)
    assert magic == b'Xcur' and header == 16 and version == 0x10000, name
    frames = {}
    for i in range(ntoc):
        kind, size, pos = struct.unpack_from('<III', b, 16 + 12 * i)
        assert kind == 0xfffd0002, name
        h, k2, sub, ver, w, ht, xh, yh, delay = struct.unpack_from('<IIIIIIIII', b, pos)
        assert (h, k2, sub, ver) == (36, 0xfffd0002, size, 1), name
        assert w == ht == size * 30 // 24 and xh < w and yh < ht and delay == 50, name
        assert pos + 36 + 4 * w * ht <= len(b), name
        frames[size] = frames.get(size, 0) + 1
    assert sorted(frames) == [24, 32, 48, 64, 96], name
    assert set(frames.values()) == ({20} if name in ('wait', 'progress') else {1}), name
    seen.add(b)
    total += len(b)
assert len(seen) == 22, len(seen)
assert total < 8_000_000, total
PY
done
# Which pointer is written: the account's own choice, else the theme's, else Halo or, for a light theme, Ink; a name
# that is not installed is passed over
(
    unset XDG_CONFIG_HOME XDG_DATA_HOME
    HOME="$(mktemp -d)"
    # shellcheck source=/dev/null
    source /usr/lib/amethystora/theme/lib.sh
    mkdir -p "${CURRENT_THEME_DIR}"
    [[ "$(pointer_theme)" == amethystora-pointer-halo ]]
    touch "${CURRENT_THEME_DIR}/light.mode"
    [[ "$(pointer_theme)" == amethystora-pointer-ink ]]
    echo nonexistent-cursors >"${CURRENT_THEME_DIR}/cursor.theme"
    [[ "$(pointer_theme)" == amethystora-pointer-ink ]]
    echo amethystora-pointer-facet >"${CONFIG_DIR}/pointer"
    [[ "$(pointer_theme)" == amethystora-pointer-facet ]]
    rm -rf "${HOME}"
)
# The theme writes over only a pointer it or an earlier image wrote
for name in Breeze_Light breeze_cursors amethystora-pointer-halo amethystora-pointer-ink amethystora-pointer-facet; do
    grep -q "${name}" /usr/bin/amethystora-theme
done
# The pointer module: its schema, its defaults, started by the extension, and the pointer handed back however it ends
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys org.gnome.shell.extensions.amethystora-ui.pointer | sort | tr '\n' ' ')" == "dense shake " ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.pointer shake)" == "'off'" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.pointer dense)" == "true" ]]
grep -q "\['pointer', Pointer\]" "${UI_EXTENSION}/extension.js"
grep -q "uninhibit_cursor_visibility" "${UI_EXTENSION}/modules/pointer.js"
grep -q "uninhibit_unfocus" "${UI_EXTENSION}/modules/pointer.js"
grep -q "set_pointer_visible" "${UI_EXTENSION}/modules/pointer.js" && false
grep -qE "gi://" "${UI_EXTENSION}/modules/pointer/shake.js" "${UI_EXTENSION}/modules/pointer/dense.js" && false
for recipe in pointer pointer-shake pointer-dense; do
    grep -q "^${recipe} [A-Z]*=\"\":" /usr/share/amethystora/just/desktop.just
done
grep -q "amethystora-pointer-halo" /usr/share/licenses/amethystora/NOTICE

# The Soft sounds (manual: desktop.md#sounds): Amethystora's sound theme, chosen by the image, falling through to the
# freedesktop theme for every other name; nine files of 48 kHz 16-bit mono PCM, none longer than two seconds or louder
# than -1 dBFS, the error bell the same bytes as the error; the login and power-plug sounds played by the sounds module,
# which has no setting and no timer that repeats; ame desktop sounds soft|off
SOUNDS=/usr/share/sounds/amethystora
grep -qx "Inherits=freedesktop" "${SOUNDS}/index.theme"
grep -qx "Directories=stereo" "${SOUNDS}/index.theme"
test -d /usr/share/sounds/freedesktop/stereo
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.desktop.sound theme-name)" == "'amethystora'" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.desktop.sound event-sounds)" == true ]]
[[ "$(find "${SOUNDS}/stereo" -type f -name '*.wav' | wc -l)" == 9 ]]
cmp "${SOUNDS}/stereo/bell-window-system.wav" "${SOUNDS}/stereo/dialog-error.wav"
# ...and Glass, Wood and Retro beside it, the same nine events in other voices (round 6's R6-04)
for set in glass wood retro; do
    grep -qx "Inherits=freedesktop" "${SOUNDS}-${set}/index.theme"
    grep -qx "Directories=stereo" "${SOUNDS}-${set}/index.theme"
    [[ "$(find "${SOUNDS}-${set}/stereo" -type f -name '*.wav' | wc -l)" == 9 ]]
    cmp "${SOUNDS}-${set}/stereo/bell-window-system.wav" "${SOUNDS}-${set}/stereo/dialog-error.wav"
done
python3 -I - "${SOUNDS}/stereo" "${SOUNDS}-glass/stereo" "${SOUNDS}-wood/stereo" "${SOUNDS}-retro/stereo" <<'EOF'
import array, os, sys, wave
for folder, name in ((folder, name) for folder in sys.argv[1:] for name in (
        "message-new-instant", "message-new-instant-phone", "power-plug", "screen-capture", "trash-empty",
        "audio-volume-change", "dialog-error", "bell-window-system", "desktop-login")):
    with wave.open(os.path.join(folder, f"{name}.wav")) as w:
        shape = (w.getnchannels(), w.getsampwidth(), w.getframerate(), w.getcomptype())
        assert shape == (1, 2, 48000, "NONE"), f"{name}: {shape}"
        frames = w.getnframes()
        assert 0 < frames <= 2 * 48000, f"{name}: {frames} frames"
        samples = array.array("h", w.readframes(frames))
        if sys.byteorder == "big":
            samples.byteswap()
        assert max(abs(s) for s in samples) <= 29200, f"{name} is louder than -1 dBFS"
EOF
grep -q "\['sounds', Sounds\]" "${UI_EXTENSION}/extension.js"
grep -q "'desktop-login'" "${UI_EXTENSION}/modules/sounds.js"
grep -q "'power-plug'" "${UI_EXTENSION}/modules/sounds.js"
[[ -z "$(grep -E 'setInterval|setTimeout|Subprocess|spawn' "${UI_EXTENSION}/modules/sounds.js")" ]]
# ...and only by it: libcanberra's own login sound would play the chord a second time (17-cleanup.sh)
grep -qx "Hidden=true" /etc/xdg/autostart/libcanberra-login-sound.desktop
test ! -e /usr/share/gnome/autostart/libcanberra-login-sound.desktop
[[ -z "$(grep -rlsx 'Exec=.*canberra-gtk-play.*desktop-login.*' /usr/share/gnome/autostart /etc/xdg/autostart)" ]]
grep -q "branding/sounds.mjs" /usr/share/licenses/amethystora/NOTICE
# Rounded screen corners (manual: themes.md#rounded-screen-corners): the bezel module's one key, off, its corners
# taking no clicks; ame desktop screen-corners on|off
BEZEL=org.gnome.shell.extensions.amethystora-ui.bezel
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${BEZEL}" | paste -sd ' ')" == "round-corners" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${BEZEL}" round-corners)" == false ]]
grep -q "\['bezel', Bezel\]" "${UI_EXTENSION}/extension.js"
grep -q "reactive: false" "${UI_EXTENSION}/modules/bezel.js"
grep -q "is_builtin()" "${UI_EXTENSION}/modules/bezel.js"
[[ -z "$(grep -E 'timeout_add|setInterval|setTimeout' "${UI_EXTENSION}/modules/bezel.js")" ]]
for recipe in sounds screen-corners; do
    grep -q "^${recipe} [A-Z]*=\"\":" /usr/share/amethystora/just/desktop.just
    grep -qx "desktop::${recipe}" <<<"$(ame --summary | tr ' ' '\n')"
done
GSETTINGS_BACKEND=memory ame desktop sounds loud >/dev/null 2>&1 && false
GSETTINGS_BACKEND=memory ame desktop screen-corners maybe >/dev/null 2>&1 && false
GSETTINGS_BACKEND=memory ame desktop sounds off >/dev/null
# GNOME's volume keys play their step whatever event-sounds says, so off needs the silent theme's
test -s /usr/share/sounds/amethystora-silent/stereo/audio-volume-change.wav
grep -q 'theme-name amethystora-silent' /usr/share/amethystora/just/desktop.just
GSETTINGS_BACKEND=memory ame desktop screen-corners on >/dev/null
GSETTINGS_BACKEND=memory ame desktop sounds glass >/dev/null
grep -q 'theme="amethystora-${choice}"' /usr/share/amethystora/just/desktop.just
# Sound, screens and devices (round 6's sound group, plans/ui-ideas/spec-sound.md; manual: desktop.md#clear-voice,
# #sound-presets, #pause-when-the-headphones-come-off, hardware.md#external-screens-brightness). The audio module's keys:
# pausing on (R6-03), Clear voice and the presets off (R6-01, R6-02), the presets by output empty; the brightness
# module's one key off (R6-05). Both after quick settings, whose menus and grid they add to.
AUDIO=org.gnome.shell.extensions.amethystora-ui.audio
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${AUDIO}" | sort | paste -sd ' ')" == \
    "clear-voice pause-on-unplug presets sound-presets" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${AUDIO}" pause-on-unplug)" == true ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${AUDIO}" clear-voice)" == false ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${AUDIO}" sound-presets)" == false ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${AUDIO}" presets)" == "@a{ss} {}" ]]
BRIGHT=org.gnome.shell.extensions.amethystora-ui.brightness
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${BRIGHT}" | paste -sd ' ')" == external-screens ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${BRIGHT}" external-screens)" == false ]]
grep -q "\['audio', Audio\]" "${UI_EXTENSION}/extension.js"
grep -q "\['brightness', Brightness\]" "${UI_EXTENSION}/extension.js"
SOUND_ORDER=" $(sed -n "s/^    \['\([a-z]*\)', [A-Za-z]*\],$/\1/p" "${UI_EXTENSION}/extension.js" | paste -sd ' ') "
[[ "${SOUND_ORDER}" == *" quick "*" audio "* && "${SOUND_ORDER}" == *" quick "*" brightness "* ]]
for file in modules/audio.js modules/audio/menu.js modules/audio/voice.js modules/audio/presets.js \
    modules/audio/unplug.js modules/brightness.js modules/power/devices.js stylesheets/audio.css stylesheets/brightness.css; do
    test -s "${UI_EXTENSION}/${file}"
done
# Nothing they show is markup, nothing runs through a shell: the audio module runs only its helper, the brightness
# module only ddcutil (under timeout), each as an argument vector
[[ -z "$(grep -rnE 'use_markup = true|use_markup: true|set_markup\(|Util\.spawn|spawn_command_line|GLib\.spawn' \
    "${UI_EXTENSION}/modules/audio.js" "${UI_EXTENSION}/modules/audio" "${UI_EXTENSION}/modules/brightness.js")" ]]
grep -q "^export const HELPER = '/usr/libexec/amethystora-sound';" "${UI_EXTENSION}/modules/audio/menu.js"
[[ "$(grep -c 'Gio.Subprocess.new' "${UI_EXTENSION}/modules/audio/menu.js" "${UI_EXTENSION}/modules/brightness.js" |
    paste -sd ' ')" == *":1 "*":1" ]]
# Clear voice and the presets: PipeWire's own echo cancel (its webrtc processor) and filter chain's builtin biquads, no
# plugin package; the helper's units start with PipeWire for every account and run only for one whose key is on
SOUND_HELPER=/usr/libexec/amethystora-sound
test -x "${SOUND_HELPER}"
test -e /usr/lib64/spa-0.2/aec/libspa-aec-webrtc.so
test -e /usr/lib64/spa-0.2/filter-graph/libspa-filter-graph-plugin-builtin.so
test -e /usr/lib64/pipewire-0.3/libpipewire-module-echo-cancel.so
test -e /usr/lib64/pipewire-0.3/libpipewire-module-filter-chain.so
command -v pactl >/dev/null
for pair in clear-voice:clear-voice sound-presets:presets; do
    unit="amethystora-${pair%%:*}.service"
    [[ "$(systemctl --global is-enabled "${unit}")" == enabled ]]
    grep -qx "ExecCondition=${SOUND_HELPER} wanted ${pair#*:}" "/usr/lib/systemd/user/${unit}"
    grep -qx "ExecStart=${SOUND_HELPER} run ${pair#*:}" "/usr/lib/systemd/user/${unit}"
done
GSETTINGS_BACKEND=memory "${SOUND_HELPER}" wanted clear-voice && false
GSETTINGS_BACKEND=memory "${SOUND_HELPER}" wanted presets && false
GSETTINGS_BACKEND=memory "${SOUND_HELPER}" preset loud >/dev/null 2>&1 && false
GSETTINGS_BACKEND=memory "${SOUND_HELPER}" conf clear-voice | grep -q "library.name = aec/libspa-aec-webrtc"
GSETTINGS_BACKEND=memory "${SOUND_HELPER}" conf clear-voice | grep -q "monitor.mode = true"
[[ "$(jq -c 'keys' /usr/share/amethystora/sound/presets.json)" == '["bass","laptop","speech"]' ]]
SOUND_HOME="$(mktemp -d)"
(
    export GSETTINGS_BACKEND=keyfile XDG_CONFIG_HOME="${SOUND_HOME}"
    gsettings set "${AUDIO}" presets "{'alsa_output.pci-0000_00_1f.3.analog-stereo': 'laptop', 'bad name': 'bass'}"
    conf="$("${SOUND_HELPER}" conf presets)"
    [[ "$(grep -c 'libpipewire-module-filter-chain' <<<"${conf}")" == 1 ]]
    grep -qF 'filter.smart.target = { node.name = "alsa_output.pci-0000_00_1f.3.analog-stereo" }' <<<"${conf}"
    grep -q 'label = bq_lowshelf' <<<"${conf}"
)
rm -rf "${SOUND_HOME}"
for recipe in pause-on-unplug clear-voice sound-presets sound-preset screen-brightness; do
    grep -q "^${recipe} [A-Z]*=\"\":" /usr/share/amethystora/just/desktop.just
    grep -qx "desktop::${recipe}" <<<"$(ame --summary | tr ' ' '\n')"
done
GSETTINGS_BACKEND=memory ame desktop sound-preset loud >/dev/null 2>&1 && false
GSETTINGS_BACKEND=memory ame desktop pause-on-unplug maybe >/dev/null 2>&1 && false
# External screens' brightness (decision 6): ddcutil installed, its own rule and modules-load file out of the image,
# and the image's one-line rule only in /usr/share until ame desktop screen-brightness on copies it; the watcher takes
# exactly that copy, and the report says the grant as information
command -v ddcutil >/dev/null
test ! -e /usr/lib/udev/rules.d/60-ddcutil-i2c.rules
test ! -e /usr/lib/modules-load.d/ddcutil.conf
test ! -e /etc/udev/rules.d/60-amethystora-ddc.rules
test ! -e /etc/modules-load.d/amethystora-ddc.conf
[[ "$(grep -v '^#' /usr/share/amethystora/ddc/60-amethystora-ddc.rules)" == \
    'SUBSYSTEM=="i2c-dev", KERNEL=="i2c-[0-9]*", ATTRS{class}=="0x03*", TAG+="uaccess"' ]]
[[ "$(grep -v '^#' /usr/share/amethystora/ddc/amethystora-ddc.conf)" == i2c-dev ]]
grep -q 'cmp -s "${etc}" /usr/share/amethystora/ddc/60-amethystora-ddc.rules' /usr/libexec/amethystora-security-watch
grep -q "report apps screen-brightness info" /usr/libexec/amethystora-security-status
# Every device's battery (R6-26): the power module's third switch, UPower's signals, a notice once at 15%
grep -q "connectSignal('DeviceAdded'" "${UI_EXTENSION}/modules/power/devices.js"
[[ -z "$(grep -E 'timeout_add|setInterval|setTimeout' "${UI_EXTENSION}/modules/power/devices.js")" ]]
# Seeing and reading, touch and pen (plans/beyond/spec-r4-access.md; manual: themes.md#seeing-and-reading,
# desktop.md#touch-and-pen): the access and touch modules' keys, everything off for a new account, their three shortcuts
# Super+Ctrl+R, S and P (the board's Super+Alt+R is the tiling's and Super+Alt+S GNOME's screen reader), both modules
# after bezel and before focus. The filter is an effect with a Cogl snippet, the screenshots it sets aside are GNOME's
# own calls; reading aloud runs only spd-say with "--" before the text, which the image has as Orca's; the pen's tools
# run only Notes' quick note; nothing is markup. GNOME's stylus settings have the keybinding action pen-button sets.
ACCESS=org.gnome.shell.extensions.amethystora-ui.access
TOUCH=org.gnome.shell.extensions.amethystora-ui.touch
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${ACCESS}" | sort | paste -sd ' ')" == \
    "filter filter-on filter-strength letters letters-key read-aloud read-aloud-key read-aloud-speed ruler ruler-key ruler-shown" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${TOUCH}" | sort | paste -sd ' ')" == \
    "pen-ring pen-ring-key pen-ring-tools touch-mode" ]]
for pair in filter:"'none'" filter-strength:100 ruler:"'off'" read-aloud:false read-aloud-speed:"'1'" \
    ruler-key:"['<Super><Control>r']" read-aloud-key:"['<Super><Control>s']"; do
    [[ "$(GSETTINGS_BACKEND=memory gsettings get "${ACCESS}" "${pair%%:*}")" == "${pair#*:}" ]]
done
for pair in touch-mode:"'off'" pen-ring:"'off'" pen-ring-key:"['<Super><Control>p']" \
    pen-ring-tools:"['scribble', 'area', 'note', 'text', 'right-click', 'back']"; do
    [[ "$(GSETTINGS_BACKEND=memory gsettings get "${TOUCH}" "${pair%%:*}")" == "${pair#*:}" ]]
done
# In order, not side by side: other groups' modules may come between (focus is last, checked with Focus profiles)
[[ "$(grep -oE "^    \['(bezel|access|touch|focus)'," "${UI_EXTENSION}/extension.js" | paste -sd ' ')" == \
    "    ['bezel',     ['access',     ['touch',     ['focus'," ]]
OTHER_COMBOS="$(XDG_CURRENT_DESKTOP=GNOME GSETTINGS_BACKEND=memory gsettings list-recursively |
    grep -v "^org.gnome.shell.extensions.amethystora-ui.\(access\|touch\) " | keycombos)"
OTHER_COMBOS+=$'\n'"$(grep -h -d skip -e "^binding=" /etc/dconf/db/distro.d/* | keycombos)"
for combo in "<control><super>r" "<control><super>s" "<control><super>p"; do
    if grep -qx "${combo}" <<<"${OTHER_COMBOS}"; then
        echo "${combo} is bound elsewhere, and is the reading ruler's, reading aloud's or the pen's tools'"
        exit 1
    fi
done
for file in modules/access.js modules/access/filter.js modules/access/matrices.js modules/access/ruler.js \
    modules/access/speech.js modules/access/lines.js modules/touch.js modules/touch/mode.js modules/touch/ring.js \
    stylesheets/access.css stylesheets/touch.css; do
    test -s "${UI_EXTENSION}/${file}"
done
test -s /usr/share/amethystora/themed/shell-access.css.tpl
test -s /usr/share/amethystora/themed/shell-touch.css.tpl
grep -q "extends Clutter.Effect" "${UI_EXTENSION}/modules/access/filter.js"
grep -q "Clutter.LayerNode.new_to_framebuffer" "${UI_EXTENSION}/modules/access/filter.js"
grep -q "Cogl.SnippetHook.FRAGMENT" "${UI_EXTENSION}/modules/access/filter.js"
# On the stage, so that Zoom's magnified picture goes through it too, and the one effect Focus's grey goes through
grep -q "global.stage.add_effect_with_name(EFFECT_NAME" "${UI_EXTENSION}/modules/access/filter.js"
grep -q "setExtraMatrix(GREY" "${UI_EXTENSION}/modules/focus.js"
# What is drawn over windows holds off direct scanout while it shows: the filter, the ruler and the pen's ring
for file in access/filter.js access/ruler.js touch/ring.js; do
    grep -q "disable_unredirect()" "${UI_EXTENSION}/modules/${file}"
    grep -q "enable_unredirect()" "${UI_EXTENSION}/modules/${file}"
done
# Nothing wraps GNOME's screenshot calls: a screenshot is a paint for no screen, which the filter leaves alone
[[ -z "$(grep -rn "Screenshot.prototype" "${UI_EXTENSION}/modules/access.js" "${UI_EXTENSION}/modules/access")" ]]
[[ -z "$(grep -rnE 'use_markup = true|use_markup: true|set_markup\(' "${UI_EXTENSION}/modules/access.js" \
    "${UI_EXTENSION}/modules/access" "${UI_EXTENSION}/modules/touch.js" "${UI_EXTENSION}/modules/touch")" ]]
# Reading aloud starts spd-say alone, twice: each sentence on its standard input (never its command line, which every
# account can read) and its stop; the pen's tools start Notes' quick note alone
[[ "$(grep -rhoE "Subprocess\.new\([A-Za-z_\[]+" "${UI_EXTENSION}/modules/access.js" "${UI_EXTENSION}/modules/access" |
    sort | paste -sd ' ')" == "Subprocess.new([HELPER Subprocess.new([SPD_SAY Subprocess.new(argv" ]]
grep -q "const argv = \[SPD_SAY, '--pipe-mode'," "${UI_EXTENSION}/modules/access/speech.js"
grep -q "communicate_utf8_async(" "${UI_EXTENSION}/modules/access/speech.js"
[[ "$(grep -rhoE "Subprocess\.new\([A-Za-z_\[]+" "${UI_EXTENSION}/modules/touch.js" "${UI_EXTENSION}/modules/touch" |
    sort | paste -sd ' ')" == "Subprocess.new([NOTES" ]]
[[ -z "$(grep -rnE 'Util\.spawn|spawn_command_line|GLib\.spawn' "${UI_EXTENSION}/modules/access.js" \
    "${UI_EXTENSION}/modules/access" "${UI_EXTENSION}/modules/touch.js" "${UI_EXTENSION}/modules/touch")" ]]
command -v spd-say >/dev/null
rpm -q speech-dispatcher-utils speech-dispatcher-espeak-ng espeak-ng >/dev/null
GSETTINGS_BACKEND=memory gsettings range org.gnome.desktop.peripherals.tablet.stylus:/org/gnome/desktop/peripherals/stylus/x/ \
    button-action | grep -qx "'keybinding'"
GSETTINGS_BACKEND=memory gsettings list-keys org.gnome.desktop.peripherals.tablet.stylus:/org/gnome/desktop/peripherals/stylus/x/ |
    grep -qx button-keybinding
for recipe in access touch; do
    grep -q "^${recipe} \*ARGS:" /usr/share/amethystora/just/desktop.just
    grep -qx "desktop::${recipe}" <<<"$(ame --summary | tr ' ' '\n')"
done
GSETTINGS_BACKEND=memory ame desktop access filter red >/dev/null 2>&1 && false
GSETTINGS_BACKEND=memory ame desktop access strength 10 >/dev/null 2>&1 && false
GSETTINGS_BACKEND=memory ame desktop touch mode tablet >/dev/null 2>&1 && false
GSETTINGS_BACKEND=memory ame desktop touch pen-tools area area >/dev/null 2>&1 && false
GSETTINGS_BACKEND=memory ame desktop access filter deutan >/dev/null
GSETTINGS_BACKEND=memory ame desktop touch pen-tools area back >/dev/null
# The "later" group of rounds 5 and 6 (plans/ui-ideas/spec-later.md; manual: desktop.md#dictation,
# desktop.md#live-captions, desktop.md#finding-things, themes.md#click-with-the-keyboard, security.md#guest-login),
# each offered and off. Captions in ten languages, each model pinned; dictation on the captions' recognizer, its
# shortcut bound only while it is on; Firefox's bookmarks from a copy that never reads a visit; Notes only on the
# session bus while unlocked; files by meaning from localsearch's text, a user timer that nothing enables, on mains
# power only; click with the keyboard, its tree read by a program of its own; the guest login a security switch.
CAPTIONS=org.gnome.shell.extensions.amethystora-ui.captions
for pair in language:"'en'" dictation:false dictate:"['<Super><Control><Shift>d']"; do
    [[ "$(GSETTINGS_BACKEND=memory gsettings get "${CAPTIONS}" "${pair%%:*}")" == "${pair#*:}" ]]
done
[[ "$(GSETTINGS_BACKEND=memory gsettings range "${CAPTIONS}" language | xargs)" == "enum en fr es de it tr pt nl ru pl" ]]
[[ "$(grep -cE "^    '[a-z]{2}': \('vosk-model-small-[a-z]{2}(-us)?-0\.[0-9]+', '[0-9a-f]{64}',$" /usr/libexec/amethystora-captions)" == 10 ]]
[[ "$(grep -cE "Subprocess\.new|Util\.spawn|spawn_command_line|GLib\.spawn" "${UI_EXTENSION}/modules/captions/dictation.js")" == 1 ]]
grep -q "Gio.Subprocess.new(\[HELPER, 'dictate', language\]," "${UI_EXTENSION}/modules/captions/dictation.js"
grep -q "Main.wm.addKeybinding(KEY, this._settings" "${UI_EXTENSION}/modules/captions/dictation.js"
[[ -z "$(grep -nE 'use_markup = true|use_markup: true|set_markup\(' "${UI_EXTENSION}/modules/captions/dictation.js")" ]]
grep -q '^dictation ACTION="":' /usr/share/amethystora/just/desktop.just
for key in bookmarks notes meaning; do
    [[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.search "${key}")" == false ]]
    test -s "${UI_EXTENSION}/modules/search/${key}.js"
done
for helper in amethystora-bookmarks amethystora-file-meaning amethystora-letters amethystora-guest; do
    test -x "/usr/libexec/${helper}"
done
for helper in amethystora-bookmarks amethystora-file-meaning amethystora-letters; do
    python3 -I -c 'import ast, sys; ast.parse(open(sys.argv[1]).read())' "/usr/libexec/${helper}"
done
bash -n /usr/libexec/amethystora-guest
# The bookmarks are read from a copy, which goes before anything is printed, and never a visit
[[ -z "$(grep -v '^ *#' /usr/libexec/amethystora-bookmarks | grep -n "moz_historyvisits\|visit_count\|last_visit")" ]]
grep -q "WHERE b.type = 1" /usr/libexec/amethystora-bookmarks
grep -q "shutil.rmtree(work, ignore_errors=True)" /usr/libexec/amethystora-bookmarks
# Notes answers only the shell, at an object that is there only while the notes are open
grep -q "Gio.DBusCallFlags.NO_AUTO_START" "${UI_EXTENSION}/modules/search/notes.js"
# Files by meaning: the account's timer, which nothing enables for every account, on mains power, and the index's own model
grep -qx "ConditionACPower=true" /usr/lib/systemd/user/amethystora-file-meaning.service
[[ "$(systemctl --global is-enabled amethystora-file-meaning.timer 2>/dev/null)" != enabled ]]
test -s /usr/share/amethystora/agents/knowledge/model/model.safetensors
[[ "$(/usr/libexec/amethystora-agent-tools files-search --dir /nonexistent -- tax refund)" == "[]" ]]
# Click with the keyboard: off, its shortcut, the program that reads the tree the only one it starts
for pair in letters:false letters-key:"['<Super><Alt>k']"; do
    [[ "$(GSETTINGS_BACKEND=memory gsettings get "${ACCESS}" "${pair%%:*}")" == "${pair#*:}" ]]
done
grep -q "Gio.Subprocess.new(\[HELPER, String(window.get_pid())" "${UI_EXTENSION}/modules/access/letters.js"
python3 -I -c 'import gi; gi.require_version("Atspi", "2.0"); from gi.repository import Atspi'
# Its key and dictation's are free: in GNOME's own schemas, the image's custom keybindings, every other key of the
# extension and the tiling's
LATER_COMBOS="$(XDG_CURRENT_DESKTOP=GNOME GSETTINGS_BACKEND=memory gsettings list-recursively |
    grep -vE "^org.gnome.shell.extensions.amethystora-ui.(access letters-key|captions dictate) " | keycombos)"
LATER_COMBOS+=$'\n'"$(grep -h -d skip -e "^binding=" /etc/dconf/db/distro.d/* | keycombos)"
for combo in "<alt><super>k" "<control><shift><super>d"; do
    if grep -qx "${combo}" <<<"${LATER_COMBOS}"$'\n'"${TILING_COMBOS}"; then
        echo "${combo} is bound elsewhere, and is click with the keyboard's or dictation's"
        exit 1
    fi
done
# The guest login: a security switch, off, its PAM lines in the image's three profiles only behind its feature, and no
# such account in the image
/usr/libexec/amethystora-hardening is-on guest-login && false
[[ "$(/usr/libexec/amethystora-hardening status --json | jq -r '.[] | select(.name == "guest-login") | "\(.group) \(.default) \(.loosens)"')" == \
    "logins off true" ]]
getent passwd amethystora-guest >/dev/null && false
for profile in local sssd winbind; do
    [[ "$(grep -c 'pam_succeed_if.so quiet user != amethystora-guest {include if "with-guest-login"}' \
        "/usr/share/authselect/vendor/${profile}/system-auth")" == 1 ]]
    grep -q 'pam_succeed_if.so quiet service = gdm-password {include if "with-guest-login"}' \
        "/usr/share/authselect/vendor/${profile}/password-auth"
done
[[ -z "$(grep -n amethystora-guest /etc/authselect/system-auth /etc/authselect/password-auth)" ]]
grep -q '^guest-login ACTION="":' /usr/share/amethystora/just/security.just
# The shelf (plans/beyond/spec-r4-shelf.md, R4-08; manual: desktop.md#the-shelf): one key, off for a new account, the
# module beside the Shelf app (the apps' loop above); the shell starts nothing but the app, with the drop on its standard
# input and never on its command line, and shows nothing as markup; the app offers what is dragged out as a copy only
SHELF=org.gnome.shell.extensions.amethystora-ui.shelf
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${SHELF}" | paste -sd ' ')" == "shelf" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${SHELF}" shelf)" == "'off'" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings range "${SHELF}" shelf | xargs)" == "enum off edge shake" ]]
grep -q "\['shelf', Shelf\]" "${UI_EXTENSION}/extension.js"
for file in modules/shelf.js modules/shelf/drop.js stylesheets/shelf.css; do
    test -s "${UI_EXTENSION}/${file}"
done
test -s /usr/share/amethystora/themed/shell-shelf.css.tpl
[[ "$(grep -rnE "Subprocess|Util\.spawn|spawn_command_line|GLib\.spawn" "${UI_EXTENSION}/modules/shelf.js" \
    "${UI_EXTENSION}/modules/shelf" | grep -c .)" == 2 ]]
grep -q "Gio.Subprocess.new(\[PROGRAM, 'take', " "${UI_EXTENSION}/modules/shelf/drop.js"
grep -q "Gio.SubprocessFlags.STDIN_PIPE" "${UI_EXTENSION}/modules/shelf/drop.js"
grep -q "^export const PROGRAM = '/usr/bin/amethystora-shelf';" "${UI_EXTENSION}/modules/shelf/drop.js"
# ...within the same limits as the program's own, and a window is the shelf's only if it runs the shelf's program
for limit in MOST_TEXT MOST_PICTURE; do
    shell_limit="$(sed -n "s/^export const ${limit} = \(.*\);$/\1/p" "${UI_EXTENSION}/modules/shelf/drop.js")"
    [[ -n "${shell_limit}" && "${shell_limit}" == "$(sed -n "s/^pub const ${limit}: usize = \(.*\);$/\1/p" /ctx/apps/shelf/src/model.rs)" ]]
done
grep -q "get_sandboxed_app_id" "${UI_EXTENSION}/modules/shelf.js"
grep -q "'/usr/lib/amethystora-shelf/amethystora-shelf'" "${UI_EXTENSION}/modules/shelf.js"
[[ -z "$(grep -rnE 'use_markup = true|use_markup: true|set_markup\(' "${UI_EXTENSION}/modules/shelf.js" \
    "${UI_EXTENSION}/modules/shelf")" ]]
grep -q "^NoDisplay=true" /usr/share/applications/amethystora-shelf.desktop
grep -q '^shelf STATE="":' /usr/share/amethystora/just/desktop.just
GSETTINGS_BACKEND=memory ame desktop shelf dock >/dev/null 2>&1 && false
GSETTINGS_BACKEND=memory ame desktop shelf edge >/dev/null
# Desks (R4-09; manual: desktop.md#desks): off for a new account and learning once on; the module, its files and recipe;
# it starts nothing, shows nothing as markup, and keeps its desks in the account's own folder
DESKS=org.gnome.shell.extensions.amethystora-ui.desks
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${DESKS}" | sort | paste -sd ' ')" == "desk-windows desks desks-learn" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${DESKS}" desks)" == false ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${DESKS}" desks-learn)" == true ]]
grep -q "\['desks', Desks\]" "${UI_EXTENSION}/extension.js"
for file in modules/desks.js modules/desks/apply.js modules/desks/card.js modules/desks/identity.js \
    modules/desks/state.js modules/desks/layouts.js modules/desks/layoutsdata.js stylesheets/desks.css; do
    test -s "${UI_EXTENSION}/${file}"
done
# Screens come back as you left them (R7-46; manual: desktop.md#screens-come-back-as-you-left-them): on; what it keeps has
# no GNOME in it, so node checks it (test-desk-layouts.mjs, with the saved sets); it blocks nothing, and it never reads a
# window's title, starts an app or closes one
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${DESKS}" desk-windows)" == true ]]
grep -qE "gi://|resource://" "${UI_EXTENSION}/modules/desks/layoutsdata.js" && false
[[ -z "$(grep -nE '_sync\(|file_get_contents|get_title|\.launch\(|open_new_window|\.delete\(|\.kill\(' \
    "${UI_EXTENSION}/modules/desks/layouts.js")" ]]
grep -qF "get('desks')?.layouts?.leaving()" "${UI_EXTENSION}/modules/tiling/engine.js"
grep -qF "layouts?.leaving();" "${UI_EXTENSION}/modules/windows/places.js"
# Turned off by any route, the windows' places go from the file; a write is refused by the file's etag when the recipe
# wrote it meanwhile
grep -qF 'forgetLayouts();' "${UI_EXTENSION}/modules/desks.js"
grep -qF 'WRONG_ETAG' "${UI_EXTENSION}/modules/desks/layouts.js"
# The flat battery's Not yet has a key of its own, bound only while its card shows; the card is after a call and a
# recording in the pill, whose controls are only there
grep -qF "Main.wm.addKeybinding(HOLD_KEY" "${UI_EXTENSION}/modules/power/flat.js"
grep -q "^export const ORDER = \['call', 'recording', 'flat'," "${UI_EXTENSION}/modules/pill/activities.js"
test -s /usr/share/amethystora/themed/shell-desks.css.tpl
[[ -z "$(grep -rnE "Subprocess|Util\.spawn|spawn_command_line|GLib\.spawn|use_markup = true|use_markup: true|set_markup\(" \
    "${UI_EXTENSION}/modules/desks.js" "${UI_EXTENSION}/modules/desks")" ]]
grep -q "GLib.get_user_config_dir(), 'amethystora'" "${UI_EXTENSION}/modules/desks/state.js"
grep -q '^desks \*ARGS:' /usr/share/amethystora/just/desktop.just
(
    export HOME="$(mktemp -d)" XDG_CONFIG_HOME=
    GSETTINGS_BACKEND=memory ame desktop desks list | grep -q .
    ame desktop desks forget 000000000000 >/dev/null 2>&1 && false
    ame desktop desks rename 000000000000 Office >/dev/null 2>&1 && false
    rm -rf "${HOME}"
)
# First start (manual: desktop.md#the-reveal-at-login, #tips, #the-app-grid, #gestures-and-keys). The reveal at login, a
# key of the transitions extension, its own file, imported; ame desktop transition reveal on|off
TRANSITIONS_EXTENSION=/usr/share/gnome-shell/extensions/amethystora-transitions@arsslensoft.github.io
# A field named as a method hides it: the reveal after every theme or wallpaper change threw once one did
grep -q 'this\._reveal = ' "${TRANSITIONS_EXTENSION}/extension.js" && false
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-transitions reveal)" == true ]]
test -s "${TRANSITIONS_EXTENSION}/reveal.js"
grep -q "^import {Reveal} from './reveal.js';" "${TRANSITIONS_EXTENSION}/extension.js"
grep -q "reactive: false" "${TRANSITIONS_EXTENSION}/reveal.js"
GSETTINGS_BACKEND=memory ame desktop transition reveal maybe >/dev/null 2>&1 && false
GSETTINGS_BACKEND=memory ame desktop transition reveal off >/dev/null
# Tips (the hints module) and the app grid's recent row (the appgrid module): one key each, on; the hints after the
# glass, whose contract they use, and the app grid after the dock; the tips' rule table and its seven owners
HINTS=org.gnome.shell.extensions.amethystora-ui.hints
APPGRID=org.gnome.shell.extensions.amethystora-ui.appgrid
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${HINTS}" | paste -sd ' ')" == enabled ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${HINTS}" enabled)" == true ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${APPGRID}" | sort | paste -sd ' ')" == "hidden icons names order recent" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${APPGRID}" recent)" == true ]]
[[ " $(sed -n "s/^    \['\([a-z]*\)', [A-Za-z]*\],$/\1/p" "${UI_EXTENSION}/extension.js" | paste -sd ' ') " == *" dock appgrid "*" glass hints "* ]]
grep -q "^export const KNOWN = \['shake', 'hold-super', 'search', 'snap', 'folder-colour', 'phone-drop', 'pinch'\];" \
    "${UI_EXTENSION}/modules/hints.js"
for owner in pointer.js:shake windows/hold.js:hold-super hints.js:search windows/snap.js:snap hints.js:folder-colour \
    connect.js:phone-drop windows/gestures.js:pinch; do
    grep -qE "offer(WhenClosed)?\('${owner#*:}'" "${UI_EXTENSION}/modules/${owner%%:*}"
done
test -s "${UI_EXTENSION}/icons/hints/amethystora-hint-symbolic.svg"
grep -q "Shell.AppUsage.get_default().get_most_used()" "${UI_EXTENSION}/modules/appgrid.js"
# No timer in the app grid but one: a single 300 ms settle after the icon size changes, which pins the grid at the
# recent row's size (1a46b20c), removed before it is set again and never repeating
[[ "$(grep -E 'timeout_add|setInterval|setTimeout' "${UI_EXTENSION}/modules/appgrid.js" | sed 's/^ *//')" == \
    'this._settle = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SETTLE, () => {' ]]
grep -q '^const SETTLE = 300;$' "${UI_EXTENSION}/modules/appgrid.js"
for recipe in hints app-grid; do
    grep -q "^${recipe} [A-Z]*=\"\"\( [A-Z]*=\"\"\)\{0,1\}:" /usr/share/amethystora/just/desktop.just
    grep -qx "desktop::${recipe}" <<<"$(ame --summary | tr ' ' '\n')"
done
GSETTINGS_BACKEND=memory ame desktop hints maybe >/dev/null 2>&1 && false
GSETTINGS_BACKEND=memory ame desktop app-grid recent maybe >/dev/null 2>&1 && false
GSETTINGS_BACKEND=memory ame desktop hints off >/dev/null
GSETTINGS_BACKEND=memory ame desktop app-grid recent off >/dev/null
# Your apps, your grid (plans/ui-ideas/spec-apps.md): hidden, renamed and re-iconed apps and the grid's order are keys
# the shell applies, never a launcher of the account's own; nothing hidden, renamed or sorted as the image ships, and the
# backup disk recognized. The helper takes only a launcher's ID, a pack that draws the app, and an order it knows.
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${APPGRID}" order)" == "'custom'" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${APPGRID}" hidden)" == "@as []" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${APPGRID}" names)" == "@a{ss} {}" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${APPGRID}" icons)" == "@a{ss} {}" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.backupdisk ask)" == true ]]
grep -qx "    \['backupdisk', BackupDisk\]," "${UI_EXTENSION}/extension.js"
test -x /usr/libexec/amethystora-app-look
GSETTINGS_BACKEND=memory /usr/libexec/amethystora-app-look hide ../evil >/dev/null 2>&1 && false
GSETTINGS_BACKEND=memory /usr/libexec/amethystora-app-look order sideways >/dev/null 2>&1 && false
GSETTINGS_BACKEND=memory /usr/libexec/amethystora-app-look icon org.gnome.Nautilus.desktop amethystora-nonesuch >/dev/null 2>&1 && false
GSETTINGS_BACKEND=memory /usr/libexec/amethystora-app-look hide org.gnome.Nautilus.desktop
[[ "$(GSETTINGS_BACKEND=memory /usr/libexec/amethystora-app-look list --json | jq -c .)" == '{"order":"custom","apps":[]}' ]]
for recipe in "app *ARGS" "tidy-downloads ACTION=\"\"" "backup-disk STATE=\"\""; do
    grep -qxF "${recipe}:" /usr/share/amethystora/just/desktop.just
done
grep -qxF "startup *ARGS:" /usr/share/amethystora/just/apps.just
# What starts with you: the helper switches only the account's own, never deletes, and its Off is exactly the shape the
# security watcher accepts (test-security-watch.sh plants both); its menu item in the grid only opens Security
test -x /usr/libexec/amethystora-startup
grep -qF 'HIDDEN = "[Desktop Entry]\nHidden=true\n"' /usr/libexec/amethystora-startup
grep -q "hidden_autostart" /usr/libexec/amethystora-security-watch
grep -qF "[SECURITY, \`apps/\${id}\`]" "${UI_EXTENSION}/modules/appgrid/looks.js"
grep -qE "ame security|amethystora-security-config|hardening" "${UI_EXTENSION}/modules/appgrid/looks.js" && false
# Templates seeded once, never executable; Downloads tidied by a user timer nobody has enabled
test -x /usr/share/amethystora/user-setup.hooks.d/28-templates.sh
grep -q "chmod 0644" /usr/share/amethystora/user-setup.hooks.d/28-templates.sh
test -x /usr/libexec/amethystora-tidy-downloads
test -s /usr/lib/systemd/user/amethystora-tidy-downloads.timer
test ! -e /etc/systemd/user/timers.target.wants/amethystora-tidy-downloads.timer
test ! -e /usr/lib/systemd/user/timers.target.wants/amethystora-tidy-downloads.timer
grep -q "rmtree\|os.remove(\|shutil.rmtree" /usr/libexec/amethystora-tidy-downloads && false
# The app grid's six folders, the image's default list, each filled from the apps' categories under a name GNOME
# translates from a menu directory file the image has; the folders the list used to name stay defined
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.desktop.app-folders folder-children)" == \
    "['Office', 'Graphics', 'Games', 'System', 'Development', 'Utilities']" ]]
for folder in Office Graphics Games System Development Utilities GamingUtilities Containers Wine Productivity; do
    grep -qx "\[org/gnome/desktop/app-folders/folders/${folder}\]" /etc/dconf/db/distro.d/01-amethystora-folders
done
while read -r directory; do
    test -s "/usr/share/desktop-directories/${directory}"
done < <(sed -n "s/^name='\(.*\.directory\)'$/\1/p" /etc/dconf/db/distro.d/01-amethystora-folders)
grep -A4 -x "\[org/gnome/desktop/app-folders/folders/System\]" /etc/dconf/db/distro.d/01-amethystora-folders |
    grep -q "^excluded-apps=.*'amethystora-security.desktop', 'amethystora-update.desktop', 'amethystora-control.desktop'"
# Pick a look, once, for a new account (user-setup.hooks.d/25-first-look.sh), on top of the Manual's welcome page,
# which the offer of classic windows then gives way to
test -x /usr/share/amethystora/user-setup.hooks.d/25-first-look.sh
grep -q "^version-script first-look user " /usr/share/amethystora/user-setup.hooks.d/25-first-look.sh
grep -q "/usr/bin/amethystora-control welcome$" /usr/share/amethystora/user-setup.hooks.d/25-first-look.sh
grep -q "amethystora/first-look.json" /usr/libexec/amethystora-layout-offer
grep -qF 'pub const WELCOME: &str = "welcome";' /ctx/apps/control/src/backend.rs
# ...brought to the front by Amethystora UI's firstlook module, which knows it by the hook's command line: started
# from a unit, it has no activation token, and a first login starts in the overview under GNOME's welcome dialog
grep -qx "const PAGE = 'welcome';" "${UI_EXTENSION}/modules/firstlook.js"
grep -qx "const CONTROL = 'org.amethystora.Control';" "${UI_EXTENSION}/modules/firstlook.js"
grep -qx "    \['firstlook', FirstLook\]," "${UI_EXTENSION}/extension.js"
# ...and before it the greeting (R7-24): on, one key, a layer of the shell that takes no window and moves no focus, a word for every
# language Amethystora speaks, and after firstlook in the order, whose page waits for it as for any modal dialog
HELLO_SCHEMA=org.gnome.shell.extensions.amethystora-ui.hello
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${HELLO_SCHEMA}")" == greet ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${HELLO_SCHEMA}" greet)" == true ]]
[[ "$(grep -oE "^    \['(comfort|firstlook|hello)'," "${UI_EXTENSION}/extension.js" | tr -d " [',")" == "$(printf '%s\n' comfort firstlook hello)" ]]
grep -q '^hello CHOICE="":' /usr/share/amethystora/just/desktop.just
env LD_LIBRARY_PATH=/apps-stage/usr/lib64 NODE_ICU_DATA="$(echo /apps-stage/usr/share/node-*/icudata)" /apps-stage/usr/bin/node /ctx/build_files/shared/test-hello.mjs \
    "${UI_EXTENSION}" /ctx/po
# How long it takes to start (R7-51): no timer and no always-on program, a date and a number and nothing else, each start at most once
bash /ctx/build_files/shared/test-start-times.sh
[[ "$(systemctl is-enabled amethystora-start-times.service)" == enabled ]]
grep -qx 'Type=exec' /usr/lib/systemd/system/amethystora-start-times.service
grep -qx 'RemainAfterExit=yes' /usr/lib/systemd/system/amethystora-start-times.service
grep -qx 'PrivateNetwork=yes' /usr/lib/systemd/system/amethystora-start-times.service
grep -qx 'RuntimeDirectoryPreserve=yes' /usr/lib/systemd/system/amethystora-start-times.service
test ! -e /usr/lib/systemd/system/amethystora-start-times.timer
grep -q '^start-times CHOICE="":' /usr/share/amethystora/just/system.just
# The Manual's moving cards: each clip a page shows is a WebM of at most 300 KB with its PNG poster of at most 40 KB,
# both ours (branding/cards.mjs)
MEDIA=/usr/share/amethystora/manual/media
while read -r card; do
    test -s "${MEDIA}/${card}.webm" && test -s "${MEDIA}/${card}.png"
    [[ "$(head -c 4 "${MEDIA}/${card}.webm" | od -An -tx1 | tr -d ' \n')" == 1a45dfa3 ]]
    (($(stat -c %s "${MEDIA}/${card}.webm") <= 300 * 1024 && $(stat -c %s "${MEDIA}/${card}.png") <= 40 * 1024))
done < <(grep -rhoE '\]\(media/[a-z0-9-]+\.webm\)' /usr/share/amethystora/manual | sed 's/^](media\///; s/\.webm)$//' | sort -u)
[[ "$(grep -rhoE '\]\(media/[a-z0-9-]+\.webm\)' /usr/share/amethystora/manual/desktop.md | sort -u | wc -l)" -eq 8 ]]
# The lock screen's clock and cards (manual: desktop.md#the-lock-screen), in GNOME Shell's own stylesheets: read with
# GIO, since gresource went with glib2-devel. Once each, the clock in Quicksand at its size, which is installed.
python3 -I - <<'EOF'
import gi
gi.require_version("Gio", "2.0")
from gi.repository import Gio
bundle = Gio.Resource.load("/usr/share/gnome-shell/gnome-shell-theme.gresource")
marker = "Amethystora: the lock screen clock and its cards"
for variant in ("dark", "light"):
    css = bundle.lookup_data(f"/org/gnome/shell/theme/gnome-shell-{variant}.css", 0).get_data().decode()
    assert css.count(marker) == 1, f"{variant}: the clock's block is there {css.count(marker)} times"
    clock = css[css.index(marker):]
    clock = clock[clock.index(".unlock-dialog-clock .unlock-dialog-clock-time {"):]
    clock = clock[:clock.index("}")]
    for line in ('font-family: "Quicksand";', "font-size: 116px;", "font-weight: 600;"):
        assert line in clock, f"{variant}: the clock has no {line}"
EOF
test -s /usr/share/fonts/amethystora/quicksand/QuicksandVariable.ttf

# The motion (manual: desktop.md#motion): the droplet and the workspace switch, two keys with their defaults, started
# after the tiling; GNOME's own handlers blocked and its methods called, never replaced; no timer; the droplet drawn
# through a Cogl pipeline of the window's own texture; and workspaces named after their apps, a key of the workspaces
MOTION=org.gnome.shell.extensions.amethystora-ui.motion
[[ "$(GSETTINGS_BACKEND=memory gsettings list-keys "${MOTION}" | sort | paste -sd ' ')" == "minimize open-close speed workspace-switch" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${MOTION}" minimize)" == "'droplet'" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get "${MOTION}" workspace-switch)" == "'depth'" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.workspaces auto-names)" == true ]]
[[ " $(sed -n "s/^    \['\([a-z]*\)', [A-Za-z]*\],$/\1/p" "${UI_EXTENSION}/extension.js" | paste -sd ' ') " == *" tiling "*" motion "* ]]
grep -q "signal_handler_block" "${UI_EXTENSION}/modules/motion.js"
grep -q "Clutter.PipelineNode" "${UI_EXTENSION}/modules/motion/droplet.js"
[[ -z "$(grep -E 'timeout_add|setInterval|setTimeout' "${UI_EXTENSION}"/modules/motion.js "${UI_EXTENSION}"/modules/motion/*.js)" ]]
[[ -z "$(grep -E 'Main\.wm\._[A-Za-z]+ *=[^=]|WindowManager\.prototype' "${UI_EXTENSION}"/modules/motion.js \
    "${UI_EXTENSION}"/modules/motion/*.js)" ]]
grep -qE "gi://|resource://" "${UI_EXTENSION}/modules/motion/path.js" && false
for recipe in minimize workspace-switch space-names sets; do
    grep -q "^${recipe} [A-Z]*=\"\":" /usr/share/amethystora/just/desktop.just
done
# Saved sets (manual: desktop.md#saved-sets): a switch of the workspaces', on by default. The file's shape and the
# strip's arithmetic have no GNOME in them, so node checks them against tampered files (test-sets.mjs); a set starts
# apps only through GNOME's list of apps, never a command, and shows names as text; the tiling, places and the motion
# ask the workspaces about a set's windows rather than placing them themselves.
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.workspaces sets)" == true ]]
grep -qE "gi://|resource://" "${UI_EXTENSION}/modules/workspaces/setsdata.js" && false
[[ -z "$(grep -E 'Util\.spawn|GLib\.spawn|Subprocess|spawn_command_line|use_markup *= *true' \
    "${UI_EXTENSION}"/modules/workspaces/*.js)" ]]
grep -qF "get('workspaces')?.claim?.(window)" "${UI_EXTENSION}/modules/tiling/engine.js"
grep -qF "get('workspaces')?.claimed?.(window)" "${UI_EXTENSION}/modules/windows/places.js"
grep -qF "get('workspaces')?.arriving?.(window)" "${UI_EXTENSION}/modules/motion.js"
"${NODE[@]}" /ctx/build_files/shared/test-sets.mjs
"${NODE[@]}" /ctx/build_files/shared/test-flat-battery.mjs
"${NODE[@]}" /ctx/build_files/shared/test-desk-layouts.mjs
test -s /usr/share/amethystora/themed/shell-motion.css.tpl
# The droplet's frames are the design board's at twice its size (plans/beyond/spec-motion.md, H1): path.js has no
# GNOME in it, so node works them out
MOTION_PATH="$(mktemp --suffix=.mjs)"
cp "${UI_EXTENSION}/modules/motion/path.js" "${MOTION_PATH}"
"${NODE[@]}" --input-type=module -e "
import * as P from '${MOTION_PATH}';
const geo = {W: 440, H: 300, R0: 24, S: 72, Fc: {x: 0, y: 0}, Ic: {x: 0, y: 586}};
const near = (a, b) => Math.abs(a - b) <= 2;
const at = (b, x, y, w, h = w) => near(b.centre.x, x) && near(b.centre.y, y) && near(b.w, w) && near(b.h, h);
const ok = at(P.minimizeAt(P.OUT.circle, geo), 0, 0, 216) && at(P.minimizeAt(P.OUT.small, geo), 0, 0, 140.4) &&
    at(P.minimizeAt(P.OUT.middle, geo), -128, 320, 85.4) && at(P.minimizeAt(P.OUT.landed, geo), 0, 586, 51.8) &&
    at(P.unminimizeAt(P.IN.shown, geo), 0, 586, 51.8) && at(P.unminimizeAt(P.IN.middle, geo), -96, 260, 153.9) &&
    at(P.unminimizeAt(P.IN.over, geo), 0, 0, 457.6, 312) && at(P.unminimizeAt(P.IN.under, geo), 0, 0, 435.6, 297) &&
    at(P.unminimizeAt(1, geo), 0, 0, 440, 300) && P.minimizeAt(1, geo).o === 0 && P.minimizeAt(0, geo).plain === 1;
if (!ok) process.exit(1);
"
rm -f "${MOTION_PATH}"

# The desktop styles (manual: themes.md#styles). The seven are the same in the theme library, the schema, the style
# module, the shell's stylesheet and the applications' files; floating is the default before a theme is applied
STYLE_JS="${UI_EXTENSION}/modules/style.js"
STYLE_SCHEMA="${UI_EXTENSION}/schemas/org.gnome.shell.extensions.amethystora-ui.style.gschema.xml"
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell.extensions.amethystora-ui.style style)" == "'floating'" ]]
grep -q '^DEFAULT_STYLE=floating$' /usr/lib/amethystora/theme/lib.sh
# shellcheck source=/dev/null
STYLE_NAMES="$(source /usr/lib/amethystora/theme/lib.sh && echo "${STYLES[*]}")"
[[ "$(grep -c '^    [a-z]*: {bar: ' "${STYLE_JS}")" == "$(wc -w <<<"${STYLE_NAMES}")" ]]
[[ "$(grep -c '<choice value=' "${STYLE_SCHEMA}")" == "$(wc -w <<<"${STYLE_NAMES}")" ]]
# The shape every style draws the applications in is one template, filled with the style's radii (STYLE_SHAPE in the
# library); floating has none and keeps the applications' own corners
test -s /usr/share/amethystora/themed/style-base.gtk.css.tpl
test -s /usr/share/amethystora/themed/style-floating.gtk.css.tpl
for style in ${STYLE_NAMES}; do
    grep -q "<choice value=\"${style}\"/>" "${STYLE_SCHEMA}"
    grep -q "#uiGroup\.amethystora-style-${style} " /usr/share/amethystora/themed/shell-styles.css.tpl
    # The corners the glass gives a window's pane are the ones the style's GTK stylesheet gives the window
    radius="$(sed -n "s/^    ${style}: {bar: '\(pane\|pills\|solid\|bare\)', window: \([0-9]*\)},$/\2/p" "${STYLE_JS}")"
    [[ -n "${radius}" ]]
    [[ "${style}" == floating ]] && continue
    # A quick settings tile with an arrow is one card: the rules that round every button would otherwise round both its parts
    grep -q "^#uiGroup\.amethystora-style-${style} \.quick-toggle-has-menu[ ,]" /usr/share/amethystora/themed/shell-styles.css.tpl
    [[ "${style}" == terminal ]] ||
        grep -q "^#uiGroup\.amethystora-style-${style} \.quick-toggle-has-menu \.quick-toggle-menu-button:ltr" \
            /usr/share/amethystora/themed/shell-styles.css.tpl
    shape="$(source /usr/lib/amethystora/theme/lib.sh && echo "${STYLE_SHAPE[${style}]}")"
    [[ " ${shape} " == *" window=${radius} "* ]]
done
# The terminal's own file is square and lined, and keeps a radio button and a face round
grep -q '^\* {$' /usr/share/amethystora/themed/style-terminal.gtk.css.tpl
grep -q '^radio,$' /usr/share/amethystora/themed/style-terminal.gtk.css.tpl
# Every value the shape template names, a style gives (s_toggle is the control's less the toggle group's padding)
(
    # shellcheck source=/dev/null
    source /usr/lib/amethystora/theme/lib.sh
    for key in $(grep -o '{{ s_[a-z]* }}' /usr/share/amethystora/themed/style-base.gtk.css.tpl | tr -d '{} ' | sort -u); do
        [[ "${key}" == s_toggle ]] && continue
        for style in "${!STYLE_SHAPE[@]}"; do
            [[ " ${STYLE_SHAPE[${style}]} " == *" ${key#s_}="* ]] || { echo "${style} gives no ${key}"; exit 1; }
        done
    done
)
# Every shipped theme names a style and a font that are there, and every font offered is installed
(
    # shellcheck source=/dev/null
    source /usr/lib/amethystora/theme/lib.sh
    for theme in /usr/share/amethystora/themes/*/; do
        is_style "$(head -n1 "${theme}style.theme")" || { echo "${theme}style.theme names no style"; exit 1; }
        font_installed "$(head -n1 "${theme}font.theme")" || { echo "${theme}font.theme is not installed"; exit 1; }
        # A theme's monospace font is optional (JetBrains Mono is the default), and installed when it names one
        if [[ -f "${theme}mono.theme" ]]; then
            font_installed "$(head -n1 "${theme}mono.theme")" || { echo "${theme}mono.theme is not installed"; exit 1; }
        fi
        # ...and so is the icon pack it names
        if [[ -f "${theme}icons.theme" ]]; then
            is_icon_pack "$(sed -e 's/^amethystora-//' "${theme}icons.theme")" || { echo "${theme}icons.theme names no pack"; exit 1; }
        fi
        # Every theme names its pointer, one of the image's: Halo for a dark theme, Ink for a light one, so that the
        # pointer is drawn by its body on its ground
        cursor="$(head -n1 "${theme}cursor.theme")"
        cursor_theme_installed "${cursor}" || { echo "${theme}cursor.theme names ${cursor}, which is not installed"; exit 1; }
        if [[ -f "${theme}light.mode" ]]; then
            [[ "${cursor}" == amethystora-pointer-ink ]] || { echo "${theme} is light and has a ${cursor} pointer"; exit 1; }
        else
            [[ "${cursor}" == amethystora-pointer-halo ]] || { echo "${theme} is dark and has a ${cursor} pointer"; exit 1; }
        fi
    done
    for family in "${UI_FONTS[@]}"; do
        font_installed "${family}" || { echo "ame desktop font offers ${family}, which is not installed"; exit 1; }
    done
    for family in "${MONO_FONTS[@]}"; do
        font_installed "${family}" || { echo "ame desktop mono-font offers ${family}, which is not installed"; exit 1; }
        # A fixed-width face: a terminal draws cells of one width, and a proportional one would blur them
        [[ "$(fc-list "${family}" spacing 2>/dev/null | grep -c 'spacing=100')" -gt 0 ]] ||
            { echo "${family} is not monospaced"; exit 1; }
    done
)
# The licences of the fonts copied from Google Fonts (09-fonts.sh) ship with them
for family_dir in /usr/share/fonts/amethystora/*/; do
    test -s "/usr/share/licenses/google-fonts/$(basename "${family_dir}")/OFL.txt"
done
# Which style and font are written: the account's own choice, else the theme's, else floating and Adwaita Sans
(
    unset XDG_CONFIG_HOME
    HOME="$(mktemp -d)"
    # shellcheck source=/dev/null
    source /usr/lib/amethystora/theme/lib.sh
    mkdir -p "${CURRENT_THEME_DIR}"
    [[ "$(desktop_style)" == floating && "$(ui_font)" == "Adwaita Sans" ]]
    echo neon >"${CURRENT_THEME_DIR}/style.theme"
    echo Inter >"${CURRENT_THEME_DIR}/font.theme"
    [[ "$(desktop_style)" == neon && "$(ui_font)" == Inter ]]
    echo bubbles >"${CONFIG_DIR}/style"
    echo "No Such Font" >"${CONFIG_DIR}/font"
    [[ "$(desktop_style)" == neon && "$(ui_font)" == Inter ]]
    echo terminal >"${CONFIG_DIR}/style"
    echo "JetBrains Mono" >"${CONFIG_DIR}/font"
    [[ "$(desktop_style)" == terminal && "$(ui_font)" == "JetBrains Mono" ]]
    # The monospace font: the account's, else the theme's, else JetBrains Mono; one that is not installed is passed over
    [[ "$(mono_font)" == "JetBrains Mono" ]]
    echo "Fira Code" >"${CURRENT_THEME_DIR}/mono.theme"
    [[ "$(mono_font)" == "Fira Code" ]]
    echo "No Such Font" >"${CONFIG_DIR}/mono-font"
    [[ "$(mono_font)" == "Fira Code" ]]
    echo "Space Mono" >"${CONFIG_DIR}/mono-font"
    [[ "$(mono_font)" == "Space Mono" ]]
    rm -rf "${HOME}"
)
# The theme hook is past the version that brought the styles and fonts, and the palette layer, the monospace fonts and
# the icon packs after them, so that an account set up before gets them
[[ "$(sed -n 's/^version-script theme user \([0-9]\+\).*/\1/p' \
    /usr/share/amethystora/user-setup.hooks.d/11-theme.sh)" -ge 13 ]]

# What the Hyprland era left behind is removed: per account by 09-retire-dms.sh, before 11-theme.sh
# re-applies the theme, and per machine by 20-retire-dms.sh. The account side keys on the names and
# signatures in DankMaterialShell's own source: its kitty.conf hides kitty's title bar (no close
# button on GNOME), its gtk.css kept the theme out of libadwaita, and its environment.d file still
# set variables for the whole session.
USER_RETIRE=/usr/share/amethystora/user-setup.hooks.d/09-retire-dms.sh
SYSTEM_RETIRE=/usr/share/amethystora/system-setup.hooks.d/20-retire-dms.sh
test -x "${USER_RETIRE}"
test -x "${SYSTEM_RETIRE}"
[[ "$(printf '%s\n' "${USER_RETIRE##*/}" 11-theme.sh | LC_ALL=C sort | head -n1)" == "${USER_RETIRE##*/}" ]]
grep -q "dank-colors" "${USER_RETIRE}"
grep -q "90-dms.conf" "${USER_RETIRE}"
# The machine side never touches a machine that has layered any of it back on, and removes the greeter
# account only by the home dms-greeter gave it
grep -q 'rpm -q "${package}"' "${SYSTEM_RETIRE}"
grep -q "== /var/lib/greeter" "${SYSTEM_RETIRE}"
# ...and nothing of that stack is in the image any more
for package in greetd dms dms-greeter quickshell hyprland; do
    rpm -q "${package}" >/dev/null 2>&1 && false
done
# Files opens on the grid, where the icon packs' folders are drawn as artwork rather than as a 16px row
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.nautilus.preferences default-folder-viewer)" == "'icon-view'" ]]
[[ "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.nautilus.icon-view default-zoom-level)" == "'large'" ]]

# ClamAV daemon listens on its socket and keeps retrying until freshclam has fetched the signatures
grep -q "^LocalSocket /run/clamd.scan/clamd.sock$" /etc/clamd.d/scan.conf
test -f /usr/lib/systemd/system/clamd@.service.d/10-amethystora.conf

# Security keys: pam_u2f ahead of the password for sudo and polkit. pam_u2f ignores its config file (and so
# the fixed origin keys are registered with) before 1.4.0, or when it is not root-owned or is writable.
grep -qE "^auth\s+sufficient\s+pam_u2f\.so" /etc/pam.d/system-auth
grep -q "^origin = pam://amethystora$" /etc/security/pam_u2f.conf
[[ "$(stat -c '%U %a' /etc/security/pam_u2f.conf)" == "root 644" ]]
[[ "$(printf '%s\n' 1.4.0 "$(rpm -q --queryformat '%{VERSION}' pam-u2f)" | sort -V | head -n1)" == 1.4.0 ]]

# Hardening (08-hardening.sh and its files in system_files)
# Updates must carry a valid signature from the key CI signs with
test -s /etc/pki/containers/amethystora.pub
jq -e '.transports.docker["ghcr.io/arsslensoft"][0] == {"type": "sigstoreSigned",
    "keyPath": "/etc/pki/containers/amethystora.pub", "signedIdentity": {"type": "matchRepository"}}' \
    /etc/containers/policy.json
grep -q "use-sigstore-attachments: true" /etc/containers/registries.d/amethystora.yaml
# openSUSE Leap's images are checked against openSUSE's container keys, from its openSUSE-build-key
# package: the one that signs them today and its 2023 successor, so that a rotation keeps them pulling.
# Leap only: the rest of openSUSE's namespace there holds build projects that sign with keys of their own.
jq -e '.transports.docker["registry.opensuse.org/opensuse/leap"][0] | .type == "sigstoreSigned"
    and (.keyPaths | length == 2) and .signedIdentity.type == "matchRepository"' /etc/containers/policy.json >/dev/null
jq -r '.transports.docker["registry.opensuse.org/opensuse/leap"][0].keyPaths[]' /etc/containers/policy.json |
    xargs -r -n1 openssl pkey -pubin -noout -in
jq -e '.transports.docker | has("registry.opensuse.org") or has("registry.opensuse.org/opensuse") | not' \
    /etc/containers/policy.json >/dev/null
grep -q "use-sigstore-attachments: true" /etc/containers/registries.d/registry.opensuse.org-opensuse-leap.yaml
# Every key the policy names is in the image: containers-common took Red Hat's from /etc/pki/rpm-gpg in 45,
# which would have refused every pull from Red Hat's registries
jq -r '.. | objects | (.keyPath // empty), (.keyPaths // empty | .[])' /etc/containers/policy.json |
    xargs -r -n1 test -s
# The policy is the one every update is verified against, so it has to be one this containers/image
# accepts, unknown keys and all: loading it is the first thing a copy does, before it opens anything
command -v skopeo >/dev/null
POLICY_CHECK="$(skopeo copy dir:/nonexistent "dir:$(mktemp -d)" 2>&1 || true)"
grep -qi "policy" <<<"${POLICY_CHECK}" && { echo "${POLICY_CHECK}"; false; }
# An image of your own built FROM Amethystora ends its build with this check, which this image passes
/usr/libexec/amethystora-image-check
# ...and `ame system trust-image` makes a machine, and that image, take it only with its owner's
# signature, in files that come out the same in both; never for Amethystora's own, never with a private key
grep -q "^trust-image REPOSITORY KEY:" /usr/share/amethystora/just/system.just
TRUST_ROOT="$(mktemp -d)"
mkdir -p "${TRUST_ROOT}/etc/containers"
cp /etc/containers/policy.json "${TRUST_ROOT}/etc/containers/"
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "${TRUST_ROOT}/cosign.key"
openssl pkey -in "${TRUST_ROOT}/cosign.key" -pubout -out "${TRUST_ROOT}/cosign.pub"
/usr/libexec/amethystora-trust-image --root "${TRUST_ROOT}" ghcr.io/someone/my-image "${TRUST_ROOT}/cosign.pub"
jq -e '.transports.docker["ghcr.io/someone/my-image"] == [{"type": "sigstoreSigned",
    "keyPath": "/etc/pki/containers/ghcr.io-someone-my-image.pub", "signedIdentity": {"type": "matchRepository"}}]
    and .transports.docker["ghcr.io/arsslensoft"][0].keyPath == "/etc/pki/containers/amethystora.pub"
    and .default == [{"type": "reject"}]' "${TRUST_ROOT}/etc/containers/policy.json" >/dev/null
cmp "${TRUST_ROOT}/cosign.pub" "${TRUST_ROOT}/etc/pki/containers/ghcr.io-someone-my-image.pub"
grep -qxF "  ghcr.io/someone/my-image:" "${TRUST_ROOT}/etc/containers/registries.d/ghcr.io-someone-my-image.yaml"
/usr/libexec/amethystora-trust-image --root "${TRUST_ROOT}" ghcr.io/arsslensoft/amethystora "${TRUST_ROOT}/cosign.pub" && false
/usr/libexec/amethystora-trust-image --root "${TRUST_ROOT}" ghcr.io/someone/other "${TRUST_ROOT}/cosign.key" && false
rm -rf "${TRUST_ROOT}"
# The rest of an image of your own, while it builds: its setup.json as every account's defaults (amethystora-setup
# bake, in test-setup.sh above) and its branding, whose login picture goes in with the helper 12-login-screen.sh
# put this image's in with. `file` tells its pictures apart.
test -x /usr/libexec/amethystora-login-background
command -v file >/dev/null
bash /ctx/build_files/shared/test-brand.sh
# ...and its policy: the agent's limits, Security's switches, GNOME's settings default or locked, the hardening a
# build can turn on and when it updates (amethystora-image-policy)
test -x /usr/libexec/amethystora-image-policy
systemctl cat uupd.timer | grep -q '^OnCalendar='
bash /ctx/build_files/shared/test-image-policy.sh
# The copy opens root's blob info cache before it fails, in /var/lib/containers/cache, and makes every
# directory on the way with the cache's own 0700, /var/lib too: see the check on /var/lib at the end
rm -rf /var/lib/containers
rmdir /var/lib 2>/dev/null || true
# Signature checking is kept on by a unit of its own, at every boot, once the network is up. It used to
# be a first-boot setup hook, whose runner reports success whatever a hook does, and the in-place switch
# that hook ran cannot work on a booted system, where /sysroot is read-only: it failed on every ISO
# install, once, without a word. The script explains all of that in comments that name the flag, so
# the check for the flag itself has to skip them.
test -x /usr/libexec/amethystora-signed-updates
grep -q "enforce-container-sigpolicy" /usr/libexec/amethystora-signed-updates
grep -vE "^[[:space:]]*#" /usr/libexec/amethystora-signed-updates | grep -q -- "--mutate-in-place" && false
grep -q "^After=network-online.target$" /usr/lib/systemd/system/amethystora-signed-updates.service
# ...and runs as uupd's unit does, so that bootc enters install_t itself: as a system service it copies itself
# into /tmp to re-execute, which ame security noexec-temp makes fail
grep -qx "SELinuxContext=system_u:unconfined_r:unconfined_t:s0" /usr/lib/systemd/system/amethystora-signed-updates.service
test -e /usr/share/amethystora/system-setup.hooks.d/20-signed-updates.sh && false
# bootc builds the next deployment from the image alone, so switching with it on a machine that has
# layered packages drops them at the next boot without a word. This unit runs at every boot, so it
# has to read the flag bootc sets for such a deployment and rebase with rpm-ostree instead.
grep -q '\.status\.booted\.incompatible' /usr/libexec/amethystora-signed-updates
grep -q "rpm-ostree rebase" /usr/libexec/amethystora-signed-updates
# The same command is offered to anyone following a dated tag back onto a stream
grep -q "rpm-ostree rebase ostree-image-signed" /usr/libexec/amethystora-security-status
# The sealed images (Containerfile.sealed) have no rpm-ostree: what shows the deployments reads them
# through amethystora-deployments, which asks rpm-ostree or bootc, whichever the image has. This image is
# not sealed, so it must not say it is.
test -x /usr/libexec/amethystora-deployments
for file in /usr/libexec/amethystora-security-status /usr/libexec/amethystora-update-alert \
    /ctx/apps/update/src/backend.rs; do
    grep -q /usr/libexec/amethystora-deployments "${file}"
done
test -e /usr/share/amethystora/sealed && false
# Everything the image claims to do, in one place, without a password, with nothing to dismiss. This is
# what makes Secure Boot and the rest surfaceable without interrupting anybody more than once. The
# recipe and the Security app read the one script, so the terminal and the window cannot tell two stories.
grep -q "^status:$" /usr/share/amethystora/just/security.just
grep -qx "    @/usr/libexec/amethystora-security-status" /usr/share/amethystora/just/security.just
/usr/libexec/amethystora-security-status >/dev/null
/usr/libexec/amethystora-security-status --json |
    jq -e '.checks | length > 0 and all(.group and .id and .state and .title and .text)' >/dev/null
# And as the security watcher runs it, a service with no HOME: the report stopped there, and the watcher, which
# then has no report, told nobody of anything
env -u HOME /usr/libexec/amethystora-security-status --json | jq -e '.checks | length > 0' >/dev/null
# Waiting firmware updates are said, and where fwupd cannot answer, as in a build, that is only information
/usr/libexec/amethystora-security-status --json | jq -e 'any(.checks[]; .id == "firmware" and .state == "info")' >/dev/null
grep -q "secure-boot-notified" /usr/libexec/amethystora-security-alert
# The recipe that enrolls the signing keys and the report that says whether they are enrolled ask the one
# helper. They used to check on their own and disagreed, because one of them counted a certificate it
# could not find as enrolled.
test -x /usr/libexec/amethystora-mok-status
grep -q "/usr/libexec/amethystora-mok-status" /usr/share/amethystora/just/security.just
grep -q "/usr/libexec/amethystora-mok-status" /usr/libexec/amethystora-security-status
grep -q "mokutil --test-key" /usr/share/amethystora/just/security.just && false
grep -q "mokutil --test-key" /usr/libexec/amethystora-security-status && false
# ...and what it checks has to be readable by the user asking. /etc/pki/akmods/certs is 0750
# root:akmods, so run without sudo every check there concluded the certificates did not exist.
SB_CERT=/usr/share/amethystora/secure-boot/amethystora-modules.der
[[ "$(stat -c '%a' "${SB_CERT}")" == 644 ]]
cmp -s /etc/pki/akmods/certs/amethystora-modules.der "${SB_CERT}"
# ...and in the app grid too, so it is not only behind a recipe name somebody has to already know. The
# Security app shows it in a window, where the terminal launcher it replaces used to print it.
SECURITY_DESKTOP=/usr/share/applications/amethystora-security.desktop
test ! -e /usr/share/applications/amethystora-security-status.desktop
test -s "${SECURITY_DESKTOP}"
command -v desktop-file-validate >/dev/null && desktop-file-validate "${SECURITY_DESKTOP}"
# An Exec or an Icon that resolves to nothing is a tile in the grid that does nothing, or a blank one
test -x "$(command -v "$(sed -n 's/^Exec=\([^ ]*\).*/\1/p' "${SECURITY_DESKTOP}" | head -n1)")"
test -e "/usr/share/icons/amethystora-glass/apps/scalable/$(sed -n 's/^Icon=//p' "${SECURITY_DESKTOP}").svg"
# Starting the whole-machine scan is the one thing its polkit rule lets through without a password, and
# only for the person at the machine
SCAN_RULE=/usr/share/polkit-1/rules.d/50-amethystora-virus-scan.rules
grep -q 'action.lookup("unit") == "amethystora-clamav-scan.service"' "${SCAN_RULE}"
grep -q 'action.lookup("verb") == "start"' "${SCAN_RULE}"
grep -q "subject.local && subject.active" "${SCAN_RULE}"
# Kernel settings, arguments and blocked modules
grep -q "^kernel.kptr_restrict = 2$" /usr/lib/sysctl.d/60-amethystora-hardening.conf
grep -q "^kernel.oops_limit = 100$" /usr/lib/sysctl.d/60-amethystora-hardening.conf
grep -q '"slab_nomerge"' /usr/lib/bootc/kargs.d/10-amethystora-hardening.toml
grep -q "^install rxrpc /usr/bin/false$" /usr/lib/modprobe.d/60-amethystora-hardening.conf
# What costs nothing is on: temporary IPv6 addresses, Bluetooth's privacy (08-hardening.sh), no SSH over
# vsock, and no setuid on what a desktop never runs
grep -q "^ipv6.ip6-privacy=2$" /usr/lib/NetworkManager/conf.d/30-amethystora-ip6-privacy.conf
[[ "$(readlink /etc/systemd/system-generators/systemd-ssh-generator)" == /dev/null ]]
for program in chfn chage gpasswd newgrp ksu mount.davfs; do
    if [[ -u "/usr/bin/${program}" ]]; then
        echo "/usr/bin/${program} is still setuid root"
        exit 1
    fi
done
# What the stricter switches write has to be what its program takes: chrony's servers, the away zone, the
# dconf keys. block-xwayland restates GNOME Shell's own command with --no-x11, so a change to it must show.
chronyd -p -f /usr/share/amethystora/hardening/chrony.conf >/dev/null
firewall-offline-cmd --get-zones | grep -qw amethystora-away
DCONF_TEST="$(mktemp -d)"
cp /usr/share/amethystora/hardening/dconf-automount /usr/share/amethystora/hardening/dconf-user-extensions "${DCONF_TEST}/"
dconf compile "${DCONF_TEST}.db" "${DCONF_TEST}"
rm -rf "${DCONF_TEST}" "${DCONF_TEST}.db"
grep -qx "ExecStart=/usr/bin/gnome-shell --mode=%i" /usr/lib/systemd/user/org.gnome.Shell@.service
# modprobe reports module names with underscores, whichever spelling the config file uses
MODPROBE_CONFIG="$(modprobe --showconfig)"
for module in dccp sctp rds tipc n_hdlc firewire_core firewire_sbp2 cramfs hfs vivid; do
    grep -q "^install ${module} /usr/bin/false$" <<<"${MODPROBE_CONFIG}"
done
# FireWire is a switch of its own (ame security firewire), for FireWire sound cards: its three modules in a file
# of their own, which turning it off masks, and none in the blocked modules' file, which would block it still
grep -qi firewire /usr/lib/modprobe.d/60-amethystora-hardening.conf && false
for module in firewire_core firewire_ohci firewire_sbp2; do
    grep -qx "install ${module} /usr/bin/false" /usr/lib/modprobe.d/61-amethystora-firewire.conf
done
jq -e '.[] | select(.name == "firewire") | .state == "on" and .default == "on"' \
    <<<"$(/usr/libexec/amethystora-hardening status --json)" >/dev/null
# What the machine sends by itself (ame security sends): every name in the helper's SENDS table is a switch it knows,
# and the three that stop Fedora's count, its connectivity check and the firmware catalogue are on
for switch in $(sed -n '/^declare -A SENDS=/,/)$/p' /usr/libexec/amethystora-hardening | grep -o '\[[a-z-]*\]=' | tr -d '[]='); do
    jq -e --arg name "${switch}" 'any(.[]; .name == $name and .sends != null)' <<<"$(/usr/libexec/amethystora-hardening status --json)" >/dev/null
done
for switch in countme connectivity-check firmware-metadata; do
    jq -e --arg name "${switch}" 'any(.[]; .name == $name and .state == "on" and .default == "on")' \
        <<<"$(/usr/libexec/amethystora-hardening status --json)" >/dev/null
done
# ...Fedora's count is the timer its preset enables, and the check is the file and address the manual names, so that a
# change of Fedora's fails the build rather than leaving the manual wrong
test -e /usr/lib/systemd/system/rpm-ostree-countme.timer
grep -qx 'uri=http://fedoraproject.org/static/hotspot.txt' /usr/lib/NetworkManager/conf.d/20-connectivity-fedora.conf
grep -q 'fedoraproject.org/static/hotspot.txt' /usr/share/amethystora/manual/security.md
grep -qx 'enabled=false' /usr/share/amethystora/hardening/60-amethystora-connectivity.conf
grep -qx 'HOMEBREW_NO_ANALYTICS=1' /etc/homebrew/brew.env
grep -q "^sends:" /usr/share/amethystora/just/security.just
grep -q "^location " /usr/share/amethystora/just/security.just
test -x /usr/libexec/amethystora-sends
/usr/libexec/amethystora-sends --json | jq -e 'length >= 6 and all(.[]; (.id | startswith("sends-")) and (.state | IN("on", "off")) and
    (.title | length > 0) and (.text | length > 0) and (.details | type == "array"))' >/dev/null
# The weather beside the clock is listed, and off as the image ships
/usr/libexec/amethystora-sends --json | jq -e 'any(.[]; .id == "sends-weather" and .state == "off")' >/dev/null
# No passwordless root for users who are not at the machine, and no user-writable directory in root's
# sudo PATH, and no world-writable USB devices
grep -q "<allow_any>no</allow_any>" /usr/share/polkit-1/actions/*privileged.user.setup.policy
grep -q "<allow_active>auth_admin</allow_active>" /usr/share/polkit-1/actions/*privileged.user.setup.policy
grep -q "polkit.Result.NO" /usr/share/polkit-1/rules.d/10-amethystora-privileged-setup.rules
test -x /usr/bin/amethystora-privileged-setup
# A rule that names an action no policy defines never applies, and whatever it meant to allow or refuse is
# left to that action's defaults: every action a rule the image ships names has to be one a policy defines
# (pkexec checks the action an exec.path annotation names, not org.freedesktop.policykit.exec)
POLKIT_ACTIONS="$(grep -ohE '<action id="[^"]+"' /usr/share/polkit-1/actions/*.policy | cut -d'"' -f2)"
for rules in /usr/share/polkit-1/rules.d/*amethystora* /usr/share/polkit-1/rules.d/20-privileged-user-setup.rules; do
    for action in $(grep -ohE 'action\.id ?===? ?"[^"]+"' "${rules}" | cut -d'"' -f2); do
        grep -qxF "${action}" <<<"${POLKIT_ACTIONS}" || { echo "${rules} names ${action}, which no policy defines"; false; }
    done
done
# rpm-ostree: only checking for updates and updating go without a password. No rule may let a program that
# runs as an administrator roll the machine back to an older build or change its packages unasked, as the
# rule fedora-release ships did (Amethystora's own replaces it, under the same name)
grep -rlE 'rpmostree1\.(install-uninstall-packages|install-local-packages|rollback|cleanup|deploy|rebase|override|bootconfig|finalize-deployment|cancel|reload-daemon)' \
    /usr/share/polkit-1/rules.d /etc/polkit-1/rules.d 2>/dev/null && false
grep -q "^// Amethystora's own" /usr/share/polkit-1/rules.d/org.projectatomic.rpmostree1.rules
for program in $(grep -ohE 'policykit\.exec\.path">[^<]+' /usr/share/polkit-1/actions/*.policy | cut -d'>' -f2); do
    for rules in /usr/share/polkit-1/rules.d/*.rules; do
        if grep -qF '"org.freedesktop.policykit.exec"' "${rules}" && grep -qF "\"${program}\"" "${rules}"; then
            echo "${rules} names ${program} under org.freedesktop.policykit.exec, which pkexec never checks for it"
            false
        fi
    done
done
grep -E "^Defaults[[:space:]]+secure_path" /etc/sudoers | grep -q linuxbrew && false
# Homebrew's folder is the first account's: a shell runs its programs on starting (brew shellenv, completion
# scripts, uutils ahead of /usr/bin) only for that account, never for root or anyone else
for file in /etc/profile.d/*.sh /etc/zsh/zshrc /usr/share/fish/vendor_conf.d/*.fish; do
    if grep -qE 'shellenv|bash_completion\.d/\*|site-functions\)|uubin' "${file}"; then
        grep -qE -- '-O (/home/linuxbrew/\.linuxbrew|"\$\{_brew_prefix\}"|\$brew_prefix)' "${file}" ||
            { echo "${file} runs Homebrew's programs without checking whose they are"; false; }
    fi
done
if [[ -f /usr/lib/udev/rules.d/50-zsa.rules ]]; then
    grep -q 'MODE:="0666"' /usr/lib/udev/rules.d/50-zsa.rules && false
    grep -q 'TAG+="uaccess"' /usr/lib/udev/rules.d/50-zsa.rules
fi
# A keyboard's hidraw device carries every key typed on it, and the devices under a vendor ID include the
# keyboards built on its chips: no rule the image adds gives either to every account, as 92-viia.rules,
# 60-arduino-mbed.rules and 50-zsa.rules did before they gave them to the user at the seat (uaccess)
for rules in /usr/lib/udev/rules.d/*.rules; do
    rpm -qf --quiet "${rules}" && continue
    if grep -nE '(hidraw|idVendor).*MODE:?="0?[0-7]{2}[1-7]"|MODE:?="0?[0-7]{2}[1-7]".*(hidraw|idVendor)' "${rules}"; then
        echo "${rules} gives a device to every account"
        false
    fi
done
# /dev/uinput makes a virtual keyboard that types into any window: no rule the image ships gives it to the
# seat, Solaar's included. `ame security virtual-input off` writes the one that does, for Steam Input and
# OpenTabletDriver.
grep -rlE 'KERNEL=="uinput".*uaccess' /usr/lib/udev/rules.d /etc/udev/rules.d && false
grep -q 'KERNEL=="uinput".*uaccess' /usr/share/amethystora/hardening/71-amethystora-uinput.rules
/usr/libexec/amethystora-hardening is-on virtual-input
# Firewall: the Amethystora zone, without Fedora Workstation's open port range, and the phone link's ports (firewalld's
# kdeconnect service, named for the protocol) open only in the zone of the networks named home
[[ "$(firewall-offline-cmd --get-default-zone)" == "amethystora" ]]
[[ -z "$(firewall-offline-cmd --zone=amethystora --list-ports)" ]]
firewall-offline-cmd --zone=amethystora --query-service=kdeconnect >/dev/null && false
firewall-offline-cmd --zone=amethystora-home --query-service=kdeconnect >/dev/null
firewall-offline-cmd --zone=trusted --query-interface=tailscale0
# Account lockout, and the SSH server's settings for when it is turned on
grep -q "pam_faillock.so" /etc/pam.d/system-auth
grep -q "^deny = 10$" /etc/security/faillock.conf
# ...and fingerprint login last, where the installer puts it (08-hardening.sh)
[[ "$(tail -n1 /etc/authselect/authselect.conf)" == "with-fingerprint" ]]
grep -q "^PermitRootLogin no$" /etc/ssh/sshd_config.d/40-amethystora-hardening.conf
grep -qE "^[[:space:]]*Include[[:space:]]+/etc/ssh/sshd_config\.d/\*\.conf" /etc/ssh/sshd_config

# Parental Controls' allowed hours at the text consoles and over SSH (08-hardening.sh): pam_malcontent once
# in login and sshd, failing open, skipped by administrators, and before the include, which the sssd and
# winbind profiles end early for local accounts. Never in GDM's services, where GNOME's lock keeps the session.
rpm -q malcontent-pam >/dev/null
test -e /usr/lib64/security/pam_malcontent.so
for service in login sshd; do
    [[ "$(grep -c 'pam_malcontent\.so' "/etc/pam.d/${service}")" == 1 ]]
    awk '/^account[[:space:]]+\[success=1 default=ignore\][[:space:]]+pam_succeed_if\.so quiet user ingroup wheel$/ { skip = NR }
        /^-account[[:space:]]+\[success=ok auth_err=die default=ignore\][[:space:]]+pam_malcontent\.so$/ { line = NR }
        /^account[[:space:]]+include[[:space:]]/ && !include { include = NR }
        END { exit !(skip && line == skip + 1 && include == line + 1) }' "/etc/pam.d/${service}"
done
# GDM's own services, which are in /usr/lib/pam.d from GDM 51 (Fedora 45)
test -f /etc/pam.d/gdm-password || test -f /usr/lib/pam.d/gdm-password
grep -qs pam_malcontent /etc/pam.d/gdm-* /usr/lib/pam.d/gdm-* && false

# Domain login (Settings' Enterprise Login, `realm join`): every SSSD back end, among them the file realmd
# looks for before an Active Directory join, and no PackageKit for realmd to look for one through
rpm -q sssd sssd-ad sssd-ipa sssd-krb5 sssd-ldap >/dev/null
test -e /usr/libexec/sssd/gpo_child
grep -qx "automatic-install = no" /etc/realmd.conf
for command in sssd-enable-logins:sssd sssd-disable-logins:image winbind-enable-logins:winbind \
    winbind-disable-logins:image; do
    grep -qx "${command%%:*} = /usr/libexec/amethystora-domain-logins ${command#*:}" /etc/realmd.conf
done
# ...whose login commands keep the image's authselect features, in the image's order, all of which both
# profiles realmd picks from have. The image's own setup is not a domain's, which the watcher reports.
DOMAIN_LOGINS=/usr/libexec/amethystora-domain-logins
test -x "${DOMAIN_LOGINS}"
AUTHSELECT_CONF="$(grep -vE '^[[:space:]]*(#|$)' /etc/authselect/authselect.conf)"
mapfile -t AUTHSELECT_FEATURES < <(tail -n +2 <<<"${AUTHSELECT_CONF}")
for feature in with-faillock with-fingerprint with-pam-u2f; do
    printf '%s\n' "${AUTHSELECT_FEATURES[@]}" | grep -qx "${feature}"
done
for profile in sssd winbind; do
    [[ "$("${DOMAIN_LOGINS}" --print "${profile}")" == \
        "authselect select ${profile} ${AUTHSELECT_FEATURES[*]} with-mkhomedir --force" ]]
    for feature in "${AUTHSELECT_FEATURES[@]}"; do
        authselect list-features "${profile}" | grep -qx "${feature}"
    done
done
[[ "$("${DOMAIN_LOGINS}" --print image)" == \
    "authselect select $(head -n1 <<<"${AUTHSELECT_CONF}") ${AUTHSELECT_FEATURES[*]} --force" ]]
"${DOMAIN_LOGINS}" check && false
# ...takes the setup its own command makes for a domain, and not one with a line added by hand afterwards,
# which `authselect check` calls valid. Tried on the image's own /etc/authselect, which comes back as it was.
DOMAIN_TEST="$(mktemp -d)"
cp -a /etc/authselect "${DOMAIN_TEST}/before"
read -ra DOMAIN_SELECT <<<"$("${DOMAIN_LOGINS}" --print sssd)"
"${DOMAIN_SELECT[@]}" --nobackup >/dev/null
"${DOMAIN_LOGINS}" check
echo "auth sufficient pam_permit.so" >>/etc/authselect/system-auth
"${DOMAIN_LOGINS}" check && false
authselect select "$(head -n1 <<<"${AUTHSELECT_CONF}")" "${AUTHSELECT_FEATURES[@]}" --force --nobackup >/dev/null
diff -r "${DOMAIN_TEST}/before" /etc/authselect
rm -rf "${DOMAIN_TEST}"
# A domain's accounts are in no /etc/passwd and numbered far above 65534: the per-account checks find
# accounts by their home folders as well
for file in /usr/libexec/amethystora-security-watch /usr/libexec/amethystora-audit-home-rules \
    /usr/libexec/amethystora-notify-users; do
    grep -q -- "-lt 65534" "${file}" && false
done
grep -qF '/var/home/*/' /usr/libexec/amethystora-security-watch
grep -qF '/var/home/*/' /usr/libexec/amethystora-audit-home-rules

# One-time codes for SSH (ame security ssh-codes): off, and when on, exactly the shipped drop-in and one line
# right after the password, which sshd takes as a key, or a password and a code. On and off are tried on
# the image's own files, which have to come back as they were, and the watcher accepts what `on` wrote.
rpm -q google-authenticator >/dev/null
test -e /usr/lib64/security/pam_google_authenticator.so
SSH_CODES=/usr/libexec/amethystora-ssh-codes
SSH_DROP_IN=/etc/ssh/sshd_config.d/30-amethystora-ssh-codes.conf
test -x "${SSH_CODES}"
[[ "$("${SSH_CODES}" status)" == off ]]
test ! -e "${SSH_DROP_IN}"
/usr/libexec/amethystora-security-status --json |
    jq -e 'any(.checks[]; .id == "ssh-codes" and .state == "off")' >/dev/null
matchpathcon -n /var/home/test/.google_authenticator | grep -q ":auth_home_t:"
SSH_TEST="$(mktemp -d)"
cp /etc/pam.d/sshd "${SSH_TEST}/sshd.pam"
"${SSH_CODES}" on
[[ "$("${SSH_CODES}" status)" == on ]]
cmp -s "/usr/share/amethystora/ssh-codes/${SSH_DROP_IN##*/}" "${SSH_DROP_IN}"
[[ "$(diff "${SSH_TEST}/sshd.pam" /etc/pam.d/sshd | grep -c '^[<>]')" == 1 ]]
grep -A1 -E "^auth[[:space:]]+substack[[:space:]]+password-auth" /etc/pam.d/sshd | tail -n1 |
    grep -qE "^auth[[:space:]]+required[[:space:]]+pam_google_authenticator\.so$"
cmp -s /etc/pam.d/sshd <("${SSH_CODES}" pam <"${SSH_TEST}/sshd.pam")
# The build has no host key of its own; sshd keeps the first value it reads, so the drop-in's are the ones
ssh-keygen -q -t ed25519 -N '' -f "${SSH_TEST}/host_key"
# Keywords in lower case, as OpenSSH printed them before 10.6 (Fedora 45), which prints them as written
SSHD_SETTINGS="$(sshd -T -h "${SSH_TEST}/host_key" | awk '{ $1 = tolower($1) } 1')"
for setting in "usepam yes" "kbdinteractiveauthentication yes" "passwordauthentication no" \
    "authenticationmethods publickey keyboard-interactive" "permitrootlogin no"; do
    grep -qx "${setting}" <<<"${SSHD_SETTINGS}"
done
"${SSH_CODES}" off
[[ "$("${SSH_CODES}" status)" == off ]]
cmp -s "${SSH_TEST}/sshd.pam" /etc/pam.d/sshd
test ! -e "${SSH_DROP_IN}"
rm -rf "${SSH_TEST}"

# The duress finger (ame security duress): off, its service not enabled, and no folder. The image's
# authselect profiles take its feature in the two files 08-hardening.sh copied and in no other, and the
# feature, tried on the image's own /etc/authselect, adds the option to those two lines only and comes off
# leaving every file as it was.
DURESS=/usr/libexec/amethystora-duress
test -x "${DURESS}"
# ...on the image's own fprintd (packaging/fprintd), whose source package travels with it
"${DURESS}" supported
[[ "$(rpm -q --queryformat '%{RELEASE}\n' fprintd fprintd-pam | grep -c '\.amethystora\.')" == 2 ]]
test -s "/usr/src/amethystora/$(rpm -q --queryformat '%{SOURCERPM}' fprintd-pam)"
test -x /usr/libexec/amethystora-duress-service
python3 -m py_compile /usr/libexec/amethystora-duress-service
[[ "$("${DURESS}" status)" == off ]]
[[ "$(systemctl is-enabled amethystora-duress.service 2>/dev/null)" == "enabled" ]] && false
test ! -e /etc/amethystora/duress.d
grep -qx "LogLevelMax=emerg" /usr/lib/systemd/system/amethystora-duress.service
test -f /usr/share/dbus-1/system.d/org.amethystora.Duress.conf
for example in /usr/share/amethystora/duress/examples/*; do bash -n "${example}"; done
for profile in local sssd winbind; do
    for file in /usr/share/authselect/vendor/"${profile}"/*; do
        case "${file##*/}" in
            fingerprint-auth)
                [[ "$(grep -c 'with-duress-fingerprint' "${file}")" == 1 ]] ;;
            system-auth)
                [[ "$(grep -c 'with-duress-fingerprint' "${file}")" == 1 ]]
                [[ "$(grep -c 'with-duress-password' "${file}")" == 1 ]] ;;
            password-auth)
                [[ "$(grep -c 'with-duress-password' "${file}")" == 1 ]] ;;
            nsswitch.conf) ;;
            *) [[ -L "${file}" ]] ;;
        esac
    done
done
authselect check >/dev/null
DURESS_TEST="$(mktemp -d)"
cp -a /etc/authselect "${DURESS_TEST}/before"
authselect enable-feature with-duress-fingerprint >/dev/null
[[ "$("${DURESS}" status)" == on ]]
[[ "$("${DURESS}" status-fingerprint)" == on ]]
grep -qE "^auth[[:space:]]+sufficient[[:space:]]+pam_fprintd\.so duress=deny$" /etc/pam.d/system-auth
grep -qE "^auth[[:space:]]+\[success=done default=bad\][[:space:]]+pam_fprintd\.so duress=allow$" /etc/pam.d/fingerprint-auth
[[ "$(diff -r "${DURESS_TEST}/before" /etc/authselect | grep -c '^[<>]')" == 5 ]]
authselect check >/dev/null
# (the helper's own `authselect` drift check reads /usr/etc/authselect, this image's committed /etc, which
# exists only at runtime, not during the build; the VM suite exercises it there)
authselect disable-feature with-duress-fingerprint >/dev/null
diff -r "${DURESS_TEST}/before" /etc/authselect
rm -rf "${DURESS_TEST}"

# The duress password (ame security duress): off by default, and when turned on it adds one pam_exec line
# after pam_unix in system-auth and password-auth and nowhere else, coming off leaving every file as it was.
# Its helper runs as pam_exec would and hashes with yescrypt, both tried without touching PAM.
test -x /usr/libexec/amethystora-duress-pam
python3 -m py_compile /usr/libexec/amethystora-duress-pam
test -e /usr/lib64/security/pam_exec.so
[[ "$("${DURESS}" status-password)" == off ]]
"${DURESS}" password-supported
# The helper's own hash (yescrypt) must verify with crypt(3), as the login path does; a wrong one must not
DURESS_HASH="$(printf 'swordfish' | /usr/libexec/amethystora-duress-pam hash)"
grep -q '^\$y\$' <<<"${DURESS_HASH}"
python3 - "${DURESS_HASH}" <<'PY'
import ctypes, sys
h = sys.argv[1].encode()
lib = ctypes.CDLL("libcrypt.so.2")
lib.crypt.restype = ctypes.c_char_p
lib.crypt.argtypes = (ctypes.c_char_p, ctypes.c_char_p)
assert lib.crypt(b'swordfish', h) == h, "right password did not verify"
assert lib.crypt(b'wrong', h) != h, "wrong password verified"
PY
# act() on a stand-in duress.d (no system bus at build time, so the action request is a harmless no-op)
DURESS_PW_DIR="$(mktemp -d)"
chmod 0700 "${DURESS_PW_DIR}"
printf '%s %s\n' "$(id -u)" "${DURESS_HASH}" >"${DURESS_PW_DIR}/passwords"
chmod 0600 "${DURESS_PW_DIR}/passwords"
duress_act() {
    local rc=0
    printf '%s' "$1" | AMETHYSTORA_DURESS_DIR="${DURESS_PW_DIR}" PAM_USER="$(id -un)" PAM_SERVICE="$2" \
        /usr/libexec/amethystora-duress-pam || rc=$?
    echo "${rc}"
}
# a wrong password is ignored everywhere
[[ "$(duress_act nope sudo)" == 1 ]]
# the right password unlocks at the graphical login, and is refused like a wrong one everywhere else
[[ "$(duress_act swordfish gdm-password)" == 0 ]]
[[ "$(duress_act swordfish sudo)" == 1 ]]
# a non-root caller is refused even with the right password and an allow-service: the file is root's and
# the action service is root-only, so verification trusts nothing a non-root process could have reached
DURESS_RC=0
printf 'swordfish' | setpriv --reuid=nobody --regid=nobody --clear-groups \
    env AMETHYSTORA_DURESS_DIR="${DURESS_PW_DIR}" PAM_USER="$(id -un)" PAM_SERVICE=gdm-password \
    /usr/libexec/amethystora-duress-pam || DURESS_RC=$?
[[ "${DURESS_RC}" == 1 ]]
# a downgraded hash is refused even when it genuinely matches the password: only yescrypt is honoured
DURESS_SHA512="$(python3 - <<'PY'
import ctypes
l = ctypes.CDLL("libcrypt.so.2"); l.crypt.restype = ctypes.c_char_p; l.crypt.argtypes = (ctypes.c_char_p,) * 2
l.crypt_gensalt.restype = ctypes.c_char_p
l.crypt_gensalt.argtypes = (ctypes.c_char_p, ctypes.c_ulong, ctypes.c_char_p, ctypes.c_int)
print(l.crypt(b"swordfish", l.crypt_gensalt(b"$6$", 0, None, 0)).decode())
PY
)"
grep -q '^\$6\$' <<<"${DURESS_SHA512}"
printf '%s %s\n' "$(id -u)" "${DURESS_SHA512}" >"${DURESS_PW_DIR}/passwords"
chmod 0600 "${DURESS_PW_DIR}/passwords"
[[ "$(duress_act swordfish gdm-password)" == 1 ]]
# a file readable by others is ignored, as pam_fprintd ignores a loose finger file
printf '%s %s\n' "$(id -u)" "${DURESS_HASH}" >"${DURESS_PW_DIR}/passwords"
chmod 0644 "${DURESS_PW_DIR}/passwords"
[[ "$(duress_act swordfish gdm-password)" == 1 ]]
rm -rf "${DURESS_PW_DIR}"
# the feature toggles cleanly in the authselect profiles
DURESS_PW_TEST="$(mktemp -d)"
cp -a /etc/authselect "${DURESS_PW_TEST}/before"
authselect enable-feature with-duress-password >/dev/null
[[ "$("${DURESS}" status-password)" == on ]]
[[ "$("${DURESS}" status)" == on ]]
for f in system-auth password-auth; do
    grep -qxE "auth[[:space:]]+\[success=done default=ignore\][[:space:]]+pam_exec\.so quiet expose_authtok /usr/libexec/amethystora-duress-pam[[:space:]]*" "/etc/pam.d/${f}"
done
# one line added to each of the two files, and the feature to authselect.conf
[[ "$(diff -r "${DURESS_PW_TEST}/before" /etc/authselect | grep -c '^[<>]')" == 3 ]]
authselect check >/dev/null
# both features on at once still leave a valid stack and come off cleanly
authselect enable-feature with-duress-fingerprint >/dev/null
authselect check >/dev/null
authselect disable-feature with-duress-fingerprint >/dev/null
authselect disable-feature with-duress-password >/dev/null
diff -r "${DURESS_PW_TEST}/before" /etc/authselect
rm -rf "${DURESS_PW_TEST}"

# Encrypted DNS (ame security dns) and a different address on each network (ame security mac): off, and
# when on, exactly the files their helpers write, which NetworkManager reads as meant. On and off are
# tried here, where neither systemd-resolved nor NetworkManager runs, and /etc has to come back as it was.
DNS=/usr/libexec/amethystora-dns
MAC=/usr/libexec/amethystora-mac
RESOLVED_DROP_IN=/etc/systemd/resolved.conf.d/60-amethystora-dns.conf
test -x "${DNS}"
test -x "${MAC}"
[[ "$("${DNS}" status)" == off && "$("${MAC}" status)" == off ]]
# The helper reloads systemd-resolved and never restarts it, which would forget the DNS settings
# NetworkManager and Tailscale gave each network: its unit has to be able to reload
grep -qxE "Type=notify-reload|ExecReload=.+" /usr/lib/systemd/system/systemd-resolved.service
grep -qE "^[^#]*(reload-or-restart|restart systemd-resolved)" "${DNS}" && false
# ...and changes each connected network's running connection, since reapplying an unchanged one
# rebuilds nothing. What offers the pause runs in the session, not from NetworkManager's dispatcher.
grep -qE "^[^#]*nmcli device reapply" "${DNS}" && false
grep -qE "^[^#]*nmcli device modify" "${DNS}"
test -L /etc/systemd/user/graphical-session.target.wants/amethystora-dns-portal.service
# The offer stays on the screen with its button until answered: GNOME Shell shows a normal one collapsed
grep -qE -- "--urgency=critical --wait" "${DNS}"
compgen -G "/usr/lib/NetworkManager/dispatcher.d/*amethystora*" >/dev/null && false
compgen -G "/etc/NetworkManager/dispatcher.d/*amethystora*" >/dev/null && false
test ! -e "${RESOLVED_DROP_IN}"
test ! -e /etc/NetworkManager/conf.d/60-amethystora-dns.conf
test ! -e /etc/NetworkManager/conf.d/60-amethystora-mac.conf
[[ "$(systemctl is-enabled amethystora-dns-resume.timer 2>/dev/null)" == "enabled" ]] && false
/usr/libexec/amethystora-security-status --json |
    jq -e '[.checks[] | select(.id == "encrypted-dns" or .id == "network-address") | .state] == ["off", "off"]' >/dev/null
RESOLVED_DIR_EXISTED=false
[[ -d "${RESOLVED_DROP_IN%/*}" ]] && RESOLVED_DIR_EXISTED=true
"${DNS}" on quad9
[[ "$("${DNS}" status)" == on && "$("${DNS}" name)" == Quad9 ]]
grep -qx "DNSOverTLS=yes" "${RESOLVED_DROP_IN}"
grep -qx "Domains=~." "${RESOLVED_DROP_IN}"
grep -q "^DNS=9.9.9.9#dns.quad9.net " "${RESOLVED_DROP_IN}"
# --print-config prints each key as key=value with nothing before it (main.c, nm_config_data_log)
/usr/sbin/NetworkManager --print-config | grep -qx "connection.dns-over-tls=0"
"${DNS}" on custom "192.0.2.1#dns.example.net" "2001:db8::1#dns.example.net"
[[ "$("${DNS}" name)" == dns.example.net ]]
"${DNS}" on custom "192.0.2.1" && false
"${DNS}" on custom "192.0.2.1#not a name" && false
"${DNS}" on nobody && false
"${DNS}" off
[[ "$("${DNS}" status)" == off ]]
test ! -e "${RESOLVED_DROP_IN}"
test ! -e /etc/NetworkManager/conf.d/60-amethystora-dns.conf
[[ "${RESOLVED_DIR_EXISTED}" == true ]] || rmdir "${RESOLVED_DROP_IN%/*}"
"${MAC}" on
[[ "$("${MAC}" status)" == on ]]
NETWORKMANAGER_CONFIG="$(/usr/sbin/NetworkManager --print-config)"
grep -qx "wifi.cloned-mac-address=stable" <<<"${NETWORKMANAGER_CONFIG}"
grep -qx "ipv4.dhcp-send-hostname=0" <<<"${NETWORKMANAGER_CONFIG}"
# DHCPv6's identity is otherwise made from the machine-id, the same on every network
grep -qx "ipv6.dhcp-duid=stable-uuid" <<<"${NETWORKMANAGER_CONFIG}"
# ...and temporary IPv6 addresses are ame security ipv6-privacy's alone, which this must not turn back on
grep -q "ip6-privacy" /usr/share/amethystora/network/60-amethystora-mac.conf && false
grep -qx "connection.llmnr=0" <<<"${NETWORKMANAGER_CONFIG}" && false
# Away from home only: the same, and no LLMNR lookups, in the file that takes the one place of the other's;
# the networks named home are given their own values by `home`, which the home networks' list calls
MAC_NETWORK=/usr/share/amethystora/network
[[ "$(diff <(grep -v '^#' "${MAC_NETWORK}/60-amethystora-mac.conf") <(grep -v '^#' "${MAC_NETWORK}/60-amethystora-mac-away.conf") |
    grep '^[<>]')" == "> connection.llmnr=0" ]]
grep -q "status|on|away|off|home" "${MAC}"
grep -q "amethystora-mac home" /usr/libexec/amethystora-hardening
"${MAC}" away
[[ "$("${MAC}" status)" == away ]]
/usr/sbin/NetworkManager --print-config | grep -qx "connection.llmnr=0"
/usr/sbin/NetworkManager --print-config | grep -qx "ethernet.cloned-mac-address=stable"
"${MAC}" on
[[ "$("${MAC}" status)" == on ]]
"${MAC}" off
[[ "$("${MAC}" status)" == off ]]
test ! -e /etc/NetworkManager/conf.d/60-amethystora-mac.conf
test ! -e /var/lib/amethystora/mac-home

# Developer groups: one list, which the recipe, Control and the report read, and nothing joins anyone to
# them by itself. Docker, Incus and libvirt are root under another name, which the report says.
DEV_GROUPS=/usr/share/amethystora/developer-groups
[[ "$(awk '!/^#/ && $2 == "root" { print $1 }' "${DEV_GROUPS}" | paste -sd' ' -)" == "docker incus-admin libvirt" ]]
for file in /usr/libexec/amethystora-control-list /usr/libexec/amethystora-security-status \
    /usr/share/amethystora/just/system.just; do
    grep -qF "${DEV_GROUPS}" "${file}"
done
grep -rlIE "usermod .*-a?G|gpasswd -a" /usr/lib/systemd/system /usr/libexec /usr/share/amethystora/*.hooks.d && false

# fail2ban: only the sshd jail is on, it reads the journal, and it bans through firewalld.
# `fail2ban-client -d` is the dump Lynis (TOOL-5104) reads the jails from, so what it shows here is
# what Lynis sees on the installed system. The action is the image's own, which bans in each of
# Amethystora's zones; 08-hardening.sh checks it names all three.
test -f /etc/fail2ban/jail.d/10-amethystora.conf
F2B_DUMP="$(fail2ban-client -d)"
grep -q "'sshd'" <<<"${F2B_DUMP}"
grep -q "'systemd'" <<<"${F2B_DUMP}"
grep -qF "'addaction', 'amethystora-zones'" <<<"${F2B_DUMP}"
grep -qF 'firewall-cmd --zone=$z --add-rich-rule=' <<<"${F2B_DUMP}"
# The ban database's directory is created at every boot, not only shipped in the image: /var reaches a
# machine once, when it is installed, so on a machine installed before fail2ban was in the image it
# never existed, and fail2ban exited with 255 straight after "Server ready". The path created has to
# be the one fail2ban.conf actually names.
grep -q "^d /var/lib/fail2ban " /usr/lib/tmpfiles.d/fail2ban-var.conf
grep -qE "^dbfile = /var/lib/fail2ban/" /etc/fail2ban/fail2ban.conf
# Lynis audits the machine against the image's profile (ame security audit); it reads every
# .prf in /etc/lynis, so the image's settings only apply while Lynis finds this one
test -f /etc/lynis/default.prf
grep -q "^machine-role=workstation$" /etc/lynis/custom.prf
lynis show profiles | grep -F "/etc/lynis/custom.prf" >/dev/null

# Flatpak: X11, the input devices and the Flatpak service are taken away from every application,
# whatever its own manifest asks for. Flatseal is how they are granted back, one application at a time.
# Flatpak reads system overrides from /var/lib/flatpak/overrides alone, so without the link the file in
# /etc is never read at all, which it was not for as long as the image shipped it without one.
FLATPAK_OVERRIDES=/etc/flatpak/overrides/global
grep -q "^sockets=!x11;!fallback-x11;$" "${FLATPAK_OVERRIDES}"
grep -q "^devices=!input;$" "${FLATPAK_OVERRIDES}"
grep -q "^org.freedesktop.Flatpak=none$" "${FLATPAK_OVERRIDES}"
grep -qxF "L /var/lib/flatpak/overrides/global - - - - ${FLATPAK_OVERRIDES}" /usr/lib/tmpfiles.d/amethystora-flatpak-overrides.conf
test -f /usr/share/flatpak/preinstall.d/flatseal.preinstall
# ...and the Flatpak service is given back by name to the apps the image installs that cannot work without it
# (Mission Center's gatherer did not start), each linked where Flatpak reads it and allowed for the report
grep -qxF "org.freedesktop.Flatpak=talk" /usr/share/amethystora/flatpak-overrides/io.missioncenter.MissionCenter
for override in /usr/share/amethystora/flatpak-overrides/*; do
    grep -qxF "org.freedesktop.Flatpak=talk" "${override}" || continue
    grep -qxF "L /var/lib/flatpak/overrides/${override##*/} - - - - ${override}" /usr/lib/tmpfiles.d/*.conf
    grep -qxF "${override##*/} flatpak" /usr/share/amethystora/allow.d/app-permissions
done

# systemd's console size query hangs the boot under Plymouth, until Fedora's has systemd/systemd#43802
grep -q '"systemd.tty.term.console=dumb"' /usr/lib/bootc/kargs.d/*.toml
# ...which rpm-ostree, updating a machine with layered packages, ignores: the system setup adds it there once,
# without restarting, unless the machine has it or is sealed. Tried against a stand-in rpm-ostree
CONSOLE_TEST="$(mktemp -d)"
mkdir "${CONSOLE_TEST}/bin"
printf '#!/usr/bin/bash\necho "$*" >>"${CALLS}"\n' >"${CONSOLE_TEST}/bin/rpm-ostree"
chmod +x "${CONSOLE_TEST}/bin/rpm-ostree"
console_hook() {
    echo "BOOT_IMAGE=/vmlinuz root=UUID=0 rhgb quiet $2" >"${CONSOLE_TEST}/cmdline"
    SETUP_CHECKER_FILE="${CONSOLE_TEST}/$1.json" AMETHYSTORA_CMDLINE="${CONSOLE_TEST}/cmdline" \
        AMETHYSTORA_SEALED="${CONSOLE_TEST}/sealed" CALLS="${CONSOLE_TEST}/calls" \
        PATH="${CONSOLE_TEST}/bin:${PATH}" bash /usr/share/amethystora/system-setup.hooks.d/27-console-query.sh
}
console_hook lacking ""
console_hook lacking ""
[[ "$(cat "${CONSOLE_TEST}/calls")" == "kargs --append-if-missing=systemd.tty.term.console=dumb" ]]
console_hook present systemd.tty.term.console=dumb
touch "${CONSOLE_TEST}/sealed"
console_hook sealed ""
[[ "$(cat "${CONSOLE_TEST}/calls")" == "kargs --append-if-missing=systemd.tty.term.console=dumb" ]]
rm -rf "${CONSOLE_TEST}"
# Only a program's own debugger writes over read-only memory through /proc/PID/mem: the image's argument, on
# until turned off, which the system setup adds the same way unless the owner turned it off. A machine that
# had turned on the extra argument it used to be loses that line of hardening-on.
grep -q '"proc_mem.force_override=ptrace"' /usr/lib/bootc/kargs.d/10-amethystora-hardening.toml
PROC_MEM_TEST="$(mktemp -d)"
mkdir -p "${PROC_MEM_TEST}/bin" "${PROC_MEM_TEST}/etc/amethystora"
printf '#!/usr/bin/bash\necho "$*" >>"${CALLS}"\n' >"${PROC_MEM_TEST}/bin/rpm-ostree"
chmod +x "${PROC_MEM_TEST}/bin/rpm-ostree"
proc_mem_hook() {
    echo "BOOT_IMAGE=/vmlinuz root=UUID=0 rhgb quiet $2" >"${PROC_MEM_TEST}/cmdline"
    SETUP_CHECKER_FILE="${PROC_MEM_TEST}/$1.json" AMETHYSTORA_CMDLINE="${PROC_MEM_TEST}/cmdline" \
        AMETHYSTORA_SEALED="${PROC_MEM_TEST}/sealed" AMETHYSTORA_HARDENING_ROOT="${PROC_MEM_TEST}" \
        CALLS="${PROC_MEM_TEST}/calls" PATH="${PROC_MEM_TEST}/bin:${PATH}" \
        bash /usr/share/amethystora/system-setup.hooks.d/28-proc-mem.sh
}
printf 'kernel strict\nkernel-args proc_mem.force_override=ptrace\n' >"${PROC_MEM_TEST}/etc/amethystora/hardening-on"
proc_mem_hook lacking ""
proc_mem_hook lacking ""
[[ "$(cat "${PROC_MEM_TEST}/calls")" == "kargs --append-if-missing=proc_mem.force_override=ptrace" ]]
[[ "$(cat "${PROC_MEM_TEST}/etc/amethystora/hardening-on")" == "kernel strict" ]]
proc_mem_hook present proc_mem.force_override=always
echo "kernel-args proc_mem.force_override=ptrace" >"${PROC_MEM_TEST}/etc/amethystora/hardening-off"
proc_mem_hook turned-off ""
touch "${PROC_MEM_TEST}/sealed"
echo "kernel-args proc_mem.force_override=ptrace" >"${PROC_MEM_TEST}/etc/amethystora/hardening-on"
proc_mem_hook sealed ""
[[ "$(cat "${PROC_MEM_TEST}/calls")" == "kargs --append-if-missing=proc_mem.force_override=ptrace" ]]
[[ ! -e "${PROC_MEM_TEST}/etc/amethystora/hardening-on" ]]
rm -rf "${PROC_MEM_TEST}"

# The ssh the live ISO's Anaconda added to the Amethystora zone goes once, unless sshd is on or the zone has more of the owner's
INSTALLER_SSH_TEST="$(mktemp -d)"
mkdir -p "${INSTALLER_SSH_TEST}/etc/firewalld/zones" "${INSTALLER_SSH_TEST}/usr/lib/firewalld/zones"
cp /usr/lib/firewalld/zones/amethystora.xml "${INSTALLER_SSH_TEST}/usr/lib/firewalld/zones/"
installer_ssh_hook() {
    sed 's#</zone>#'"$2"'\n</zone>#' /usr/lib/firewalld/zones/amethystora.xml >"${INSTALLER_SSH_TEST}/etc/firewalld/zones/amethystora.xml"
    SETUP_CHECKER_FILE="${INSTALLER_SSH_TEST}/$RANDOM.json" AMETHYSTORA_FIREWALL_ROOT="${INSTALLER_SSH_TEST}" AMETHYSTORA_SSHD_ENABLED="$1" \
        bash /usr/share/amethystora/system-setup.hooks.d/31-installer-ssh.sh
}
installer_ssh_hook no '<service name="ssh"/>'
[[ ! -e "${INSTALLER_SSH_TEST}/etc/firewalld/zones/amethystora.xml" ]]
installer_ssh_hook no '<service name="ssh"/>\n<service name="http"/>'
grep -q 'name="http"' "${INSTALLER_SSH_TEST}/etc/firewalld/zones/amethystora.xml"
[[ "$(grep -c 'name="ssh"' "${INSTALLER_SSH_TEST}/etc/firewalld/zones/amethystora.xml")" == 0 ]]
installer_ssh_hook yes '<service name="ssh"/>'
grep -q 'name="ssh"' "${INSTALLER_SSH_TEST}/etc/firewalld/zones/amethystora.xml"
rm -rf "${INSTALLER_SSH_TEST}"

# Kernel lockdown: on everywhere except the NVIDIA images, whose driver is a machine-owner-key module
# that a machine with Secure Boot turned off would no longer be able to load
LOCKDOWN_KARGS=/usr/lib/bootc/kargs.d/11-amethystora-lockdown.toml
if [[ "${IMAGE_NAME}" =~ nvidia ]]; then
    test -e "${LOCKDOWN_KARGS}" && false
else
    grep -q '"lockdown=integrity"' "${LOCKDOWN_KARGS}"
fi
# ...and with Secure Boot on, everywhere: Fedora's kernel locks itself down then, which is all that keeps
# the NVIDIA images, and a machine whose owner took the argument off (ame security lockdown), locked down.
# A kernel built without it would leave them open with nothing said.
KERNEL_CONFIGS=(/usr/lib/modules/*/config)
test -f "${KERNEL_CONFIGS[0]}"
for config in "${KERNEL_CONFIGS[@]}"; do
    for option in CONFIG_SECURITY_LOCKDOWN_LSM CONFIG_SECURITY_LOCKDOWN_LSM_EARLY CONFIG_LOCK_DOWN_IN_EFI_SECURE_BOOT; do
        grep -qx "${option}=y" "${config}"
    done
done
# What the file-watch sensor's BPF needs (plans/done/PLAN-audit-cost.md section 4.8). The sensor's own
# --probe is the real gate and falls back to the audit rules per boot when a kernel cannot take it, so a
# missing option here is a heads-up, not a wall.
for option in CONFIG_DEBUG_INFO_BTF CONFIG_BPF_SYSCALL CONFIG_BPF_JIT CONFIG_FUNCTION_TRACER \
    CONFIG_DYNAMIC_FTRACE_WITH_DIRECT_CALLS CONFIG_BPF_EVENTS CONFIG_AUDIT CONFIG_SECURITY; do
    grep -qx "${option}=y" "${KERNEL_CONFIGS[0]}" || echo "::warning::sensor: ${option} is not =y in the kernel"
done

# Key remapping runs as root and reads every input device, so it ships installed and switched off.
# `ame security input-remapper` is how someone who remaps keys turns it on.
rpm -q input-remapper >/dev/null
[[ "$(systemctl is-enabled input-remapper.service 2>/dev/null)" == "enabled" ]] && false
# The same for Android apps, whose container runs as root, and for the sched-ext CPU schedulers, which
# replace the kernel's own: installed, and off until `ame apps android` or `ame system scheduler`. Games
# reach gamemoded through its portal, from Steam's sandbox (ame apps gaming).
rpm -q waydroid gamemode scx-scheds scx-tools >/dev/null
[[ "$(systemctl is-enabled waydroid-container.service 2>/dev/null)" == "enabled" ]] && false
[[ "$(systemctl is-enabled scx_loader.service 2>/dev/null)" == "enabled" ]] && false
test ! -e /etc/scx_loader/config.toml
# The loader sees each kernel's build configuration where libbpf looks for it (04-packages.sh)
for config in /usr/lib/modules/*/config; do
    cmp -s "${config}" "/usr/lib/amethystora/kconfig/config-$(basename "$(dirname "${config}")")"
done
grep -qx 'BindReadOnlyPaths=/usr/lib/amethystora/kconfig:/boot' \
    /usr/lib/systemd/system/scx_loader.service.d/50-amethystora-kconfig.conf
# Same for USB protection, which blocks every device that was not present when it was set up: useless
# as a default, because the first boot would block the keyboard nobody had allowed yet
[[ "$(systemctl is-enabled usbguard.service 2>/dev/null)" == "enabled" ]] && false
# Its lock-screen mode (ame security usb locked) is GNOME's, through USBGuard's D-Bus service, which stays
# masked otherwise: D-Bus would start it, and every session's plugin would then allow each new device while
# the screen is unlocked, under `on` too. The polkit rule grants the plugin its three calls and no other, and
# the GNOME keys it reads are set and locked, so that no account turns it off from Settings.
rpm -q usbguard-dbus >/dev/null
[[ "$(systemctl is-enabled usbguard-dbus.service 2>/dev/null)" == masked ]]
USB_RULE=/usr/share/polkit-1/rules.d/50-amethystora-usb-lockscreen.rules
[[ "$(grep -v '^//' "${USB_RULE}" | grep -o 'org\.usbguard[A-Za-z0-9.]*' | sort | paste -sd ' ')" == \
    "org.usbguard.Devices1.applyDevicePolicy org.usbguard.Policy1.appendRule org.usbguard1.setParameter" ]]
grep -v '^//' "${USB_RULE}" | grep -q 'subject.local && subject.active'
grep -qx 'usb-protection=true' /etc/dconf/db/distro.d/07-amethystora-usb-protection
grep -qx "usb-protection-level='lockscreen'" /etc/dconf/db/distro.d/07-amethystora-usb-protection
grep -qx /org/gnome/desktop/privacy/usb-protection /etc/dconf/db/distro.d/locks/02-amethystora-usb-protection
grep -qx /org/gnome/desktop/privacy/usb-protection-level /etc/dconf/db/distro.d/locks/02-amethystora-usb-protection
/usr/libexec/amethystora-security-status --json |
    jq -e 'any(.checks[]; .id == "usb-protection" and .state == "off")' >/dev/null
# The private folder that closes when the screen locks (ame security private-folder), an account's own and
# none as the image ships: gocryptfs, one helper, and a watcher that nothing enables, which a desktop session's
# end stops and which closes the folder however it ends
rpm -q gocryptfs >/dev/null
test -x /usr/libexec/amethystora-private-folder
grep -q "^private-folder " /usr/share/amethystora/just/security.just
PRIVATE_UNIT=/usr/lib/systemd/user/amethystora-private-folder-watch.service
grep -q '^\[Install\]' "${PRIVATE_UNIT}" && false
grep -qx 'PartOf=graphical-session.target' "${PRIVATE_UNIT}"
grep -qx 'ExecStopPost=/usr/libexec/amethystora-private-folder close' "${PRIVATE_UNIT}"
command -v desktop-file-validate >/dev/null && desktop-file-validate /usr/share/applications/amethystora-private-folder.desktop
/usr/libexec/amethystora-security-status --json |
    jq -e 'any(.checks[]; .id == "private-folder" and .state == "off")' >/dev/null
bash /ctx/build_files/shared/test-private-folder.sh
# And for browser protection, the extension list and the block on web pages reaching hardware. The
# image ships Firefox unaltered, so the policy is only data here, and `ame security browser`
# writes it on the machine that asks: for Chromium-based browsers and for Firefox, each in its own
# form. Neither installs an extension, it only allows some
BROWSER_POLICY=/usr/libexec/amethystora-browser-policy
test -x "${BROWSER_POLICY}"
[[ "$("${BROWSER_POLICY}" status)" == off ]]
grep -q "^browser " /usr/share/amethystora/just/security.just
/usr/libexec/amethystora-security-status --json |
    jq -e 'any(.checks[]; .id == "browser-protection" and .state == "off")' >/dev/null
"${BROWSER_POLICY}" show chromium |
    jq -e '.ExtensionInstallBlocklist == ["*"] and (.ExtensionInstallAllowlist | length > 0)
        and (has("ExtensionSettings") or has("ExtensionInstallForcelist") | not)' >/dev/null
# Bitwarden's extension works with the Vault (Chrome Web Store and Edge Add-ons have an ID each)
"${BROWSER_POLICY}" show chromium |
    jq -e '(.ExtensionInstallAllowlist | index("nngceckbapebfimnlniiiahkandclblb") and index("jbkfoedolllekgbhcbcoahefnbanhhlh"))' >/dev/null
"${BROWSER_POLICY}" show firefox | jq -e '.policies.ExtensionSettings | has("{446900e4-71c2-419f-a6a7-df9c091e268b}")' >/dev/null
for guard in DefaultWebHidGuardSetting DefaultWebUsbGuardSetting DefaultSerialGuardSetting DefaultWebBluetoothGuardSetting; do
    "${BROWSER_POLICY}" show chromium | jq -e ".${guard} == 2" >/dev/null
done
"${BROWSER_POLICY}" show firefox |
    jq -e 'has("amethystora") and (.policies.ExtensionSettings["*"].allowed_types | index("extension") | not)
        and ([.policies.ExtensionSettings[] | .installation_mode // "allowed"] | all(. == "allowed"))' >/dev/null
# Site checking (Osprey) is offered only: allowed once chosen, never as the image ships, and never installed
"${BROWSER_POLICY}" show chromium | jq -e '.ExtensionInstallAllowlist | index("jmnpibhfpmpfjhhkmpadlbgjnbhpjgnd") | not' >/dev/null
OSPREY_CONF="$(mktemp)"
echo "OSPREY=on" >"${OSPREY_CONF}"
AMETHYSTORA_SECURITY_CONF="${OSPREY_CONF}" "${BROWSER_POLICY}" show chromium |
    jq -e '(.ExtensionInstallAllowlist | index("jmnpibhfpmpfjhhkmpadlbgjnbhpjgnd") and index("nopglhplnghfhpniofkcopmhbjdonlgn"))
        and (has("ExtensionInstallForcelist") | not)' >/dev/null
AMETHYSTORA_SECURITY_CONF="${OSPREY_CONF}" "${BROWSER_POLICY}" show firefox |
    jq -e '.policies.ExtensionSettings["osprey@foulest.net"].installation_mode == "allowed"' >/dev/null
rm -f "${OSPREY_CONF}"
# Data collection off is a part of its own: off as the image ships, the report says so, it installs and sets
# nothing, and the image carries none of the files it writes
[[ "$("${BROWSER_POLICY}" status privacy)" == off ]]
/usr/libexec/amethystora-security-status --json |
    jq -e 'any(.checks[]; .id == "browser-privacy" and .state == "off")' >/dev/null
test ! -e /etc/amethystora/browser-privacy
find /etc -name '11-amethystora-privacy.json' | grep -q . && false
"${BROWSER_POLICY}" show chromium-privacy |
    jq -e '.MetricsReportingEnabled == false and .DiagnosticData == 1 and .BraveP3AEnabled == false
        and (has("ExtensionInstallForcelist") or has("ExtensionInstallAllowlist") or has("HomepageLocation") | not)' >/dev/null
"${BROWSER_POLICY}" show firefox-privacy |
    jq -e '.policies.DisableTelemetry == true and .policies.DisableFirefoxStudies == true
        and .policies.FirefoxHome.Locked == false and .policies.FirefoxSuggest.Locked == false
        and (.policies | has("Extensions") or has("ExtensionSettings") or has("Homepage") or has("NewTabPage")
            or has("OverrideFirstRunPage") | not)' >/dev/null
# ...together with protection's, in the one file Firefox reads
AMETHYSTORA_BROWSER_PRIVACY=/dev/null "${BROWSER_POLICY}" show firefox |
    jq -e '.policies | has("ExtensionSettings") and .DisableTelemetry == true and (has("Homepage") | not)' >/dev/null
bash /ctx/build_files/shared/test-browser-policy.sh

# Audit rules: the watches that ship with the image, one file per key, which the boot-time generator writes
# into /etc/audit/rules.d for each key not switched off, with the per-user ones, which can only be written
# on a machine that already has home directories. The image ships none of them in /etc.
test ! -e /etc/audit/rules.d/60-amethystora.rules
# ...and nothing turns syscall auditing off under them, as the audit package's own audit.rules does
grep -rqE '^-a[[:space:]]+(task,never|never,task)' /etc/audit/rules.d && false
AUDIT_SHIPPED=/usr/share/amethystora/audit
AUDIT_RULES="$(cat "${AUDIT_SHIPPED}"/*.rules)"
grep -q "^-w /etc/flatpak/overrides/ -p wa -k hardening$" <<<"${AUDIT_RULES}"
grep -q "^-w /etc/containers/policy.json -p wa -k hardening$" <<<"${AUDIT_RULES}"
# A watch needs its path to be there when the rules load, and this one is the Firefox package's
grep -q "^-w /etc/firefox/ -p wa -k hardening$" <<<"${AUDIT_RULES}"
test -d /etc/firefox
grep -q "dir=/dev/input" <<<"${AUDIT_RULES}"
test -x /usr/libexec/amethystora-audit-home-rules
# One line augenrules cannot parse stops every rule in the directory from loading, which would leave
# the machine silently unaudited. auditctl cannot be asked about it during a container build, where
# there is no audit netlink socket to talk to, so the shape of each rule is what gets checked.
grep -vE '^[[:space:]]*(#|$)' <<<"${AUDIT_RULES}" | grep -qvE '^-(w|a) ' && false
# Every file is one key's, named after it, and that key is a switch (amethystora-security-config): a key
# that is off loads no rule, from the next restart
for rules in "${AUDIT_SHIPPED}"/*.rules; do
    key="$(basename "${rules}" .rules)"
    grep -vE '^[[:space:]]*(#|$)' "${rules}" | grep -qvE -- "-k ${key}$" && false
    /usr/libexec/amethystora-security-config "$(tr 'a-z-' 'A-Z_' <<<"${key}")" >/dev/null
done

# The key the update policy names, not only the policy file that names it: a watch on policy.json
# alone watches the lock and not the key.
for path in /etc/pki/containers/ /etc/containers/registries.d/ /etc/pki/akmods/certs/; do
    grep -q "^-w ${path} -p wa -k hardening$" <<<"${AUDIT_RULES}"
done

# How the rule set ends. augenrules concatenates every .rules file in this directory in name order and
# loads the result, so this reproduces that concatenation, with what the generator writes at boot for
# the keys as the image ships them, and checks it, rather than checking the files one at a time: a -D
# ("delete every rule loaded so far") in a file that sorts after ours would erase the whole lot, which is
# why 08-hardening.sh renames the file the audit package ships.
AUDIT_BOOT="$(mktemp -d)"
mkdir -p "${AUDIT_BOOT}/etc/audit/rules.d" "${AUDIT_BOOT}/var/home"
: >"${AUDIT_BOOT}/etc/passwd"
AMETHYSTORA_AUDIT_ROOT="${AUDIT_BOOT}" bash /usr/libexec/amethystora-audit-home-rules
AUDIT_CONCAT="$(find /etc/audit/rules.d "${AUDIT_BOOT}/etc/audit/rules.d" -name '*.rules' -printf '%f\t%p\n' |
    LC_ALL=C sort | cut -f2 | while read -r rules; do cat "${rules}"; done)"
rm -rf "${AUDIT_BOOT}"
AUDIT_DIRECTIVES="$(grep -vE '^[[:space:]]*(#|$)' <<<"${AUDIT_CONCAT}" || true)"
# Immutable, and it is the last word
[[ "$(tail -n1 <<<"${AUDIT_DIRECTIVES}")" == "-e 2" ]]
# The audit backlog may never make a process wait: a watch on /dev/input would stop the session
grep -q -- "--backlog_wait_time 0" <<<"${AUDIT_DIRECTIVES}"
LAST_DELETE="$(grep -n '^-D$' <<<"${AUDIT_DIRECTIVES}" | tail -n1 | cut -d: -f1 || true)"
FIRST_WATCH="$(grep -n '^-w ' <<<"${AUDIT_DIRECTIVES}" | head -n1 | cut -d: -f1 || true)"
[[ -n "${FIRST_WATCH}" ]]
[[ -z "${LAST_DELETE}" || "${LAST_DELETE}" -lt "${FIRST_WATCH}" ]]

# The file-watch sensor's side of the generator (plans/done/PLAN-audit-cost.md). The run above had no
# BPF to probe, so it fell back to the audit rules, which the FIRST_WATCH check confirmed. Forced on, the
# generator writes one -a never,task instead of every syscall rule, and the same watches into a watch list.
SENSOR_BOOT="$(mktemp -d)"
mkdir -p "${SENSOR_BOOT}/etc/audit/rules.d" "${SENSOR_BOOT}/etc/amethystora" "${SENSOR_BOOT}/var/home" "${SENSOR_BOOT}/run/sensor"
: >"${SENSOR_BOOT}/etc/passwd"
# exec-watch ships off, so turn it on here to exercise the exec-tree conversion as well
echo "EXEC_WATCH=on" >"${SENSOR_BOOT}/etc/amethystora/security.conf"
AMETHYSTORA_AUDIT_ROOT="${SENSOR_BOOT}" AMETHYSTORA_SENSOR_RUN="${SENSOR_BOOT}/run/sensor" \
    AMETHYSTORA_SECURITY_CONF="${SENSOR_BOOT}/etc/amethystora/security.conf" \
    AMETHYSTORA_SENSOR_FORCE=sensor bash /usr/libexec/amethystora-audit-home-rules
SENSOR_SYS="${SENSOR_BOOT}/etc/audit/rules.d/60-amethystora.rules"
# The only directive loaded is the task filter; no -w and no -S syscall rule
grep -qx -- "-a never,task" "${SENSOR_SYS}"
grep -vE '^[[:space:]]*(#|$)' "${SENSOR_SYS}" | grep -qvxE -- '-a never,task' && false
# The watch list carries the folders and files, from the same shipped rules, and the plan gives every key
grep -qx 'tree hardening /etc/sysctl.d' "${SENSOR_BOOT}/run/sensor/watch.list"
grep -qx 'file identity /etc/passwd' "${SENSOR_BOOT}/run/sensor/watch.list"
grep -qx 'exec-tree exec-watch /tmp' "${SENSOR_BOOT}/run/sensor/watch.list"
grep -qx 'identity sensor' "${SENSOR_BOOT}/run/sensor/plan"
# Every key the sensor covers is in the plan (modules among them, covered by its bit with no watch line)
grep -qx 'modules sensor' "${SENSOR_BOOT}/run/sensor/plan"
# It runs only while it is wanted: with the audit log on and the plan giving it keys, and not with it off
SENSOR_CHECK=(env AMETHYSTORA_SECURITY_CONF="${SENSOR_BOOT}/etc/amethystora/security.conf"
    AMETHYSTORA_SENSOR_RUN="${SENSOR_BOOT}/run/sensor" /usr/libexec/amethystora-sensor --check)
"${SENSOR_CHECK[@]}"
# A key switched off is neither in the plan nor in the watch list, as it loads no audit rule either
printf 'EXEC_WATCH=on\nPRIVILEGE=off\n' >"${SENSOR_BOOT}/etc/amethystora/security.conf"
AMETHYSTORA_AUDIT_ROOT="${SENSOR_BOOT}" AMETHYSTORA_SENSOR_RUN="${SENSOR_BOOT}/run/sensor" \
    AMETHYSTORA_SECURITY_CONF="${SENSOR_BOOT}/etc/amethystora/security.conf" \
    AMETHYSTORA_SENSOR_FORCE=sensor bash /usr/libexec/amethystora-audit-home-rules
grep -qx 'identity sensor' "${SENSOR_BOOT}/run/sensor/plan"
grep -q '^privilege ' "${SENSOR_BOOT}/run/sensor/plan" && false
grep -q ' privilege ' "${SENSOR_BOOT}/run/sensor/watch.list" && false
echo "AUDIT_LOG=off" >>"${SENSOR_BOOT}/etc/amethystora/security.conf"
"${SENSOR_CHECK[@]}" && false
rm -rf "${SENSOR_BOOT}"

# A full disk may never halt or suspend somebody's laptop. A gap in the audit log is the lesser loss,
# so the actions that stop the machine are the ones this asserts are absent.
grep -q "^max_log_file = 32$" /etc/audit/auditd.conf
grep -qiE "^(space_left_action|admin_space_left_action|disk_full_action|disk_error_action) = (halt|single|suspend)$" \
    /etc/audit/auditd.conf && false

# The per-home watches reach the kernel through the one load at boot, by audit-rules.service once auditd
# has started (audit 4.2; auditd itself before), because once -e 2 is in the rule set a second
# `augenrules --load` is refused. So they are written before both.
grep -qE "^Before=auditd.service( |$)" /usr/lib/systemd/system/amethystora-audit-rules.service
grep -qE "^Before=.* audit-rules.service( |$)" /usr/lib/systemd/system/amethystora-audit-rules.service
grep -qE "^After=auditd.service" /usr/lib/systemd/system/amethystora-audit-rules.service && false
# auditd starts before sysinit.target, so a unit ordered before it has to as well: with the default
# dependencies it sits after sysinit.target, and systemd breaks the cycle by not starting auditd
grep -q "^DefaultDependencies=no$" /usr/lib/systemd/system/amethystora-audit-rules.service
grep -q "^Before=.*sysinit.target" /usr/lib/systemd/system/amethystora-audit-rules.service
grep -q "^Before=.*sysinit.target" /usr/lib/systemd/system/auditd.service
# It runs as root in every home, so it never follows a link an account made out of its home, neither to
# give a folder back nor to watch one, tried against a stand-in /etc/passwd and /var/home; and it can write
# nowhere else (systemd-resolved runs this early with the same sandbox). The watcher says a watched place a
# link took out of a home, from the generator's own list.
bash /ctx/build_files/shared/test-audit-home-rules.sh
grep -q "^ProtectSystem=strict$" /usr/lib/systemd/system/amethystora-audit-rules.service
# /etc and the homes to write the rules; /var/lib/amethystora for the sensor's probe cache; the sensor's
# plan and watch list go in RuntimeDirectory (/run is read-only under ProtectSystem=strict otherwise)
grep -q "^ReadWritePaths=/etc -/var/home -/var/lib/amethystora$" /usr/lib/systemd/system/amethystora-audit-rules.service
grep -q "^RuntimeDirectory=amethystora/sensor$" /usr/lib/systemd/system/amethystora-audit-rules.service
grep -qF "amethystora-audit-home-rules --paths" /usr/libexec/amethystora-security-watch
# The rootkit check counts a file of the system's owned by a login account, so the image ships none
find /etc /var/usrlocal -xdev -uid +999 ! -uid 65534 2>/dev/null | grep . && false

# Weekly virus scan and monthly Lynis audit. What is done with a found file is ON_DETECTION's to say,
# through the one helper, never a clamdscan flag in a script. It ships as report: a false positive that
# takes away a file the user wanted is worse than most of what it would be removing.
test -x /usr/libexec/amethystora-clamav-scan
grep -vE "^[[:space:]]*#" /usr/libexec/amethystora-clamav-scan | grep -qE "(--remove|--move=)" && false
test -x /usr/libexec/amethystora-security-config
test -x /usr/libexec/amethystora-quarantine
[[ "$(/usr/libexec/amethystora-security-config ON_DETECTION)" == report ]]
[[ "$(/usr/libexec/amethystora-security-config REALTIME)" == off ]]
grep -q "^ON_DETECTION=report$" /etc/amethystora/security.conf
for script in amethystora-clamav-scan amethystora-clamav-onaccess; do
    grep -q "/usr/libexec/amethystora-quarantine act" "/usr/libexec/${script}"
done
# The helper scans each file again itself before it acts, whoever called it, and acts from inside the file's
# own folder, so an account swapping a folder or the file for a link cannot point root anywhere else
grep -q "clamdscan --fdpass" /usr/libexec/amethystora-quarantine
bash /ctx/build_files/shared/test-quarantine.sh
grep -q "^d /var/lib/amethystora/quarantine 0700 root root" /usr/lib/tmpfiles.d/amethystora-security.conf
grep -q "^settings " /usr/share/amethystora/just/security.just
grep -qF 'put_back="$(gettext "Put a file back")"' /usr/share/amethystora/just/security.just
grep -qF '"${put_back}" | restore)' /usr/share/amethystora/just/security.just

# Real-time watching and scanning: enabled, but each unit skips itself unless its half of REALTIME is on,
# the way the setting is meant to switch it rather than a unit somebody has to know about. The watcher's
# trigger also runs while network protection is on, to follow Suricata's alerts, and asks its script whether to.
grep -q "^ExecCondition=/usr/libexec/amethystora-security-config --is REALTIME_SCAN on$" \
    /usr/lib/systemd/system/amethystora-clamav-onaccess.service
grep -q "^ExecCondition=/usr/libexec/amethystora-security-config --is VIRUS_SCAN manual weekly$" \
    /usr/lib/systemd/system/amethystora-clamav-onaccess.service
grep -q "^ExecCondition=/usr/libexec/amethystora-security-realtime --check$" \
    /usr/lib/systemd/system/amethystora-security-realtime.service
/usr/libexec/amethystora-security-realtime --check && false
command -v clamonacc >/dev/null
grep -q "^OnAccessIncludePath /var/home$" /etc/clamd.d/scan.conf
# Without this, every file the weekly scan opens as root would be scanned a second time
grep -q "^OnAccessExcludeRootUID yes$" /etc/clamd.d/scan.conf

# The security watcher reads what the audit rules record: every key the rules write has to be one the
# watcher reads and the real-time trigger follows, or those events are recorded for nobody. The rules in
# home folders are written at boot by amethystora-audit-home-rules, and its keys count too.
test -x /usr/libexec/amethystora-security-watch
test -x /usr/libexec/amethystora-security-realtime
grep -q "^OnFailure=amethystora-scan-alert@%n.service$" /usr/lib/systemd/system/amethystora-security-watch.service
grep -q -- "-k user-programs" /usr/libexec/amethystora-audit-home-rules
for key in $( { grep -ohE -- '-k [a-z-]+$' /usr/share/amethystora/audit/*.rules | cut -d' ' -f2
    grep -ohE -- '(-k |key=)[a-z-]+' /usr/libexec/amethystora-audit-home-rules | sed -E 's/^(-k |key=)//'; } | sort -u); do
    grep -E "^AUDIT_KEYS=\(" /usr/libexec/amethystora-security-watch | grep -qw -- "${key}"
    grep -E "^KEYS=" /usr/libexec/amethystora-security-realtime | grep -qw -- "${key}"
    # ...and is a switch, as every audit key is
    /usr/libexec/amethystora-security-config "$(tr 'a-z-' 'A-Z_' <<<"${key}")" >/dev/null
done
# What reaches into another program's memory (INJECTION) has its fallback beside the other keys' rules, and its
# debuggers, whose doings are recorded and never said, are programs only an update writes
test -s /usr/share/amethystora/audit/injection.rules
/usr/libexec/amethystora-security-allow --features | grep -qx injection
[[ -z "$(grep -vE '^(#|$)' /usr/share/amethystora/debuggers | grep -vE '^/usr/[^ ]+$')" ]]
# ...and a watch on a path that is not there stops every rule after it from loading, which the generator
# leaves out on a machine rather than write. What only a machine has (/var, the disks it unlocks, a preloaded
# library) is watched once it is there; everything else has to be in the image.
for path in $(grep -ohE '^-w [^ ]+' /usr/share/amethystora/audit/*.rules | cut -d' ' -f2); do
    [[ "${path}" == /var/* || "${path}" == /etc/crypttab || "${path}" == /etc/ld.so.preload ]] && continue
    test -e "${path}"
done

# Every security feature is a switch, and off costs nothing: each unit the image enables for one reads its
# key as it starts (ExecCondition=), the packages' own through a drop-in, so that a line in security.conf
# edited by hand applies at the next boot. Then the settings helper, the generator, the report with every
# key off, the prevention switches and accepted checks, against stand-ins.
for unit in amethystora-security-watch amethystora-clamav-scan amethystora-clamav-onaccess amethystora-lynis-audit \
    amethystora-audit-rules amethystora-security-realtime amethystora-ips amethystora-ips-rules amethystora-signed-updates \
    amethystora-dns-names amethystora-yara amethystora-behaviour amethystora-integrity; do
    grep -q "^ExecCondition=" "/usr/lib/systemd/system/${unit}.service"
done
grep -q "^ExecCondition=/usr/libexec/amethystora-security-config --is APP_INTEGRITY on$" \
    /usr/lib/systemd/system/amethystora-integrity.service
for unit in auditd.service audit-rules.service clamav-freshclam.service clamd@.service; do
    grep -q "^ExecCondition=/usr/libexec/amethystora-security-config --is " "/usr/lib/systemd/system/${unit}.d/10-amethystora.conf"
done
test -x /usr/libexec/amethystora-hardening
test -x /usr/libexec/amethystora-security-allow
grep -q "^profile " /usr/share/amethystora/just/security.just
grep -q "^allow " /usr/share/amethystora/just/security.just
for switch in $(/usr/libexec/amethystora-hardening status --json | jq -r '.[].name'); do
    grep -q "^${switch} " /usr/share/amethystora/just/security.just
done
# What runs from /var/tmp is watched, and two tests need the folder, which a machine has from its first boot and
# the build does not (clean-stage.sh empties /var): there while they run, gone again after
mkdir -m 1777 /var/tmp
bash /ctx/build_files/shared/test-security-switches.sh
bash /ctx/build_files/shared/test-locked-poweroff.sh
# Its checks are the report's: the watcher runs the report rather than keeping checks of its own
grep -q "/usr/libexec/amethystora-security-status --json" /usr/libexec/amethystora-security-watch
# What an account plants is only ever read: text it wrote reaches the history, a notification and a terminal
# as plain text, a read in a home cannot keep a run waiting, and a run that hangs anyway is stopped. Every
# notification goes through the one filter (amethystora-security-text), in amethystora-notify-users.
test -x /usr/libexec/amethystora-security-text
grep -q "^TimeoutStartSec=" /usr/lib/systemd/system/amethystora-security-watch.service
grep -q "amethystora-security-text" /usr/libexec/amethystora-notify-users
bash /ctx/build_files/shared/test-security-watch.sh
rmdir /var/tmp
grep -q "^events " /usr/share/amethystora/just/security.just
# What only an account can see is read as that account and never as root, by the one reader, each of its
# modes one the watcher names, and for no longer than a minute
test -x /usr/libexec/amethystora-security-reader
grep -q 'timeout 60 setpriv --reuid=' /usr/libexec/amethystora-security-watch
for mode in $(grep -E '^declare -A READERS=' /usr/libexec/amethystora-security-watch | grep -oE '\]=[a-z-]+' | cut -c3-); do
    grep -qE "^    ${mode}\) " /usr/libexec/amethystora-security-reader
done
bash /ctx/build_files/shared/test-security-reader.sh
# Installed apps and the system's files against their updates (APP_INTEGRITY): the check, daily and after each
# change to the system's Flatpaks, against stand-in repositories, deploys and boot files, an account's own read as
# that account; and the image has what it imports
test -x /usr/libexec/amethystora-integrity
systemctl is-enabled amethystora-integrity.timer amethystora-integrity.path >/dev/null
grep -qx "PathChanged=/var/lib/flatpak/.changed" /usr/lib/systemd/system/amethystora-integrity.path
# Every line of them one this systemd reads: verify warns of an ignored one (CPUAccounting=, since 258), and
# exits 0 all the same, while the journal says it at every boot
[[ -z "$(systemd-analyze verify --man=no --recursive-errors=no \
    /usr/lib/systemd/system/amethystora-integrity.{service,timer,path} 2>&1)" ]]
grep -q "^app-integrity " /usr/share/amethystora/just/security.just
# ...which the person at the machine starts without a password, and nothing more of the unit
INTEGRITY_RULE=/usr/share/polkit-1/rules.d/50-amethystora-integrity.rules
grep -q 'action.lookup("unit") == "amethystora-integrity.service"' "${INTEGRITY_RULE}"
grep -q 'action.lookup("verb") == "start"' "${INTEGRITY_RULE}"
grep -q "subject.local && subject.active" "${INTEGRITY_RULE}"
bash /ctx/build_files/shared/test-app-integrity.sh
# The behaviour sensor: Falco from falcosecurity's own repository, with its rules as packaged, run only by
# amethystora-behaviour.service while BEHAVIOUR is on. None of its own units runs, its rules follower is masked,
# its web server is off, and its configuration with Amethystora's is one Falco loads.
rpm -q falco >/dev/null
# 0.45.1 or later, whose probe loads into Fedora 44's kernel: its release candidate until it is stable
[[ "$(printf '%s\n' 0.45.1 "$(rpm -q --queryformat '%{VERSION}' falco)" | sort -V | head -n1)" == 0.45.1 ]]
for unit in falco-modern-bpf.service falco-kmod.service falco-custom.service; do
    [[ "$(systemctl is-enabled "${unit}" 2>/dev/null)" == "enabled" ]] && false
done
[[ "$(systemctl is-enabled falcoctl-artifact-follow.service 2>/dev/null)" == "masked" ]]
grep -A1 "^webserver:" /etc/falco/config.d/60-amethystora.yaml | grep -q "enabled: false"
timeout 120 falco --dry-run >/dev/null
test -s /usr/share/licenses/falco/LICENSE
# ...with the probe's MIT licence and those of the libraries compiled into it, which its package leaves out (NOTICE)
test -s /usr/share/licenses/falco/driver-MIT.txt
[[ "$(find /usr/share/licenses/falco/bundled -type f -size +0 | wc -l)" == 16 ]]
# YARA rules are fetched on the machine, as freshclam's account and never as root, and split and tested so that
# ClamAV loads every one it is given
grep -q "^User=clamupdate$" /usr/lib/systemd/system/amethystora-yara.service
bash /ctx/build_files/shared/test-yara.sh
# What turns an agent's own checks off is known for every agent amethystora-agent offers
for agent in $(sed -n 's/^AGENTS=(\(.*\))$/\1/p' /usr/bin/amethystora-agent); do
    grep -q "^${agent}"$'\t' /usr/share/amethystora/agent-cli-flags
done
/usr/libexec/amethystora-security-status --json |
    jq -e '.settings.on_detection == "report" and .settings.realtime == "off" and .settings.network == "off"' >/dev/null

# Network protection (amethystora-ips): Suricata inline, enabled but skipped while NETWORK=off, which is
# how the image ships. Whatever goes wrong with it has to cost inspection and never the network: the
# table lets traffic through while nothing reads the queue, and the configuration 14-security.sh derives
# fails open, logs only alerts, and bypasses with the mark the table tests. That configuration has to be
# one this Suricata loads; its rules are downloaded on the machine, so one rule stands in for them.
command -v suricata-update >/dev/null
test -x /usr/libexec/amethystora-ips
# Its account has the fixed ID 04-packages.sh gives it, not one taken from the other accounts' range
[[ "$(id -u suricata):$(id -g suricata)" == "850:850" ]]
[[ "$(/usr/libexec/amethystora-security-config NETWORK)" == off ]]
grep -q "^NETWORK=off$" /etc/amethystora/security.conf
/usr/libexec/amethystora-ips enabled && false
[[ "$(systemctl is-enabled suricata.service 2>/dev/null)" == "enabled" ]] && false
for unit in amethystora-ips amethystora-ips-rules; do
    grep -q "^ExecCondition=/usr/libexec/amethystora-ips enabled$" "/usr/lib/systemd/system/${unit}.service"
done
grep -q "^OnFailure=amethystora-scan-alert@%n.service$" /usr/lib/systemd/system/amethystora-ips.service
IPS_TABLE=/usr/share/amethystora/ips/amethystora-ips.nft
grep -qE "^ +queue " "${IPS_TABLE}"
grep -E "^ +queue " "${IPS_TABLE}" | grep -v " flags bypass " && false
grep -q "ct mark 0x10000000 accept" "${IPS_TABLE}"
IPS_CONFIG=/usr/share/amethystora/ips/suricata.yaml
IPS_DUMP="$(suricata --dump-config -c "${IPS_CONFIG}")"
for setting in "nfq.fail-open = yes" "nfq.bypass-mark = 268435456" "exception-policy = ignore" \
    "stream.drop-invalid = no" "outputs.0.eve-log.types.0.alert.payload = no"; do
    grep -qxF "${setting}" <<<"${IPS_DUMP}"
done
grep -q "^outputs\.1\." <<<"${IPS_DUMP}" && false
IPS_TEST="$(mktemp -d)"
echo 'drop tcp any any -> any 9 (msg:"amethystora-ips build test"; sid:1; rev:1;)' >"${IPS_TEST}/test.rules"
# Everything it writes goes to the temporary directory. Suricata 8 caches what it compiles in
# /var/lib/suricata/cache/sgh by default, and making that here remade the /var/lib clean-stage.sh had
# removed, as 0750: see the check on /var/lib at the end of this file for what that did.
suricata -T -c "${IPS_CONFIG}" -S "${IPS_TEST}/test.rules" -l "${IPS_TEST}" \
    --set detect.sgh-mpm-caching-path="${IPS_TEST}"
rm -rf "${IPS_TEST}"
for dir in "lib/suricata 2770" "log/suricata 0750"; do
    grep -q "^d /var/${dir} suricata suricata " /usr/lib/tmpfiles.d/amethystora-security.conf
done
grep -q "^connections " /usr/share/amethystora/just/security.just
grep -q "amethystora-ips.service" /usr/libexec/amethystora-scan-alert
/usr/libexec/amethystora-security-status --json |
    jq -e 'any(.checks[]; .id == "network-protection" and .state == "off")' >/dev/null
grep -q "^ExcludePath \^/proc/$" /etc/clamd.d/scan.conf
grep -q "^ExcludePath \^/var/lib/flatpak/$" /etc/clamd.d/scan.conf
# The caches left out of the scan are under /var/home, where this system keeps home directories
grep -qF 'ExcludePath ^/var/home/[^/]+/\.cache/' /etc/clamd.d/scan.conf

# A finding nobody is told about is not a finding. Every way either scan can end badly fails its unit,
# and the failure reaches the person at the machine: OnFailure while they are logged in, and the login
# check for what happened while they were not.
test -x /usr/libexec/amethystora-notify-users
test -x /usr/libexec/amethystora-scan-alert
test -x /usr/libexec/amethystora-security-alert
test -x /usr/libexec/amethystora-lynis-audit
test -f /usr/lib/systemd/system/amethystora-scan-alert@.service
for unit in amethystora-clamav-scan amethystora-lynis-audit; do
    grep -q "^OnFailure=amethystora-scan-alert@%n.service$" "/usr/lib/systemd/system/${unit}.service"
done
grep -q "^ExecStart=/usr/libexec/amethystora-lynis-audit$" \
    /usr/lib/systemd/system/amethystora-lynis-audit.service
test -L /etc/systemd/user/graphical-session.target.wants/amethystora-security-alert.service
# clamd not starting is the likeliest reason a machine is not being scanned, and a Requires= on it would
# cancel this job rather than fail it, which runs no OnFailure at all. The scan has to reach the script.
# clamd runs on demand: the scan wants its socket, which starts clamd as clamdscan connects.
grep -q "^Wants=clamd@scan.socket$" /usr/lib/systemd/system/amethystora-clamav-scan.service
grep -qE "^Requires=" /usr/lib/systemd/system/amethystora-clamav-scan.service && false
# A connection left waiting while clamd is skipped (VIRUS_SCAN=off) would start it again and again until the
# socket failed (trigger-limit-hit); FlushPending refuses it instead
grep -qx "FlushPending=yes" /usr/lib/systemd/system/clamd@.socket

# Signature age is what says whether a clean scan means anything; `systemctl is-active` of the freshclam
# daemon does not, because it stays active whether or not a download ever succeeded. The image ships no
# signatures, so the helper reports exactly that here rather than a number.
test -x /usr/libexec/amethystora-clamav-signature-age
SIGNATURE_AGE_RC=0
/usr/libexec/amethystora-clamav-signature-age || SIGNATURE_AGE_RC=$?
[[ "${SIGNATURE_AGE_RC}" -eq 1 ]]
grep -q "amethystora-clamav-signature-age" /usr/libexec/amethystora-clamav-scan
grep -q "amethystora-clamav-signature-age" /usr/share/amethystora/just/security.just

# Backups. The helper never removes anything from the repository: an append-only repository is the
# whole of the defence against ransomware, and a client able to delete from one gives that away.
test -x /usr/libexec/amethystora-backup
test -s /usr/share/amethystora/backup-excludes
test -f /usr/lib/systemd/user/amethystora-backup.service
test -f /usr/lib/systemd/user/amethystora-backup.timer
grep -vE "^[[:space:]]*#" /usr/libexec/amethystora-backup | grep -qE "restic +(forget|prune)" && false
# Getting files back (amethystora-restore, which the Backups window, `ame backup snapshots` and `ame security
# ransomware restore` read) deletes nothing either: not from the backup, not a snapshot, and not with
# restore's --delete from the home folder. Each run that succeeded leaves the time the report warns by.
grep -vE "^[[:space:]]*#" /usr/libexec/amethystora-restore |
    grep -qE "restic(_read)?( +--[a-z-]+)* +(forget|prune|rewrite|key)\b|subvolume +delete|(^|[[:space:]])--delete\b" && false
# ...and the window runs neither restic nor btrfs itself, only the helper
grep -rhvE '^[[:space:]]*//' /ctx/apps/backups/src | grep -qE '"(restic|btrfs)"|"--delete"' && false
grep -q "amethystora-restore points$" /usr/share/amethystora/just/backup.just
grep -q "amethystora-restore points --local" /usr/share/amethystora/just/security.just
grep -q "backup-last-success" /usr/libexec/amethystora-backup

# Ransomware protection: hourly read-only snapshots of /var/home, off until `ame
# security ransomware` because they cost disk space. Both virus scans leave the snapshots out,
# or they would read every home folder once for each snapshot kept.
test -x /usr/libexec/amethystora-home-snapshot
test -f /usr/lib/systemd/system/amethystora-home-snapshot.service
test -f /usr/lib/systemd/system/amethystora-home-snapshot.timer
[[ "$(systemctl is-enabled amethystora-home-snapshot.timer 2>/dev/null)" == "enabled" ]] && false
command -v btrfs >/dev/null
grep -qF 'ExcludePath ^/var/home/\.snapshots/' /etc/clamd.d/scan.conf
grep -qF 'echo /var/home/.snapshots' /usr/libexec/amethystora-clamav-onaccess

# TPM disk unlock (ame security disk-unlock) only works if the initramfs can talk to the TPM at all.
# 19-initramfs.sh adds dracut's tpm2-tss module where dracut has it, and warns where it does not.
grep -q "tss2" <<<"${INITRAMFS_FILES}" ||
    echo "::warning::no TPM2 libraries in the initramfs, ame security disk-unlock will not work"
# It is the one TPM unlocker, and it replaces any key the TPM held before, so that one bound to no PCRs
# (as upstream's luks-tpm2-autounlock enrolled) cannot open the disk without the checks
test ! -e /usr/bin/luks-tpm2-autounlock
grep -q -- "--wipe-slot=tpm2 --tpm2-device=auto --tpm2-pcrs=7+14" /usr/share/amethystora/just/security.just

# DisplayLink: evdi built for the image kernel, and no module signing key left behind by the build
KERNEL_VERSION="$(rpm -q kernel-core --queryformat '%{VERSION}-%{RELEASE}.%{ARCH}')"
modinfo "/usr/lib/modules/${KERNEL_VERSION}/extra/evdi/evdi.ko.xz" >/dev/null
test -f /usr/lib/udev/rules.d/99-displaylink.rules
# Started by the device, not from boot: the rule has to match DisplayLink's vendor id and pull the unit in
grep -qE 'ATTR\{idVendor\}=="17e9".*ENV\{SYSTEMD_WANTS\}\+="displaylink\.service"' /usr/lib/udev/rules.d/99-displaylink.rules
if systemctl is-enabled -q displaylink.service; then
    echo "displaylink.service is enabled: it would run from boot on every machine, dock or not"
    exit 1
fi
# Skipped, not restarted every five seconds, where evdi cannot load (Secure Boot without the key enrolled)
grep -qx "ExecStartPre=" /usr/lib/systemd/system/displaylink.service.d/10-amethystora.conf
grep -qx "ExecCondition=/usr/sbin/modprobe evdi" /usr/lib/systemd/system/displaylink.service.d/10-amethystora.conf
grep -q "^d /var/log/displaylink " /usr/lib/tmpfiles.d/displaylink.conf
# DisplayLinkManager is proprietary. Its licence lets it be redistributed unmodified, and only with
# the licence itself beside it
test -s /usr/share/licenses/displaylink/LICENSE
find /etc/pki/akmods/private -type f 2>/dev/null | grep -q . && false
# ame security secure-boot enrolls both: the one evdi is signed with and the kernel's
test -f /etc/pki/akmods/certs/amethystora-modules.der
test -f /etc/pki/akmods/certs/akmods-amethystora.der
# ...and every other module the image adds is signed with one of the certificates it enrolls: a module that is not
# could never load with Secure Boot on. evdi is signed above, with a build's throwaway key where it has no other.
declare -A SB_SERIALS=()
for cert in /usr/share/amethystora/secure-boot/*.der; do
    SB_SERIALS["$(openssl x509 -inform DER -in "${cert}" -noout -serial | cut -d= -f2 | sed 's/^0*//' | tr a-f A-F)"]=1
done
while IFS= read -r module; do
    serial="$(modinfo -F sig_key "${module}" | tr -d ':' | sed 's/^0*//' | tr a-f A-F)"
    if [[ -z "${serial}" || -z "${SB_SERIALS[${serial}]+set}" ]]; then
        echo "${module} is not signed with a certificate in /usr/share/amethystora/secure-boot"
        exit 1
    fi
done < <(find "/usr/lib/modules/${KERNEL_VERSION}/extra" -name '*.ko*' ! -path '*/extra/evdi/*')

# Xbox controllers: xpadneo over Bluetooth, xone for Microsoft's wireless adapter, both from Universal Blue's
# akmods beside v4l2loopback. Microsoft's firmware for the adapter is not in the image, nor the blacklist of
# mt76x2u that would take MT7612U Wi-Fi sticks away: `ame apps xbox-adapter on` puts both on a machine that asks,
# the firmware from Microsoft, checked against the hashes pinned here, into a folder the kernel is pointed at
# for that machine alone (never in kargs.d) and that only root writes.
rpm -q kmod-xpadneo xpadneo-kmod-common kmod-xone xone-kmod-common >/dev/null
compgen -G "/usr/lib/modules/${KERNEL_VERSION}/extra/xone/xone_dongle.ko*" >/dev/null
compgen -G "/usr/lib/modules/${KERNEL_VERSION}/extra/xpadneo/hid-xpadneo.ko*" >/dev/null
[[ -z "$(find /usr/lib/firmware -name 'xow_dongle*' -o -name 'xone_dongle*')" ]]
grep -qx "blacklist mt76x2u" <<<"${MODPROBE_CONFIG:-$(modprobe --showconfig)}" && false
grep -qx "blacklist mt76x2u" /usr/share/amethystora/xone/xone.conf
compgen -G "/usr/src/amethystora/xone-kmod-common-*" >/dev/null && false
# Both drivers are GPL and built outside any repository, where source-manifest.sh only warns when it cannot fetch
# a source package: theirs travel with them
for kmod in kmod-xpadneo xpadneo-kmod-common kmod-xone; do
    test -s "/usr/src/amethystora/$(rpm -q --queryformat '%{SOURCERPM}' "${kmod}")"
done
ame --show apps::xbox-adapter >/dev/null
[[ "$(grep -cE '^[0-9a-f]{4} https://catalog\.s\.download\.windowsupdate\.com/[^ ]+\.cab [0-9a-f]{64} [A-Za-z0-9_.]+ [0-9a-f]{64}$' \
    /usr/share/amethystora/xone/firmware)" == 4 ]]
grep -rq "firmware_class" /usr/lib/bootc/kargs.d && false
grep -qx "d /var/lib/amethystora/firmware 0755 root root -" /usr/lib/tmpfiles.d/amethystora-firmware.conf
command -v cabextract >/dev/null
# Left out, with their reasons in the manual: Broadcom's wl (a licence that makes whoever passes it on answer for
# it, and security holes nobody may fix) and openrazer (a daemon from a repository the image does not use). Their
# packages, and wl's blacklist of the kernel's own Broadcom drivers, are nowhere.
for package in kmod-wl broadcom-wl kmod-openrazer openrazer; do
    rpm -q "${package}" >/dev/null 2>&1 && { echo "${package} is in the image"; exit 1; }
done
grep -rlE '^[[:space:]]*blacklist[[:space:]]+(brcmfmac|brcmsmac|bcma|b43|ssb)$' /usr/lib/modprobe.d && false

# The NVIDIA images: NVIDIA's open module on -nvidia-open, its closed one of the 580 branch on -nvidia-lts
NVIDIA_KO="$(find "/usr/lib/modules/${KERNEL_VERSION}" -name 'nvidia.ko*' | head -n1)"
case "${IMAGE_NAME}" in
    *-nvidia-open) [[ "$(modinfo -F license "${NVIDIA_KO}")" == "Dual MIT/GPL" ]] ;;
    *-nvidia-lts) [[ "$(modinfo -F license "${NVIDIA_KO}")" == "NVIDIA" ]] ;;
    *) [[ -z "${NVIDIA_KO}" ]] ;;
esac
# ...whose services are skipped where the driver is not loaded, rather than restarted until the machine is degraded
if [[ "${IMAGE_NAME}" =~ nvidia ]]; then
    for unit in nvidia-persistenced.service nvidia-cdi-refresh.service; do
        grep -qx "ConditionPathExists=/sys/module/nvidia" "/usr/lib/systemd/system/${unit}.d/10-amethystora-driver.conf"
    done
fi

# What keeps a device from working, found by the machine itself (amethystora-driver-status, a few minutes after
# each start), with what fixes it; tried against a stand-in machine with one of each kind
test -x /usr/libexec/amethystora-driver-status
ame --show system::drivers >/dev/null
/usr/libexec/amethystora-driver-status --json | jq -e '(.findings | type) == "array" and (.checked | type) == "number"' >/dev/null
DRIVERS_TEST="$(mktemp -d)"
pci_device() {
    mkdir -p "${DRIVERS_TEST}/sys/bus/pci/devices/$1"
    printf '%s\n' "$2" >"${DRIVERS_TEST}/sys/bus/pci/devices/$1/class"
    printf '%s\n' "$3" >"${DRIVERS_TEST}/sys/bus/pci/devices/$1/vendor"
    printf '%s\n' "$4" >"${DRIVERS_TEST}/sys/bus/pci/devices/$1/device"
}
pci_device 0000:00:02.0 0x030000 0x8086 0x46a6
mkdir -p "${DRIVERS_TEST}/sys/bus/pci/drivers/i915"
ln -s ../../../../bus/pci/drivers/i915 "${DRIVERS_TEST}/sys/bus/pci/devices/0000:00:02.0/driver"
pci_device 0000:00:1f.0 0x060100 0x8086 0x7a84
pci_device 0000:01:00.0 0x030000 0x10de 0x1b80
pci_device 0000:02:00.0 0x028000 0x14e4 0x43a0
pci_device 0000:03:00.0 0x028000 0x8086 0x2725
pci_device 0000:04:00.0 0x010802 0x1987 0x5018
mkdir -p "${DRIVERS_TEST}/sys/bus/usb/devices/1-2" "${DRIVERS_TEST}/sys/bus/usb/devices/1-3" "${DRIVERS_TEST}/sys/bus/usb/devices/1-3:1.0"
printf '045e\n' >"${DRIVERS_TEST}/sys/bus/usb/devices/1-2/idVendor"
printf '02fe\n' >"${DRIVERS_TEST}/sys/bus/usb/devices/1-2/idProduct"
printf '0bda\n' >"${DRIVERS_TEST}/sys/bus/usb/devices/1-3/idVendor"
printf 'b812\n' >"${DRIVERS_TEST}/sys/bus/usb/devices/1-3/idProduct"
printf 'e0\n' >"${DRIVERS_TEST}/sys/bus/usb/devices/1-3:1.0/bInterfaceClass"
mkdir -p "${DRIVERS_TEST}/proc" "${DRIVERS_TEST}/usr/share/amethystora"
echo "BOOT_IMAGE=/vmlinuz root=UUID=0 rw" >"${DRIVERS_TEST}/proc/cmdline"
echo '{"image-name": "amethystora", "image-flavor": "main", "image-tag": "stable"}' >"${DRIVERS_TEST}/usr/share/amethystora/image-info.json"
cat >"${DRIVERS_TEST}/kernel.log" <<'EOF'
iwlwifi 0000:03:00.0: Direct firmware load for iwlwifi-ty-a0-gf-a0-89.ucode failed with error -2
iwlwifi 0000:03:00.0: Direct firmware load for iwlwifi-ty-a0-gf-a0-88.ucode failed with error -2
amdgpu 0000:00:02.0: Direct firmware load for amdgpu/optional.bin failed with error -2
Loading of module with unavailable key is rejected
Failed to insert module 'v4l2loopback': Key was rejected by service
EOF
echo "not-enrolled /usr/share/amethystora/secure-boot/akmods-amethystora.der" >"${DRIVERS_TEST}/mok"
DRIVERS_FOUND="$(AMETHYSTORA_DRIVERS_ROOT="${DRIVERS_TEST}" AMETHYSTORA_DRIVERS_LOG="${DRIVERS_TEST}/kernel.log" \
    AMETHYSTORA_DRIVERS_MOK="${DRIVERS_TEST}/mok" /usr/libexec/amethystora-driver-status --json)"
jq -e '[.findings[] | "\(.kind) \(.id) \(.fix // "")"] | sort == ([
    "broadcom-wifi broadcom-wifi:0000:02:00.0 ",
    "firmware firmware:iwlwifi-ty-a0-gf-a0-89.ucode ",
    "no-driver no-driver:0000:04:00.0:0x5018 ",
    "no-driver no-driver:usb:0bda:b812 ",
    "nvidia nvidia:0000:01:00.0 ame system rebase amethystora-nvidia-lts stable",
    "refused refused:v4l2loopback ",
    "xbox-adapter xbox-adapter:02fe ame apps xbox-adapter on"] | sort)' <<<"${DRIVERS_FOUND}" >/dev/null ||
    { echo "amethystora-driver-status found: ${DRIVERS_FOUND}"; exit 1; }
mapfile -t DRIVERS_MANUAL < <(jq -r '.findings[].manual' <<<"${DRIVERS_FOUND}" | sort -u)
"${NODE[@]}" /ctx/build_files/shared/check-manual.js /usr/share/amethystora/manual --links "${DRIVERS_MANUAL[@]}"
rm -rf "${DRIVERS_TEST}"
bash /ctx/build_files/shared/test-driver-status.sh
# A DisplayLink dock's arrival starts the driver check, after the service that tries to load the dock's driver
grep -qE 'ATTR\{idVendor\}=="17e9".*ENV\{SYSTEMD_WANTS\}\+="amethystora-driver-check\.service"' /usr/lib/udev/rules.d/99-amethystora-displaylink.rules
grep -qx 'After=displaylink.service' /usr/lib/systemd/system/amethystora-driver-check.service
"${NODE[@]}" /ctx/build_files/shared/check-manual.js /usr/share/amethystora/manual --links hardware#docks-and-displays \
    hardware#secure-boot troubleshooting#a-displaylink-dock-shows-nothing

# Make sure this garbage never makes it to an image
test -f /usr/lib/systemd/system/flatpak-add-fedora-repos.service && false

IMPORTANT_PACKAGES=(
    audit
    clamav
    clamav-freshclam
    clamd
    displaylink
    distrobox
    dotnet-sdk-10.0
    fail2ban-firewalld
    fail2ban-server
    firefox
    fish
    flatpak
    gdm
    gnome-shell
    lynis
    mutter
    pam-u2f
    pamu2fcfg
    pipewire
    ptyxis
    restic
    suricata
    systemd
    tailscale
    tpm2-tools
    usbguard
    uupd
    wireplumber
    zsh
)

for package in "${IMPORTANT_PACKAGES[@]}"; do
    rpm -q "${package}" >/dev/null || { echo "Missing package: ${package}... Exiting"; exit 1 ; }
done

# The agentic features. The agents themselves install into each user's home (amethystora-agent
# install), or Homebrew's; a copy in /usr/bin would be one the user cannot update. Control offers
# each of them. A skill's name has to match its directory, or some of the agents skip it.
test -x /usr/bin/amethystora-agent
AGENT_IDS=(claude opencode codex agy copilot cursor-agent)
[[ "$(amethystora-agent list | cut -f1 | paste -sd ' ')" == "${AGENT_IDS[*]}" ]]
for agent in "${AGENT_IDS[@]}"; do
    [[ ! -e "/usr/bin/${agent}" ]]
done
[[ "$(jq -r '.items[] | select(.id == "agent-choice") | [.choices[].value] | join(" ")' <<<"${CONTROL_LIST}")" == "${AGENT_IDS[*]}" ]]
test -x /usr/share/amethystora/user-setup.hooks.d/13-agentic.sh
# Off as the image ships them: no account has the marker, and the update that turned them off says so once to
# an account that had them on, from a hook that runs before 13-agentic.sh unlinks the skills it looks for
test -x /usr/libexec/amethystora-agentic-off-notice
[[ "$(find /usr/share/amethystora/user-setup.hooks.d -maxdepth 1 -name '13-agentic*' -printf '%f\n' | LC_ALL=C sort | head -n1)" == 13-agentic-off.sh ]]
test ! -e /etc/skel/.config/amethystora/agentic
for skill in amethystora amethystora-diagnose; do
    grep -qx "name: ${skill}" "/usr/share/amethystora/agents/skills/${skill}/SKILL.md"
done
# The main skill is a short core, and its references are read only when needed: every reference the core
# names is there, and every one there is named, or no agent would find it
SKILL_DIR=/usr/share/amethystora/agents/skills/amethystora
[[ "$(grep -o "${SKILL_DIR}/references/[a-z-]*\.md" "${SKILL_DIR}/SKILL.md" | sort -u)" == \
    "$(find "${SKILL_DIR}/references" -name '*.md' | sort)" ]]
(($(stat -c %s "${SKILL_DIR}/SKILL.md") < 7000))
ame --show agent::toggle >/dev/null
grep -q "^install " /usr/share/amethystora/just/agent.just
# The administrator's lock and root mode: shipped as the image's defaults, read by the tools as the one reader,
# and changed only by their two commands, which the security watcher's drift check accepts
for recipe in lock root history tools; do
    ame --show "agent::${recipe}" >/dev/null
done
# Level 2 needs level 1, and lists the four tools that change things beside the five that read; the
# amethystora-tools skill names exactly the nine
(
    export HOME="$(mktemp -d)" XDG_CONFIG_HOME="" SHELL=/usr/bin/bash
    ame agent tools on && false
    ame agent toggle on
    ame agent tools on
    test -e "${HOME}/.config/amethystora/agent-tools"
    [[ "$(/usr/libexec/amethystora-agent-tools mcp <<<'{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | jq -sr '[.[0].result.tools[].name] | join(" ")')" == \
        "search read state logs history change software run undo" ]]
    [[ "$(sed -n 's/^description: .*tools (\(.*\)) to understand.*/\1/p' /usr/share/amethystora/agents/skills/amethystora-tools/SKILL.md |
        tr -c 'a-z\n' ' ' | tr -s ' ' '\n' | grep -vxE 'with|changes|allowed|' | sort | paste -sd ' ')" == \
        "change history logs read run search software state undo" ]]
    ame agent toggle off
    test ! -e "${HOME}/.config/amethystora/agent-tools"
    rm -rf "${HOME}"
)
# The agent's root commands: one polkit action for each mode but terminal, each an administrator's password at
# the machine and never anyone else's, and no rule letting anyone through without it. The helper runs only as
# pkexec starts it, for the tool server, over a socket
python3 -I - <<'PY'
import xml.etree.ElementTree as ET
want = {"org.amethystora.agent.root-session": ("auth_admin", "session"),
        "org.amethystora.agent.root-minutes": ("auth_admin_keep", "minutes"),
        "org.amethystora.agent.root-each": ("auth_admin", "each")}
got = {}
for action in ET.parse("/usr/share/polkit-1/actions/org.amethystora.agent.policy").getroot().findall("action"):
    defaults = action.find("defaults")
    notes = {note.get("key"): note.text for note in action.findall("annotate")}
    assert defaults.find("allow_any").text == "no" and defaults.find("allow_inactive").text == "no", action.get("id")
    assert notes["org.freedesktop.policykit.exec.path"] == "/usr/libexec/amethystora-agent-root", action.get("id")
    got[action.get("id")] = (defaults.find("allow_active").text, notes["org.freedesktop.policykit.exec.argv1"])
assert got == want, got
PY
grep -rqs "org.amethystora.agent" /usr/share/polkit-1/rules.d /etc/polkit-1/rules.d && false
test -x /usr/libexec/amethystora-agent-root
[[ "$(stat -c '%U %a' /usr/libexec/amethystora-agent-root)" == "root 755" ]]
/usr/libexec/amethystora-agent-root session </dev/null 2>/dev/null && false
[[ "$(PKEXEC_UID=0 /usr/libexec/amethystora-agent-root session </dev/null 2>&1)" == *socket* ]]
# The hand-off to a terminal shows what the agent asks to run and reports how it ended
grep -q -- "--agent REASON" /usr/libexec/amethystora-in-terminal
grep -q -- "--report DIR" /usr/libexec/amethystora-in-terminal
grep -qx "ALLOW=act" /etc/amethystora/agent.conf
grep -qx "ROOT=session" /etc/amethystora/agent.conf
/usr/libexec/amethystora-agent-tools policy --json | jq -e '.allow == "act" and .root == "session" and .allow_for == {}' >/dev/null
grep -q "amethystora/agent.conf" /usr/libexec/amethystora-security-watch
[[ "$(jq -r '.items[] | select(.id == "agent-root") | [.choices[].value] | join(" ")' <<<"${CONTROL_LIST}")" == "session minutes each terminal" ]]
[[ "$(jq -r '.items[] | select(.id == "agent-lock") | [.choices[].value] | join(" ")' <<<"${CONTROL_LIST}")" == "act know off" ]]
[[ -z "$(jq -c '.items[] | select(.id == "agent-root" or .id == "agent-lock") | .choices[] | select(.terminal != true)' <<<"${CONTROL_LIST}")" ]]

# The agent's tools (plans/done/PLAN-agentic-os.md): an MCP server over stdio that lists nothing for an account
# with the features off and the five read-only tools once they are on, over an index of the manual, the
# skills, the knowledge and the image's own reference, written when the image was built
AGENT_TOOLS=/usr/libexec/amethystora-agent-tools
KNOWLEDGE=/usr/share/amethystora/agents/knowledge
test -x "${AGENT_TOOLS}"
AGENT_HOME="$(mktemp -d)"
MCP_ASK=$'{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}\n{"jsonrpc":"2.0","id":2,"method":"tools/list"}'
HOME="${AGENT_HOME}" XDG_CONFIG_HOME="" "${AGENT_TOOLS}" mcp <<<"${MCP_ASK}" |
    jq -se '.[0].result.serverInfo.name == "amethystora" and .[1].result.tools == []' >/dev/null
mkdir -p "${AGENT_HOME}/.config/amethystora"
touch "${AGENT_HOME}/.config/amethystora/agentic"
HOME="${AGENT_HOME}" XDG_CONFIG_HOME="" "${AGENT_TOOLS}" mcp <<<"${MCP_ASK}" |
    jq -se '[.[1].result.tools[].name] == ["search", "read", "state", "logs", "history"]' >/dev/null
jq -e '.dims == 256 and .kinds.manual > 100 and .kinds.skill > 10 and .kinds.command > 80 and .kinds.helper > 30
    and .kinds.unit > 20 and .kinds.setting > 30 and .kinds.control > 20' "${KNOWLEDGE}/meta.json" >/dev/null
[[ "$(stat -c %s "${KNOWLEDGE}/vectors.f16")" == "$(($(jq .passages "${KNOWLEDGE}/meta.json") * 256 * 2))" ]]
# The model is the one the apps stage pinned, unchanged, and its licences and those of what it was made from ship
sed -n 's/^    \[\([a-z.]*\)\]=\([0-9a-f]\{64\}\)$/\2  \1/p' /ctx/build_files/shared/build-apps.sh >/tmp/model.sha256
[[ "$(wc -l </tmp/model.sha256)" == 3 ]]
(cd "${KNOWLEDGE}/model" && sha256sum -c --quiet /tmp/model.sha256)
rm -f /tmp/model.sha256
for licence in LICENSE LICENSE.bge-base-en-v1.5 LICENSE.bert-vocabulary SOURCE; do
    test -s "/usr/share/licenses/potion-base-8M/${licence}"
done
grep -q "potion-base-8M" /usr/share/licenses/amethystora/NOTICE
# Every passage's file is there, and nothing in the index names the duress feature
jq -r 'select(.path != null) | .path' "${KNOWLEDGE}/index.jsonl" | sort -u | while read -r path; do test -e "${path}"; done
grep -qi duress "${KNOWLEDGE}/index.jsonl" && false
HOME="${AGENT_HOME}" XDG_CONFIG_HOME="" "${AGENT_TOOLS}" search "roll back an update" | grep -q "manual/updates#rolling-back"
HOME="${AGENT_HOME}" XDG_CONFIG_HOME="" "${AGENT_TOOLS}" read "ame/security/dns" | grep -q "ame security dns"
# The golden questions find their passage in the first three, on this image's own index, words and meaning fused
HOME="${AGENT_HOME}" XDG_CONFIG_HOME="" "${AGENT_TOOLS}" eval /ctx/apps/agent-tools/tests/golden.tsv --dir "${KNOWLEDGE}" --min 0.9
rm -rf "${AGENT_HOME}"
# The knowledge written for agents names only what is there: every path under /usr (placeholders, /usr/etc,
# which only a deployed machine has, and the sealed images' marker aside) and every unit; the check of `ame` commands above reads it too. Every
# playbook a doctor finding points to is there.
KNOWLEDGE_FILES=("${KNOWLEDGE}"/*.md "${KNOWLEDGE}"/playbooks/*.md)
(($(ls "${KNOWLEDGE}"/playbooks/*.md | wc -l) >= 15))
grep -ohE '/usr/[A-Za-z0-9._@/+-]+' "${KNOWLEDGE_FILES[@]}" | sed 's/[.,:;)]*$//' | grep -vE '[A-Z]{2,}|^/usr/etc(/|$)|^/usr/share/amethystora/sealed$' | sort -u |
    while read -r path; do
        [[ -e "${path}" || -L "${path}" ]] || { echo "The agent's knowledge names ${path}, which is not in the image"; exit 1; }
    done
grep -ohE '\b[a-z0-9][a-z0-9_.@-]*\.(service|timer|socket|path)\b' "${KNOWLEDGE_FILES[@]}" | sort -u | while read -r unit; do
    template="$(sed -E 's/@[^.]+\./@./' <<<"${unit}")"
    compgen -G "/usr/lib/systemd/system/${template}" >/dev/null || compgen -G "/usr/lib/systemd/user/${template}" >/dev/null ||
        { echo "The agent's knowledge names ${unit}, which is not in the image"; exit 1; }
done
for playbook in $(grep -oE 'playbook/[a-z][a-z-]+' /usr/libexec/amethystora-doctor | sort -u); do
    test -s "${KNOWLEDGE}/playbooks/${playbook#playbook/}.md"
done
# The health check reads only, as an account, and says what it found as JSON a tool reads
test -x /usr/libexec/amethystora-doctor
ame --show system::doctor >/dev/null
/usr/libexec/amethystora-doctor --json | jq -e '(.findings | type) == "array" and (.seconds | type) == "number"' >/dev/null
# The launcher gives an agent the tools only while the features are on, and with options none of which turns
# the agent's own checks off (agent-cli-flags, as the watcher matches them). Cursor CLI and Antigravity CLI,
# which take no such option, have the entry in their settings only while the features are on.
for skill in amethystora-tools amethystora-widgets; do
    grep -qx "name: ${skill}" "/usr/share/amethystora/agents/skills/${skill}/SKILL.md"
done
(
    export HOME="$(mktemp -d)" XDG_CONFIG_HOME="" XDG_RUNTIME_DIR="$(mktemp -d)" SHELL=/usr/bin/bash
    mkdir -p "${HOME}/.cursor"
    for agent in "${AGENT_IDS[@]}"; do
        amethystora-agent argv "${agent}" hi | grep -q "amethystora-agent-tools\|amethystora_" && false
    done
    ame agent toggle on
    jq -e '.mcpServers.amethystora == {"command": "/usr/libexec/amethystora-agent-tools", "args": ["mcp"]}' "${HOME}/.cursor/mcp.json" >/dev/null
    for agent in "${AGENT_IDS[@]}"; do
        argv="$(amethystora-agent argv "${agent}" hi)"
        [[ "${agent}" =~ ^(agy|cursor-agent)$ ]] || grep -q "amethystora" <<<"${argv}"
        words=" $(grep -v '^env:' <<<"${argv}" | sed -E 's/^(--[a-z-]+)=/\1 /' | tr '\n' ' ') "
        environment="$(sed -n 's/^env://p' <<<"${argv}")"
        while IFS=$'\t' read -r name flag; do
            [[ "${name}" == "${agent}" ]] || continue
            if [[ "${flag}" == env:*=* ]]; then
                variable="${flag#env:}"
                glob="${variable#*=}"
                variable="${variable%%=*}"
                while IFS= read -r entry; do
                    # shellcheck disable=SC2053 # the glob is the image's own, from agent-cli-flags
                    [[ "${entry%%=*}" == "${variable}" && "${entry#*=}" == ${glob} ]] && false
                done <<<"${environment}"
            else
                [[ "${words}" == *" ${flag} "* ]] && false
            fi
        done < <(grep -v '^#' /usr/share/amethystora/agent-cli-flags)
    done
    ame agent toggle off
    jq -e '.mcpServers.amethystora == null' "${HOME}/.cursor/mcp.json" >/dev/null
    rm -rf "${HOME}" "${XDG_RUNTIME_DIR}"
)

# The Vault (plans/PLAN-vault.md): its service's sandbox, its polkit policy with no yes and no kept answer, its sockets enabled,
# every recipe, every sentence of the service a line of the recipes' catalog, no rsa crate, the data and licences it ships
# (tests/vm/suites/74-vault.sh is what runs it)
bash /ctx/build_files/shared/test-vault.sh /ctx

# The plugin system (plans/PLAN-plugins.md): the root helper and the host installed, the polkit policy with an administrator for
# every verb that gives more and no kept answer, `grant` with no --yes, the host's unit with no namespace setting and enabled for
# nobody, the sandbox recipe, the agent's never list (tests/vm/suites/72-plugins.sh is what runs it)
bash /ctx/build_files/shared/test-plugins.sh /ctx

# these packages are supposed to be removed
# and are considered footguns
UNWANTED_PACKAGES=(
    akmod-evdi
    fedora-logos
    gnome-software
    gnome-software-rpm-ostree
    podman-docker
)

for package in "${UNWANTED_PACKAGES[@]}"; do
    if rpm -q "${package}" >/dev/null 2>&1; then
        echo "Unwanted package found: ${package}... Exiting"; exit 1
    fi
done

if [[ "${IMAGE_NAME}" =~ nvidia ]]; then
  NV_PACKAGES=(
      libnvidia-container-tools
      kmod-nvidia
      nvidia-driver-cuda
)
  for package in "${NV_PACKAGES[@]}"; do
      rpm -q "${package}" >/dev/null || { echo "Missing NVIDIA package: ${package}... Exiting"; exit 1 ; }
  done
fi

# --- The build and signing chain -----------------------------------------------------------------------
# The base image is at most 10 days older than this build. image-versions.yml pins it by digest, so every
# package it brings stays as it was until the pin moves; bump-base-images.yml proposes the move daily. A
# warning, not a failure: a failed build would also hold back the kernel and every package the build
# installs itself, and the report says how old the base is on every machine.
BASE_CREATED="$(jq -r '."base-created"' /usr/share/amethystora/image-info.json)"
[[ "${BASE_CREATED}" =~ ^[0-9]+$ ]]
if (($(date +%s) - BASE_CREATED > 10 * 86400)); then
    echo "::warning::The base image was built $((($(date +%s) - BASE_CREATED) / 86400)) days ago: merge the pull request that updates image-versions.yml"
fi
# The repositories whose keys the build pinned (pin_repo_key, copr-helpers.sh) name the checked copy,
# never a key on their server
for file in /etc/yum.repos.d/{tailscale,docker-ce,falcosecurity,_copr_ublue-os-akmods}.repo \
    /etc/yum.repos.d/_copr:copr.fedorainfracloud.org:{che:nerd-fonts,ublue-os:packages,bieszczaders:kernel-cachyos-addons}.repo; do
    [[ -f "${file}" ]] || continue
    if grep -E '^gpgkey[[:space:]]*=' "${file}" | grep -v '=file:///etc/pki/rpm-gpg/'; then
        echo "${file} names a signing key on a server"
        exit 1
    fi
done

# Swapped-out memory: lzo-rle, with zstd for what stays unused (zram-generator.conf.d, and the hourly timer below)
grep -qx 'compression-algorithm = lzo-rle zstd' /usr/lib/systemd/zram-generator.conf.d/10-amethystora.conf

IMPORTANT_UNITS=(
    clamav-freshclam.service
    clamd@scan.socket
    amethystora-clamd-idle.timer
    fail2ban.service
    gdm.service
    rpm-ostree-countme.timer
    tailscaled.service
    uupd.timer
    auditd.service
    amethystora-audit-rules.service
    amethystora-clamav-scan.timer
    amethystora-lynis-audit.timer
    amethystora-security-watch.timer
    amethystora-security-realtime.service
    amethystora-clamav-onaccess.service
    amethystora-ips.service
    amethystora-ips-rules.timer
    amethystora-system-setup.service
    amethystora-signed-updates.service
    amethystora-gpu-check.timer
    amethystora-driver-check.timer
    amethystora-raid-check.timer
    amethystora-raid-scrub.timer
    amethystora-disk-health.timer
    amethystora-raid-boot.service
    amethystora-boot-guard.service
    amethystora-boot-guard-good.service
    amethystora-raid-mirror.service
    amethystora-zram-recompress.timer
  )

for unit in "${IMPORTANT_UNITS[@]}"; do
    if ! systemctl is-enabled "$unit" 2>/dev/null | grep -q "^enabled$"; then
        echo "${unit} is not enabled"
        exit 1
    fi
done

# Last, after everything above has run: the rechunker copies /var/lib onto /usr/lib with
# `rsync ./var/lib/ ./usr/lib/`, which gives /usr/lib the mode of /var/lib. clean-stage.sh removes
# /var/lib, so one here was made by something in this build since, with whatever mode that chose. A
# /usr/lib that is not 0755 locks every service that does not run as root out of its own program
# (polkit, systemd-resolved, the login screen's user manager), and the machine boots to a black screen.
if [[ -e /var/lib ]]; then
    [[ "$(stat -c '%a %U %G' /var/lib)" == "755 root root" ]] ||
        { echo "/var/lib is $(stat -c '%a %U %G' /var/lib); the rechunker would give /usr/lib that mode"; exit 1; }
fi
[[ "$(stat -c '%a %U %G' /usr/lib)" == "755 root root" ]]

echo "::endgroup::"
