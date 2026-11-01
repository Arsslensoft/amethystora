#!/usr/bin/bash
# The tiling's settings (modules/tiling/config.js, the extension's prefs.js), as a person's session has them. The lab
# sees no window's place on Wayland, so it asks what the session can say for sure:
#   - every setting has the image's value, and the shortcuts are the 100 of the schema, 54 of them bound;
#   - every setting changed while the tiling runs, and put back, leaves nothing thrown in the shell's log;
#   - ame desktop tiling-settings opens the settings window, with its five pages, and its switch turns the tiling off
#     and on again, as ame desktop layout does;
#   - a shortcut bound while the tiling runs works at once: on a real keyboard (uinput), Super+Left, the new
#     "previous window" key and the live switcher move the keyboard between Files windows, which Ctrl+W then closes.
# The lab's machine has one screen, so a workspace on each screen (screens.js) is checked in the headless shell only.
# vmlab-paths: @ui/*tiling* @ui/prefs.js
source "$(dirname "$0")/lib.sh"

OWN=/run/vmlab-96
HOME_DIR="/var/home/${TEST_USER}"
TILING=org.gnome.shell.extensions.amethystora-ui.tiling
UUID=amethystora-ui@arsslensoft.github.io

if ! wait_for 300 has_session; then
    skip "no session for ${TEST_USER} (10-account.sh)"
    finish
fi

gsettings_get() { user_bus gsettings get "$1" "$2" 2>/dev/null; }
keys() { python3 "$(dirname "$0")/keys.py" "$@"; }
since="$(date '+%Y-%m-%d %H:%M:%S')"
shell_errors() {
    journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --since "${since}" --no-pager -o cat 2>/dev/null |
        grep -E "JS ERROR|Amethystora UI: the tiling" | head -n 10
}
mkdir -p "${OWN}"
install -m 0644 "$(dirname "$0")/atspi.py" "${OWN}/"

# --- The settings, as the image ships them ----------------------------------------------------------------------------
user_bus gsettings set "${TILING}" enabled true >/dev/null 2>&1
defaults_ok=true
for entry in window-gap=8 horizontal-margin=8 vertical-margin=8 vertical-margin-bottom=8 selection-border-size=10 \
    animation-time=0.25 default-focus-mode=0 open-window-position=0 edge-preview-enable=true gesture-enabled=true \
    gesture-horizontal-fingers=3 maximize-within-tiling=true "winprops=@as []" show-workspace-indicator=false \
    show-window-position-bar=false use-default-background=true overview-ensure-viewport-animation=1; do
    key="${entry%%=*}" want="${entry#*=}"
    got="$(gsettings_get "${TILING}" "${key}")"
    [[ "${got}" == "${want}" ]] || { defaults_ok=false; not_ok "${key} is ${got}, not the image's ${want}"; }
done
${defaults_ok} && ok "the tiling's settings have the image's values"
check "the schema has the 100 shortcuts" test "$(user_bus gsettings list-keys "${TILING}.keybindings" | wc -l)" == 100
check "54 of them are bound" \
    test "$(user_bus gsettings list-recursively "${TILING}.keybindings" | grep -vc '@as \[\]$')" == 54

