#!/usr/bin/bash
# Amethystora UI (plans/PLAN-amethystora-ui.md): the one extension that replaced PaperWM, Dash to Dock, Blur my Shell and
# the rest, and the icon packs and window lights that go with it, as a person's session has them. No window tree of its
# own to walk (client windows give no positions on Wayland), so it asks what the image can say for sure:
#   - the extension is active, no module said it did not start, none threw, and none of what it replaced is installed;
#   - the tiling is one switch: classic gives GNOME's Super+N back, tiling takes it again, and the state file says so;
#   - the icon packs, eight: Glass on a dark theme, Glass for light on a light one, any by choice, candy-icons refused, every launcher's
#     icon and other makers' apps found in each pack by GTK itself, and Firefox keeping its own;
#   - the window lights are in the account's GTK 4 and GTK 3 stylesheets;
#   - the dock is on the bottom of the screen, in the shell's accessibility tree;
#   - the glass has its picture of the screen and the theme's colours.
# Two Files windows and the quick settings are left open for the screenshot run.mjs takes after the suite: the strip,
# the glass of the top bar and a menu, and the lights.
# vmlab-paths: @ui/* @sf/*themed/* @sf/usr/share/icons/* @sf/usr/lib/amethystora/theme/*
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
OWN=/run/vmlab-27
HOME_DIR="/var/home/${TEST_USER}"
TILING=org.gnome.shell.extensions.amethystora-ui.tiling

if ! wait_for 300 has_session; then
    skip "no session for ${TEST_USER} (10-account.sh)"
    finish
fi

gsettings_get() { in_session "gsettings get $1 $2" 2>/dev/null; }
# Until a value is what it should be (the shell reacts to a key a moment after it is written)
settles() {
    local schema="$1" key="$2" want="$3" i
    for i in $(seq 1 15); do
        [[ "$(gsettings_get "${schema}" "${key}")" == "${want}" ]] && return 0
        sleep 1
    done
    return 1
}

# --- The extension ---------------------------------------------------------------------------------------------------
info="$(in_session "gnome-extensions info ${UUID}" 2>&1)"
if grep -qE 'State: *(ACTIVE|ENABLED)' <<<"${info}"; then
    ok "Amethystora UI is active"
else
    not_ok "Amethystora UI is not active" "${info}"
fi
extensions="$(in_session "gnome-extensions list" 2>&1)"
left="$(grep -E 'paperwm|dash-to-dock|blur-my-shell|just-perfection|space-bar|tophat|appindicator|search-light|fuzzy-app|gradia-integration|bazaar-integration|caffeine|gsconnect' <<<"${extensions}")"
if [[ -z "${left}" ]]; then
    ok "none of the extensions it replaced is installed"
else
    not_ok "an extension it replaced is still installed" "${left}"
fi
log="$(journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --no-pager -o cat 2>/dev/null)"
modules="$(grep -E 'Amethystora UI: the [a-z]+ module did not (start|stop cleanly)' <<<"${log}")"
if [[ -z "${modules}" ]]; then
    ok "every module started, and the log has no module that stopped badly"
else
    not_ok "a module did not start" "${modules}"
fi
errors="$(grep -E "JS ERROR" <<<"${log}" | grep -E "amethystora-ui@" | head -n 10)"
if [[ -z "${errors}" ]]; then
    ok "nothing of the extension threw in the shell's log"
else
    not_ok "the extension threw" "${errors}"
fi

# --- The tiling, one switch ---------------------------------------------------------------------------------------------
check "the tiling is on as the image ships" test "$(gsettings_get "${TILING}" enabled)" == true
# While it runs, its Super+N (a new window) takes the combination from GNOME's focus-active-notification, in dconf,
# and writes down what it took
if settles org.gnome.shell.keybindings focus-active-notification "@as []"; then
    ok "the tiling has taken Super+N from GNOME's notification key"
else
    not_ok "the tiling has not taken Super+N" "focus-active-notification: $(gsettings_get org.gnome.shell.keybindings focus-active-notification)"
fi
state="${HOME_DIR}/.local/state/amethystora/tiling.json"
if jq -e '.taken | any(.key == "focus-active-notification")' "${state}" >/dev/null 2>&1; then
    ok "and its state file says what it took"
else
    not_ok "the state file does not record the taken key" "$(cat "${state}" 2>&1 | head -c 600)"
fi
in_session "ame desktop layout classic" >/dev/null 2>&1
if settles org.gnome.shell.keybindings focus-active-notification "['<Super>n']" && [[ "$(gsettings_get "${TILING}" enabled)" == false ]]; then
    ok "ame desktop layout classic turns it off and GNOME has its Super+N back"
else
    not_ok "classic did not give the key back" "enabled: $(gsettings_get "${TILING}" enabled)," \
        "focus-active-notification: $(gsettings_get org.gnome.shell.keybindings focus-active-notification)"
fi
in_session "ame desktop layout tiling" >/dev/null 2>&1
if settles org.gnome.shell.keybindings focus-active-notification "@as []" && [[ "$(gsettings_get "${TILING}" enabled)" == true ]]; then
    ok "ame desktop layout tiling takes it again"
else
    not_ok "tiling did not take the key again" "focus-active-notification: $(gsettings_get org.gnome.shell.keybindings focus-active-notification)"
fi

# --- The icon packs --------------------------------------------------------------------------------------------------
icons() { gsettings_get org.gnome.desktop.interface icon-theme; }
[[ "$(in_session "amethystora-theme current" 2>/dev/null)" == amethystora ]] || in_session "amethystora-theme set amethystora" >/dev/null 2>&1
check "Glass is the icon theme on the dark theme" test "$(icons)" == "'amethystora-glass'"
in_session "ame desktop icons neon" >/dev/null 2>&1
check "ame desktop icons neon picks Neon" test "$(icons)" == "'amethystora-neon'"
in_session "amethystora-theme set amethystora" >/dev/null 2>&1
check "and a theme switch leaves the account's choice alone" test "$(icons)" == "'amethystora-neon'"
in_session "ame desktop icons theme" >/dev/null 2>&1
check "ame desktop icons theme goes back to the theme's own" test "$(icons)" == "'amethystora-glass'"
in_session "amethystora-theme set amethystora-light" >/dev/null 2>&1
check "Glass for light is the icon theme on the light theme" test "$(icons)" == "'amethystora-glass-light'"
in_session "amethystora-theme set amethystora" >/dev/null 2>&1
check "and Glass again on the dark one" test "$(icons)" == "'amethystora-glass'"
if in_session "ame desktop icons candy" >/dev/null 2>&1; then
    not_ok "ame desktop icons took candy-icons, which the image no longer has"
else
    ok "ame desktop icons refuses candy-icons"
fi
[[ ! -e /usr/share/icons/candy-icons ]] && ok "candy-icons is not installed" || not_ok "candy-icons is still installed"
# GTK itself finds what every launcher of the image names, in each pack
mkdir -p "${OWN}"
cat >"${OWN}/icons.py" <<'EOF'
import glob, re, sys
import gi
gi.require_version('Gtk', '4.0')
from gi.repository import Gtk
Gtk.init_check()
names = set()
for path in glob.glob('/usr/share/applications/amethystora-*.desktop'):
    for line in open(path, encoding='utf-8'):
        m = re.match(r'Icon=(\S+)$', line)
        if m and not m.group(1).startswith('/'):
            names.add(m.group(1))
names |= {'folder', 'text-x-generic', 'application-pdf', 'user-trash', 'org.gnome.Nautilus', 'org.gnome.Settings'}
# Other makers' apps, by what they do, and the file types and dialogs drawn since 2026-10-24
names |= {'htop', 'nvtop', 'org.gnome.Shell.Extensions', 'org.kde.konsole', 'org.gnome.Geary', 'text-x-python',
          'application-zip', 'dialog-warning', 'network-wireless'}
bad = 0
for pack in ('glass', 'glass-light', 'neon', 'neon-light', 'duotone', 'line', 'clay', 'muted'):
    theme = Gtk.IconTheme()
    theme.set_search_path(['/usr/share/icons'])
    theme.set_theme_name('amethystora-' + pack)
    own = '/usr/share/icons/amethystora-' + pack + '/'
    # The pack itself has the file, not Adwaita or hicolor through inheritance: has_icon says yes to a pack with its
    # icons deleted

    def from_pack(n):
        icon = theme.lookup_icon(n, None, 48, 1, Gtk.TextDirection.NONE, 0)
        f = icon.get_file() if icon else None
        return bool(f and (f.get_path() or '').startswith(own))
    missing = sorted(n for n in names if not from_pack(n))
    print(pack, 'missing:', missing)
    bad += len(missing)
    # A maker's logo is never drawn: Firefox keeps the icon in its package
    if from_pack('firefox') or from_pack('org.mozilla.firefox'):
        print(pack, 'draws Firefox')
        bad += 1