# --- Every setting changed while the tiling runs ------------------------------------------------------------------------
# A value other than the image's for each, within its range, as KEY VALUE pairs; the shell has to take each at once
# and stay whole, and a window rule that is not JSON is only left out
changes=(
    window-gap 30 horizontal-margin 40 vertical-margin 24 vertical-margin-bottom 0 selection-border-size 0
    selection-border-radius-top 0 selection-border-radius-bottom 12 cycle-width-steps "[0.25, 0.75, 800.0]"
    cycle-height-steps "[0.5]" maximize-width-percent 0.8 maximize-within-tiling false default-focus-mode 1
    open-window-position 4 open-window-position-option-up true animation-time 0.5 drift-speed 5
    drag-drift-speed 5 minimap-scale 0.0 minimap-shade-opacity 0 edge-preview-enable false
    edge-preview-scale 0.3 edge-preview-click-enable false edge-preview-timeout-enable true
    edge-preview-timeout 300 edge-preview-timeout-continual true gesture-enabled false
    gesture-horizontal-fingers 4 gesture-workspace-fingers 0 swipe-sensitivity "[1.0, 1.0]" swipe-friction "[1.0, 0.5]"
    show-focus-mode-icon false show-open-position-icon false default-show-top-bar false
    topbar-mouse-scroll-enable true window-switcher-preview-scale 0.3
    winprops "['{\"wm_class\": \"org.gnome.Nautilus\", \"preferredWidth\": \"50%\"}', 'not json']"
    workspace-colors "['#314e6c', 'not a colour']"
    workspace-backgrounds "['/usr/share/backgrounds/no-such-picture.png', '']" default-background "/nowhere.jpg"
)
for ((i = 0; i < ${#changes[@]}; i += 2)); do
    user_bus gsettings set "${TILING}" "${changes[i]}" "${changes[i + 1]}" >/dev/null 2>&1 ||
        not_ok "gsettings would not set ${changes[i]} to ${changes[i + 1]}"
done
sleep 3
for ((i = 0; i < ${#changes[@]}; i += 2)); do
    user_bus gsettings reset "${TILING}" "${changes[i]}" >/dev/null 2>&1
done
sleep 3
errors="$(shell_errors)"
if [[ -z "${errors}" ]]; then
    ok "every setting changed and put back while the tiling ran, and the shell threw nothing"
else
    not_ok "the shell threw while the settings changed" "${errors}"
fi
check "and the tiling is still on" test "$(gsettings_get "${TILING}" enabled)" == true

# --- The settings window ------------------------------------------------------------------------------------------------
# Which app's accessibility tree holds a text, the settings window being an app of its own
cat >"${OWN}/find.py" <<'EOF'
import sys
sys.path.insert(0, "/run/vmlab-96")
from atspi import Atspi, walk

want = sys.argv[1]
desktop = Atspi.get_desktop(0)
for i in range(desktop.get_child_count()):
    app = desktop.get_child_at_index(i)
    if app is None:
        continue
    for node, _ in walk(app):
        try:
            if want in (node.get_name() or ""):
                print(app.get_name())
                sys.exit(0)
        except Exception:
            continue
sys.exit(1)
EOF
chmod -R a+rX "${OWN}"
keys click:0.5,0.5 >/dev/null 2>&1
in_session "ame desktop tiling-settings" >/dev/null 2>&1
app=""
for _ in $(seq 1 20); do
    app="$(user_bus python3 "${OWN}/find.py" "Tile windows" 2>/dev/null)" && break
    sleep 1
done
atspi() { user_bus python3 "${OWN}/atspi.py" "${app}" "$@"; }
if [[ -n "${app}" ]]; then
    ok "ame desktop tiling-settings opens the settings window (${app})"
    pages=true
    for page in Windows Motion Workspaces Keyboard "Window rules"; do
        atspi has "${page}" >/dev/null 2>&1 || { pages=false; not_ok "the settings window has no ${page} page"; }
    done
    ${pages} && ok "it has its five pages"
    # The page's tab, not the page itself, which carries the same name and cannot be pressed
    atspi click Keyboard "page tab" >/dev/null 2>&1
    if atspi wait 5 "Go to the column on the left" >/dev/null 2>&1; then
        ok "the keyboard page lists the shortcuts"
    else
        not_ok "the keyboard page does not list the shortcuts" "$(atspi text 2>&1 | head -n 20)"
    fi
    atspi click Windows "page tab" >/dev/null 2>&1
    # Read again at every try: the switch's change reaches dconf a moment after it is pressed
    tiling_is() { [[ "$(gsettings_get "${TILING}" enabled)" == "$1" ]]; }
    atspi check "Tile windows" off >/dev/null 2>&1
    if wait_for 10 tiling_is false; then
        ok "its switch turns the tiling off"
    else
        not_ok "its switch did not turn the tiling off" "enabled: $(gsettings_get "${TILING}" enabled)" \
            "$(atspi tree 2>&1 | grep -F "Tile windows" | head -n 4)"
    fi
    atspi check "Tile windows" on >/dev/null 2>&1
    if wait_for 10 tiling_is true; then
        ok "and on again"
    else
        not_ok "its switch did not turn the tiling on again" "enabled: $(gsettings_get "${TILING}" enabled)"
        in_session "ame desktop layout tiling" >/dev/null 2>&1
    fi
    # Closed as a person closes it, by its close button: killing an app's window is what suite 26 found leaves GNOME
    # taking no more input
    atspi click Close button >/dev/null 2>&1
    gone() { ! user_bus python3 "${OWN}/find.py" "Tile windows" >/dev/null 2>&1; }
    if wait_for 10 gone; then
        ok "its close button closes it"
    else
        not_ok "its close button did not close it; it was killed instead"
        pkill -u "${TEST_USER}" -f org.gnome.Shell.Extensions >/dev/null 2>&1 || true
    fi
else
    not_ok "ame desktop tiling-settings opened no settings window" "$(in_session "ame desktop tiling-settings" 2>&1 | tail -n 5)"
fi
sleep 2

# --- A shortcut bound while the tiling runs -------------------------------------------------------------------------------
# Files windows side by side, each on a folder of its own. GTK 4 tells the accessibility tree no window is active, so the
# window that has the keyboard is the one Ctrl+W closes: each key below is pressed, then Ctrl+W, and the window it
# should have gone to has to be the one gone, the other still there.
cat >"${OWN}/frames.py" <<'EOF'
import sys
sys.path.insert(0, "/run/vmlab-96")
from atspi import application

app = application("org.gnome.Nautilus")
for i in range(app.get_child_count() if app is not None else 0):
    print(app.get_child_at_index(i).get_name())
EOF
chmod -R a+rX "${OWN}"
frames() { user_bus python3 "${OWN}/frames.py" 2>/dev/null; }
# A Files window on a folder of its own, which has the keyboard once it is open
open_files() {
    mkdir -p "${HOME_DIR}/vmlab-96-$1"
    chown "${TEST_USER}:" "${HOME_DIR}/vmlab-96-$1"
    user_bus systemd-run --user --quiet --collect --unit="vmlab-96-$1" -- \
        nautilus --new-window "${HOME_DIR}/vmlab-96-$1" >/dev/null 2>&1 || true
    for _ in $(seq 1 20); do
        frames | grep -qx "vmlab-96-$1" && break
        sleep 1
    done
    sleep 2
}
# Ctrl+W, and whether it closed window $1 and left window $2 open
closed() {
    keys ctrl+w >/dev/null 2>&1
    for _ in $(seq 1 10); do
        frames | grep -qx "vmlab-96-$1" || break
        sleep 1
    done
    ! frames | grep -qx "vmlab-96-$1" && frames | grep -qx "vmlab-96-$2"
}
open_files a
open_files b
if frames | grep -qx vmlab-96-b; then
    keys super+left >/dev/null 2>&1
    sleep 2
    if closed a b; then
        ok "Super+Left gives the keyboard to the window on the left"
    else
        not_ok "Super+Left did not move the keyboard" "Files' windows: $(frames | paste -sd ' ')"
    fi
    # Previous window, unbound in the image: bound now, it works at once
    open_files c
    user_bus gsettings set "${TILING}.keybindings" switch-previous "['<Super><Control><Shift>j']" >/dev/null 2>&1
    sleep 2
    keys super+ctrl+shift+j >/dev/null 2>&1
    sleep 2
    if closed b c; then
        ok "a shortcut bound while the tiling runs (the previous window, Super+Ctrl+Shift+J) works at once"
    else
        not_ok "the shortcut bound for the previous window did nothing" "Files' windows: $(frames | paste -sd ' ')"
    fi
    user_bus gsettings reset "${TILING}.keybindings" switch-previous >/dev/null 2>&1
    # The live switcher, unbound too: one press goes to the window used before
    open_files d
    user_bus gsettings set "${TILING}.keybindings" live-alt-tab "['<Super><Control><Shift>k']" >/dev/null 2>&1
    sleep 2
    keys super+ctrl+shift+k >/dev/null 2>&1
    sleep 2
    if closed c d; then
        ok "the live switcher (Super+Ctrl+Shift+K) goes to the window used before"
    else
        not_ok "the live switcher did not go to the window used before" "Files' windows: $(frames | paste -sd ' ')"
    fi
    user_bus gsettings reset "${TILING}.keybindings" live-alt-tab >/dev/null 2>&1
    # The last one, d, has the keyboard now
    frames | grep -qx vmlab-96-d && keys ctrl+w >/dev/null 2>&1
else
    not_ok "no Files window opened" "Files' windows: $(frames | paste -sd ' ')"
fi
for side in a b c d; do
    user_bus systemctl --user stop "vmlab-96-${side}" >/dev/null 2>&1 || true
    rm -rf "${HOME_DIR}/vmlab-96-${side}"
done

errors="$(shell_errors)"
if [[ -z "${errors}" ]]; then
    ok "nothing of the tiling threw in the shell's log during the suite"
else
    not_ok "the shell threw during the suite" "${errors}"
fi
finish