sys.exit(1 if bad else 0)
EOF
chmod -R a+rX "${OWN}"
out="$(user_bus python3 "${OWN}/icons.py" 2>&1)"
status=$?
if [[ ${status} == 0 ]]; then
    ok "GTK finds every launcher's icon, and the common ones, in each of the eight packs, and Firefox keeps its own"
else
    not_ok "GTK cannot find an icon in a pack" "${out}"
fi

# --- The window lights ------------------------------------------------------------------------------------------------
for entry in "gtk-4.0/gtk.css windowcontrols" "gtk-3.0/gtk.css titlebutton.close"; do
    read -r file marker <<<"${entry}"
    if grep -q "${marker}" "${HOME_DIR}/.config/${file}" 2>/dev/null; then
        ok "the account's ${file} draws the window lights"
    else
        not_ok "the account's ${file} does not draw the window lights" "$(head -c 400 "${HOME_DIR}/.config/${file}" 2>&1)"
    fi
done
check "the buttons are on the right, close last" \
    test "$(gsettings_get org.gnome.desktop.wm.preferences button-layout)" == "':minimize,maximize,close'"

# --- The dock ------------------------------------------------------------------------------------------------------------
install -m 0644 "$(dirname "$0")/atspi.py" "${OWN}/"
chmod -R a+rX "${OWN}"
dock="$(user_bus python3 - <<'EOF' 2>&1
import sys
sys.path.insert(0, "/run/vmlab-27")
from atspi import Atspi, application, walk

shell = application("gnome-shell")
shown = []
for node, _ in walk(shell):
    try:
        if not node.get_state_set().contains(Atspi.StateType.SHOWING):
            continue
        box = Atspi.Component.get_extents(node, Atspi.CoordType.SCREEN)
    except Exception:
        continue
    if node.get_name():
        shown.append((node.get_name(), box.x, box.y, box.width, box.height))
bottom = max((y + h for _, _, y, _, h in shown), default=0)
files = [item for item in shown if item[0] in ("Files", "Firefox", "Show Apps", "Trash") and item[2] > bottom * 0.7]
print(bottom, [item[0] for item in files])
sys.exit(0 if files else 1)
EOF
)"
if [[ $? == 0 ]]; then
    ok "the dock is at the bottom of the screen (${dock})"
else
    not_ok "no dock at the bottom of the screen in the shell's accessibility tree" "${dock}"
fi

# --- The glass ------------------------------------------------------------------------------------------------------------
# Its veils and text are the theme's (shell-glass.css), and it says in the log when the GPU gave it no picture to cut
# the panes from, which leaves every surface a bare veil over whatever is behind it
glass_css="${HOME_DIR}/.config/amethystora/current/theme/shell-glass.css"
if [[ -s "${glass_css}" ]] && ! grep -q '{{' "${glass_css}"; then
    ok "the theme has the glass's colours"
else
    not_ok "the theme has no rendered shell-glass.css" "$(ls "$(dirname "${glass_css}")" 2>&1 | paste -sd ' ')"
fi
log="$(journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --no-pager -o cat 2>/dev/null)"
if grep -q 'the glass has no picture' <<<"${log}"; then
    not_ok "the glass had no picture of a screen" "$(grep 'the glass has no picture' <<<"${log}" | head -n 3)"
else
    ok "the glass has its picture of the screen"
fi

# --- Left open for the screenshot ----------------------------------------------------------------------------------------
for unit in vmlab-27-one vmlab-27-two; do
    user_bus systemd-run --user --quiet --collect --unit="${unit}" -- nautilus --new-window "${HOME_DIR}" >/dev/null 2>&1 || true
    sleep 3
done
# GNOME's Super+S: the quick settings, on glass over the windows
python3 "$(dirname "$0")/keys.py" super+s >/dev/null 2>&1 || true
sleep 2
finish
