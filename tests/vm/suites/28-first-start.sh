#!/usr/bin/bash
# First start (plans/beyond/spec-phone-files-first.md 1.7-1.11, 4, 5; acceptance 9-23), as a new account sees it:
#   - the reveal at login: once per session start, never after unlocking, not with animations off or its key off, and
#     nothing of it left behind (the top bar whole, no sky);
#   - Pick a look, once, on top of the Manual's welcome page, without the sidebar, its picks running Control's commands,
#     and the offer of classic windows giving way to it;
#   - the Manual's moving cards, playing only with animations on, a click pausing one;
#   - the tips' rules, driven through the hints module, and that a tip never takes a click or the keyboard;
#   - the app grid's six folders and the row of the apps used most.
# The lab renders in software, where GNOME turns animations off: the new account's shell is started with
# --force-animations (a drop-in of its own user unit), and the lab's Eval extension is its, as in the other suites.
# vmlab-paths: @ui/*hints* @ui/*hello* @ui/*firstlook* @ui/*appgrid* @sf/usr/share/amethystora/user-setup.hooks.d/* @sf/usr/libexec/amethystora-layout-offer @sf/*just/desktop.just @sf/usr/share/gnome-shell/extensions/amethystora-transitions* @sf/usr/libexec/amethystora-if-found @ui/modules/found.js apps/control/src/welcome.rs
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
TRANSITIONS=org.gnome.shell.extensions.amethystora-transitions
ORIGINAL="${TEST_USER}"
NEW=vmlab-first
HOME_NEW="/var/home/${NEW}"
here="$(dirname "$0")"
keys() { python3 "${here}/keys.py" "$@"; }
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        sed -E -e "s/^\(true, '(.*)'\)\$/\1/" -e 's/^"(.*)"$/\1/'
}
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
gs() { in_session "gsettings $*" 2>/dev/null; }
atspi() { user_bus python3 "${here}/atspi.py" "$@"; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
H="${UI}.get('hints')"
shell_up() { [[ "$(ev 'Main.layoutManager._startingUp === false')" == true ]]; }
# Counted from this run on: the account is made again with the same number each run, and the boot keeps its lines
SUITE_SINCE="$(date '+%Y-%m-%d %H:%M:%S')"
reveal_lines() { journalctl -b --since "${SUITE_SINCE}" "_UID=$(id -u "${NEW}")" --no-pager -o cat 2>/dev/null | grep -c "Amethystora Transitions: reveal"; }
log_in() {
    printf '[daemon]\nAutomaticLoginEnable=True\nAutomaticLogin=%s\n' "$1" >/etc/gdm/custom.conf
    loginctl unlock-sessions
    systemctl restart gdm
    wait_for 240 has_session && wait_for 120 shell_up
}
# Back to the lab's own account, and the suite's own taken away, whatever happened (a failure, the phase's limit)
cleanup() {
    TEST_USER="${ORIGINAL}"
    printf '[daemon]\nAutomaticLoginEnable=True\nAutomaticLogin=%s\n' "${ORIGINAL}" >/etc/gdm/custom.conf
    [[ -n "${MONITOR:-}" ]] && kill "${MONITOR}" 2>/dev/null
    [[ -n "${CLICKER:-}" ]] && pkill -u "${NEW}" -f vmlab-28-clicker 2>/dev/null
    systemctl restart gdm
    loginctl terminate-user "${NEW}" 2>/dev/null
    sleep 2
    userdel -r "${NEW}" >/dev/null 2>&1
    rm -f /run/vmlab-28-*
    wait_for 240 has_session
}
trap cleanup EXIT
# Until the first-login setup has come to hook 25 (its hooks run one after another; 11-theme renders a theme)
hook_ran() { [[ "$(jq -r '.version.user."first-look"' "${HOME_NEW}/.local/share/amethystora/setup_versioning.json" 2>/dev/null)" == 1 ]]; }

# --- 16: the lab's account, made before this suite ------------------------------------------------------------------
versions="/var/home/${ORIGINAL}/.local/share/amethystora/setup_versioning.json"
same "16 the existing account recorded first-look 1" "$(jq -r '.version.user."first-look"' "${versions}" 2>/dev/null)" 1
same "16 ...and welcome stays at 1 (17-welcome.sh unchanged)" "$(jq -r '.version.user.welcome' "${versions}" 2>/dev/null)" 1
# 10-account.sh records the hook as run for it, as for an account that existed before the hook came
no_first_look() { ! test -e "/var/home/${ORIGINAL}/.local/state/amethystora/first-look.json"; }
check "16 the existing account had no Pick a look" no_first_look

# --- A new account, its first login ---------------------------------------------------------------------------------
userdel -r "${NEW}" >/dev/null 2>&1
useradd -m -c "First start" "${NEW}"
echo "${NEW}:${NEW}" | chpasswd
install -d -o "${NEW}" -g "${NEW}" "${HOME_NEW}/.config"
install -o "${NEW}" -g "${NEW}" /dev/null "${HOME_NEW}/.config/gnome-initial-setup-done"
# The lab's Eval and a shell with animations forced, both the account's own and never in the image
install -d -o "${NEW}" -g "${NEW}" "${HOME_NEW}/.local/share/gnome-shell/extensions/vmlab-eval@lab"
cat >"${HOME_NEW}/.local/share/gnome-shell/extensions/vmlab-eval@lab/metadata.json" <<'EOF'
{"uuid": "vmlab-eval@lab", "name": "VM lab", "description": "The lab's Eval", "shell-version": ["49", "50", "51"]}
EOF
cat >"${HOME_NEW}/.local/share/gnome-shell/extensions/vmlab-eval@lab/extension.js" <<'EOF'
export default class { enable() { global.context.unsafe_mode = true; } disable() {} }
EOF
# GNOME 50 runs the shell from the template org.gnome.Shell@.service, instance user (there is no @wayland unit any
# more): the drop-in is the instance's, and its ExecStart the template's own (%i expands in it)
start="$(sed -n 's/^ExecStart=//p' /usr/lib/systemd/user/org.gnome.Shell@.service | head -n 1)"
[[ -n "${start}" ]] || not_ok "GNOME Shell's user unit has no ExecStart to force animations with"
install -d "${HOME_NEW}/.config/systemd/user/org.gnome.Shell@user.service.d"
printf '[Service]\nExecStart=\nExecStart=%s --force-animations\n' "${start}" \
    >"${HOME_NEW}/.config/systemd/user/org.gnome.Shell@user.service.d/vmlab-animations.conf"
defaults="$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell enabled-extensions)"
# Fedora 45 has no dbus-run-session: the account's dconf database is compiled from a key file instead
dconf_src="$(mktemp -d)"
printf '[org/gnome/shell]\nenabled-extensions=%s\ndisable-user-extensions=false\n\n[org/gnome/shell/extensions/amethystora-ui/hints]\nenabled=true\n' \
    "${defaults%]*}, 'vmlab-eval@lab']" >"${dconf_src}/00-vmlab"
install -d -o "${NEW}" -g "${NEW}" "${HOME_NEW}/.config/dconf"
dconf compile "${HOME_NEW}/.config/dconf/user" "${dconf_src}"
rm -rf "${dconf_src}"
chown -R "${NEW}:" "${HOME_NEW}/.local" "${HOME_NEW}/.config"
TEST_USER="${NEW}"
if ! log_in "${NEW}"; then
    not_ok "the new account did not get a session" "$(journalctl -b -u gdm --no-pager -o cat | tail -n 10)"
    finish
fi
login_at=${SECONDS}
user_bus dbus-monitor "interface='org.freedesktop.Notifications',member='Notify'" >/run/vmlab-28-notify 2>&1 &
MONITOR=$!

# --- 9: the reveal ran once, and left nothing behind -----------------------------------------------------------------
sleep 3
echo "# the new account's shell: $(pgrep -a -u "${NEW}" gnome-shell | head -n 1 | cut -c1-120); animations $(ev 'imports.gi.St.Settings.get().enable_animations'); transitions $(ev "Main.extensionManager.lookup('amethystora-transitions@arsslensoft.github.io')?.state"); ui $(ev "Main.extensionManager.lookup('${UUID}')?.state")"
# The reveal plays once GNOME has finished starting, which the lab takes its time over
revealed() { (($(reveal_lines) >= 1)); }
wait_for 30 revealed
same "9 the reveal ran once at this login" "$(reveal_lines)" 1
same "9 the reveal ends on the desktop, not in the overview GNOME starts in" "$(ev "Main.overview.visible")" false
same "9 the top bar is whole" "$(ev 'Main.layoutManager.panelBox.opacity')" 255
same "9 no sky is left in the shell's top group" \
    "$(ev "Main.layoutManager.uiGroup.get_children().filter(c => c.name === 'amethystoraRevealCover').length")" 0

# The greeting (R7-24) holds the keyboard over GNOME's own welcome dialog of a first login, and any key goes straight on: the
# first Escape below may only end it, which the second then does for the dialog
# GNOME's own welcome dialog of a first login (it offers the Tour, which the image keeps), closed as a person would,
# with Escape, as 10-account.sh does for the lab's account: while it is up it is modal, so nothing else has the keyboard
# and no tip may show
shows_welcome() { atspi gnome-shell has "Take Tour"; }
welcome_gone() { ! atspi gnome-shell has "Take Tour"; }
if wait_for 60 shows_welcome; then
    sleep 2
    keys esc
    wait_for 10 welcome_gone || keys esc
    wait_for 20 welcome_gone || not_ok "GNOME's welcome dialog did not close with Escape"
fi

# --- 12: Pick a look, on top of the Manual's welcome -----------------------------------------------------------------
wait_for 300 hook_ran || not_ok "12 the first-login setup did not reach the first-look hook" "$(user_systemctl status amethystora-user-setup.service | tail -n 5)"
if atspi amethystora-control wait 15 "Pick a look" >/dev/null 2>&1; then
    ok "12 Control opened on Pick a look"
    control_focused() { [[ "$(ev "global.display.focus_window?.get_wm_class() ?? ''")" == org.amethystora.Control ]]; }
    check "12 ...with the focus, over the Manual" wait_for 10 control_focused
    no_sidebar() { ! atspi amethystora-control has "Your setup" >/dev/null 2>&1; }
    check "12 ...without its sidebar" no_sidebar
else
    not_ok "12 Control did not open on Pick a look" "$(atspi amethystora-control text 2>&1 | head -n 5)"
fi
first_look="${HOME_NEW}/.local/state/amethystora/first-look.json"
same "12 first-look.json says version 1" "$(jq -r .version "${first_look}" 2>/dev/null)" 1
test -s "${first_look}.bak" && ok "12 ...with its copy" || not_ok "12 first-look.json has no copy"

# --- 13: a pick runs the entry's command -----------------------------------------------------------------------------
atspi amethystora-control click Classic >/dev/null 2>&1
classic() { [[ "$(gs get org.gnome.shell.extensions.amethystora-ui.tiling enabled)" == false ]]; }
check "13 Classic sets the tiling off" wait_for 6 classic
atspi amethystora-control click "Tokyo Night" >/dev/null 2>&1
tokyo() { [[ "$(cat "${HOME_NEW}/.config/amethystora/current/theme.name" 2>/dev/null)" == tokyo-night ]]; }
check "13 a theme picked is applied" wait_for 60 tokyo
# By its role: each pick that ran says "Done" beside its row as well, a label found before the button
atspi amethystora-control click Done button >/dev/null 2>&1
check "13 Done closes the window" wait_for 10 sh -c "! pgrep -u ${NEW} -f 'amethystora-control welcome'"

# --- 17: the Manual's moving cards -----------------------------------------------------------------------------------
user_bus systemd-run --user --collect --quiet amethystora-manual "desktop#gestures-and-keys" >/dev/null
# The cards by name, each once: the page holds the same eight cards again further down (15 in all, the first one
# only once), and the tree also names each as a table cell
cards_named() { atspi amethystora-manual tree 2>/dev/null | grep -oE "button \['[^']*'\]" | grep -E "$1" | sort -u | wc -l; }
if atspi amethystora-manual wait 20 "Four fingers for Today" >/dev/null 2>&1; then
    same "17 eight cards" "$(cards_named 'button.*(Shake to find|Pinch for the apps|Search anything|Hold Super|Switch windows|Snap layouts|Spread for the desktop|Four fingers for Today), (playing|paused)')" 8
    # A clip plays only in the active window: the Manual is given the focus, as a person's click would
    ev "Main.overview.hide(); global.display.list_all_windows().find(w => w.get_wm_class() === 'org.amethystora.Manual')?.activate(global.get_current_time()); true" >/dev/null
    playing() { atspi amethystora-manual has "Shake to find, playing" >/dev/null 2>&1; }
    if wait_for 10 playing; then
        ok "17 with animations on, the first card plays"
    else
        not_ok "17 with animations on, the first card plays" \
            "focus: $(ev "global.display.focus_window?.get_wm_class() ?? 'none'")" \
            "GTK's animations and reduced motion: $(in_session "gjs -c \"imports.gi.versions.Gtk = '4.0'; const {Gtk} = imports.gi; Gtk.init(); const s = Gtk.Settings.get_default(); print(s.gtk_enable_animations, s.gtk_interface_reduced_motion);\"" 2>&1)" \
            "enable-animations: $(gs get org.gnome.desktop.interface enable-animations); reduced-motion: $(gs get org.gnome.desktop.a11y.interface reduced-motion)" \
            "$(atspi amethystora-manual tree 2>/dev/null | grep -oE "button \['[^']*, (playing|paused)'\]" | sort -u | head -n 9)"
    fi
    atspi amethystora-manual click "Shake to find, playing" >/dev/null 2>&1
    paused() { atspi amethystora-manual has "Shake to find, paused" >/dev/null 2>&1; }
    check "17 a click pauses it" wait_for 6 paused
else
    not_ok "17 the Manual did not show the cards" "$(atspi amethystora-manual text 2>&1 | head -n 5)"
fi
pkill -u "${NEW}" -f amethystora-manual
gs set org.gnome.desktop.interface enable-animations false
sleep 1
user_bus systemd-run --user --collect --quiet amethystora-manual "desktop#gestures-and-keys" >/dev/null
if atspi amethystora-manual wait 20 "Four fingers for Today" >/dev/null 2>&1; then
    ev "global.display.list_all_windows().find(w => w.get_wm_class() === 'org.amethystora.Manual')?.activate(global.get_current_time()); true" >/dev/null
    sleep 3
    same "17 with animations off, every card is still" "$(cards_named 'button.*, paused')" 8
else
    not_ok "17 the Manual did not show the cards with animations off"
fi
pkill -u "${NEW}" -f amethystora-manual
gs set org.gnome.desktop.interface enable-animations true

# --- 14: no offer of classic windows ----------------------------------------------------------------------------------
offer_ran() {
    journalctl -b "_UID=$(id -u "${NEW}")" --no-pager -o cat 2>/dev/null |
        grep -qE 'Started run-[^ ]*\.service.*amethystora-layout-offer'
}
check "14 the offer of classic windows has run" wait_for 300 offer_ran
sleep 5
check "14 no offer of classic windows came" sh -c "! grep -q 'Windows here tile side by side' /run/vmlab-28-notify"
kill "${MONITOR}" 2>/dev/null

# --- 18-20: the tips' rules -------------------------------------------------------------------------------------------
STATE="${HOME_NEW}/.local/state/amethystora/hints.json"
put_state() { printf '%s' "$1" >"${STATE}"; chown "${NEW}:" "${STATE}"; sleep 1; }
now=$(date +%s)
ev "Main.overview.hide(); true" >/dev/null; sleep 2
put_state "{\"version\":1,\"since\":${now},\"last\":0,\"shown\":{}}"
same "18 none in the first hour" "$(ev "${H}.offer('shake', {text: 'x', anchor: 'pointer'})")" false
put_state "{\"version\":1,\"since\":$((now - 3660)),\"last\":0,\"shown\":{}}"
same "18 after an hour, a tip shows" "$(ev "${H}.offer('shake', {text: 'Tip: shake the mouse to find the pointer.', anchor: 'pointer'})")" true
same "18 ...in the shell's top group" "$(ev "Main.layoutManager.uiGroup.contains(${H}._card)")" true
same "18 never two at once" "$(ev "${H}.offer('pinch', {text: 'x'})")" false
same "18 shown is recorded" "$(jq -r '.shown | has("shake")' "${STATE}")" true
same "19 the tip has not taken the keyboard" "$(ev "!global.stage.key_focus || !${H}._card.contains(global.stage.key_focus)")" true
same "19 no grab while it shows" "$(ev "global.stage.get_grab_actor?.() ?? null")" null
same "19 no modal dialog while it shows" "$(ev "Main.modalCount")" 0
ev "${H}._card.get_children()[1].emit('clicked', 1); true" >/dev/null
sleep 0.5
same "18 Got it closes the card" "$(ev "!!${H}._card")" false
pill_gone() { [[ "$(ev "!!${H}._pill")" == false ]]; }
check "18 ...and the pill goes" wait_for 3 pill_gone
same "18 the same tip never again" "$(ev "${H}.offer('shake', {text: 'x'})")" false
put_state "{\"version\":1,\"since\":$((now - 7200)),\"last\":$((now - 1860)),\"shown\":{\"shake\":$((now - 1860))}}"
same "18 31 minutes later, another" "$(ev "${H}.offer('pinch', {text: 'x'})")" true
ev "${H}._close(true); true" >/dev/null
put_state "{\"version\":1,\"since\":$((now - 7200)),\"last\":$((now - 1740)),\"shown\":{}}"
same "18 29 minutes later, none" "$(ev "${H}.offer('pinch', {text: 'x'})")" false
# 19: a window that counts its clicks, under a tip at the top of the screen; a click beside the card reaches it
cat >/run/vmlab-28-clicker.py <<'EOF'
import sys
import gi
gi.require_version("Gtk", "4.0")
from gi.repository import Gtk
out = sys.argv[1]
count = [0]
def clicked(*_):
    count[0] += 1
    with open(out, "w") as f:
        f.write(str(count[0]))
def activate(app):
    window = Gtk.ApplicationWindow(application=app, title="vmlab clicker")
    gesture = Gtk.GestureClick()
    gesture.connect("pressed", clicked)
    window.add_controller(gesture)
    window.maximize()
    window.present()
app = Gtk.Application(application_id="org.vmlab.Clicker")
app.connect("activate", activate)
app.run([])
EOF
chmod 0644 /run/vmlab-28-clicker.py
CLICKS="${HOME_NEW}/vmlab-28-clicks"
# A unit of its own: what in_session leaves running in the background goes with its unit when the command ends
user_bus systemd-run --user --collect --quiet --unit="vmlab-28-clicker-$$" python3 /run/vmlab-28-clicker.py "${CLICKS}" >/dev/null
CLICKER=1
clicker_up() { [[ "$(ev "global.display.focus_window?.get_title() ?? ''")" == "vmlab clicker" ]]; }
wait_for 20 clicker_up
put_state "{\"version\":1,\"since\":$((now - 7200)),\"last\":0,\"shown\":{}}"
same "19 a tip shows over the window" "$(ev "${H}.offer('search', {text: 'Tip: Super+Space searches apps, files and settings.'})")" true
keys click:0.25,0.7
clicked() { [[ "$(cat "${CLICKS}" 2>/dev/null)" == 1 ]]; }
check "19 a click beside the tip reaches the window under it" wait_for 5 clicked
same "19 ...and the tip is still there" "$(ev "!!${H}._card")" true
ev "${H}._close(true); true" >/dev/null
pkill -u "${NEW}" -f vmlab-28-clicker
CLICKER=""
in_session "ame desktop hints reset" >/dev/null 2>&1
same "20 reset empties what was shown" "$(jq -c .shown "${STATE}")" "{}"
same "20 ...in the copy too" "$(jq -c .shown "${STATE}.bak")" "{}"
in_session "ame desktop hints off" >/dev/null 2>&1
put_state "{\"version\":1,\"since\":$((now - 7200)),\"last\":0,\"shown\":{}}"
same "20 with tips off, none" "$(ev "${H}.offer('pinch', {text: 'x'})")" false
in_session "ame desktop hints on" >/dev/null 2>&1

# --- 21-23: the app grid ---------------------------------------------------------------------------------------------
same "21 the image's six folders" "$(gs get org.gnome.desktop.app-folders folder-children)" \
    "['Office', 'Graphics', 'Games', 'System', 'Development', 'Utilities']"
AD="Main.overview._overview.controls._appDisplay"
ev "Main.overview.showApps(); true" >/dev/null
sleep 3
# The Manual and Control opened by themselves at the first login, and GNOME records every app that has run: the row
# holds those and nothing else yet
same "23 the row holds only the apps that opened by themselves" \
    "$(ev "(() => { const ids = [...${UI}.get('appgrid')._icons.keys()]; return ids.length > 0 && ${UI}.get('appgrid')._row.visible && ids.every(id => ['amethystora-manual.desktop', 'amethystora-control.desktop'].includes(id)); })()")" true
items="$(ev "JSON.stringify(${AD}._orderedItems.map(i => (i._folder ? 'F:' : 'A:') + (i._folder ? i.name : i.app.get_id())))")"
for loose in amethystora-security amethystora-update amethystora-control amethystora-backups amethystora-manual; do
    [[ "${items}" == *"A:${loose}.desktop"* ]] && ok "21 ${loose} stays loose" || not_ok "21 ${loose} is not loose" "${items}"
done
for folder in Office System Utilities; do
    [[ "${items}" == *"F:"* && "$(ev "${AD}._orderedItems.some(i => i._folder && i.id === '${folder}')")" == true ]] &&
        ok "21 the ${folder} folder is there" || not_ok "21 no ${folder} folder" "${items}"
done
same "21 Notes is in Office" "$(ev "${AD}._orderedItems.find(i => i.id === 'Office')?.view?._orderedItems.some(i => i.app?.get_id() === 'amethystora-notes.desktop') ?? false")" true
ev "Main.overview.hide(); true" >/dev/null
for app in org.gnome.Nautilus amethystora-manual org.gnome.Settings; do
    in_session "gtk-launch ${app}" >/dev/null 2>&1 &
    sleep 4
    # GNOME counts an app's use while it has the focus
    ev "imports.gi.Shell.AppSystem.get_default().lookup_app('${app}.desktop')?.activate(); true" >/dev/null
    sleep 3
done
ev "Main.overview.showApps(); true" >/dev/null
sleep 3
same "22 the recent row shows after apps are used" "$(ev "${UI}.get('appgrid')._row.visible")" true
for app in org.gnome.Nautilus amethystora-manual org.gnome.Settings; do
    same "22 ${app} is in the row" "$(ev "${UI}.get('appgrid')._icons.has('${app}.desktop')")" true
done
in_session "ame desktop app-grid recent off" >/dev/null 2>&1
same "22 recent off takes the row away" "$(ev "${UI}.get('appgrid')._row === null")" true
in_session "ame desktop app-grid recent on" >/dev/null 2>&1
same "22 on brings it back" "$(ev "!!${UI}.get('appgrid')._row")" true
sleep 2
# Added while the grid was already open, the row and labels still find its first column: nothing tells them it settled
same "22 the labels and the row line up with the grid's first column" \
    "$(ev "(() => { const g = ${UI}.get('appgrid'); const scale = imports.gi.St.ThemeContext.get_for_stage(global.stage).scale_factor; const first = ${AD}._orderedItems.find(i => i.visible); const rtl = ${AD}._box.get_text_direction() === imports.gi.Clutter.TextDirection.RTL; const inset = rtl ? ${AD}._scrollView.width - first.get_transformed_position()[0] - first.width : first.get_transformed_position()[0]; const want = 'padding-' + (rtl ? 'right' : 'left') + ': ' + Math.round(inset / scale) + 'px;'; return g._recentLabel.get_style() === want && g._allLabel.get_style() === want && g._row.get_style().startsWith(want) && inset > 0; })()")" true
# A second page only once the first is full: a saved layout with one app on its first page closes up, in its order
ids="$(ev "JSON.stringify(${AD}._orderedItems.filter(i => i.visible).map(i => i.id))")"
ev "(() => { const G = imports.gi.GLib; const ids = ${AD}._orderedItems.filter(i => i.visible).map(i => i.id); const pack = list => Object.fromEntries(list.map((id, i) => [id, new G.Variant('a{sv}', {position: G.Variant.new_int32(i)})])); global.settings.set_value('app-picker-layout', new G.Variant('aa{sv}', [pack(ids.slice(0, 1)), pack(ids.slice(1))])); return true; })()" >/dev/null
sleep 2
same "22 a saved page with room is filled from the next, and no arrow or dots for one page" \
    "$(ev "(() => { const g = ${AD}._grid; const n = ${AD}._orderedItems.filter(i => i.visible).length; return g.getItemsAtPage(0).filter(i => i.visible).length === Math.min(n, g.itemsPerPage) && g.nPages === Math.ceil(n / g.itemsPerPage) && (g.nPages > 1 || !(${AD}._pageIndicators.visible || ${AD}._nextPageArrow.visible)); })()")" true
same "22 ...in the same order" "$(ev "JSON.stringify(${AD}._orderedItems.filter(i => i.visible).map(i => i.id))")" "${ids}"
gs reset org.gnome.shell app-picker-layout
ev "Main.overview.hide(); true" >/dev/null

# --- 10: locking the screen is not a session start -------------------------------------------------------------------
before="$(reveal_lines)"
loginctl lock-sessions
sleep 5
loginctl unlock-sessions
sleep 5
same "10 no reveal after unlocking" "$(reveal_lines)" "${before}"

# --- 15 and 10: the next login --------------------------------------------------------------------------------------
in_session "pkill -f amethystora-control" >/dev/null 2>&1
log_in "${NEW}"
sleep 15
check "15 Pick a look does not open again" sh -c "! pgrep -u ${NEW} -f 'amethystora-control welcome'"
same "15 first-look is recorded" "$(jq -r '.version.user."first-look"' "${HOME_NEW}/.local/share/amethystora/setup_versioning.json")" 1
revealed_again() { (($(reveal_lines) > before)); }
wait_for 30 revealed_again
same "10 a new session reveals once more" "$(reveal_lines)" $((before + 1))

# --- 11: with animations off, or the reveal off, nothing ------------------------------------------------------------
before="$(reveal_lines)"
gs set org.gnome.desktop.interface enable-animations false
log_in "${NEW}"
sleep 3
same "11 no reveal with animations off" "$(reveal_lines)" "${before}"
same "11 ...and the top bar whole at once" "$(ev 'Main.layoutManager.panelBox.opacity')" 255
gs set org.gnome.desktop.interface enable-animations true
in_session "ame desktop transition reveal off" >/dev/null 2>&1
log_in "${NEW}"
sleep 3
same "11 no reveal with the reveal off" "$(reveal_lines)" "${before}"
in_session "ame desktop transition reveal on" >/dev/null 2>&1

# 24: a change after the login reveal opens its frozen picture, and takes it away. Only this shell has animations in
# the lab, and without them nothing ever freezes (a reveal that threw left the old picture over the desktop for good)
T="Main.extensionManager.lookup('amethystora-transitions@arsslensoft.github.io').stateObj"
frozen() { [[ "$(ev "${T}._frozen")" == true ]]; }
shown() { [[ "$(ev 'Main.layoutManager.screenTransition.visible')" == true ]]; }
open() { ! frozen && ! shown; }
scheme="$(gs get org.gnome.desktop.interface color-scheme)"
gs set org.gnome.desktop.interface color-scheme "$([[ "${scheme}" == "'prefer-light'" ]] && echo default || echo prefer-light)"
check "24 a change freezes the screen" wait_for 5 frozen
check "24 ...and the reveal takes the picture away" wait_for 15 open
same "24 ...without an error" "$(journalctl -b --since "${SUITE_SINCE}" "_UID=$(id -u "${NEW}")" --no-pager -o cat 2>/dev/null | grep -c 'is not a function')" 0
gs set org.gnome.desktop.interface color-scheme "${scheme}"
wait_for 15 open
# A reveal that throws still gives the screen back within the five seconds the extension promises
ev "${T}._realReveal = ${T}._reveal; ${T}._reveal = () => { throw new Error('vmlab'); }; 1" >/dev/null
gs set org.gnome.desktop.interface color-scheme "$([[ "${scheme}" == "'prefer-light'" ]] && echo default || echo prefer-light)"
check "24 a reveal that throws freezes the screen" wait_for 5 frozen
check "24 ...and still gives it back" wait_for 8 open
ev "delete ${T}._reveal; delete ${T}._realReveal; 1" >/dev/null
gs set org.gnome.desktop.interface color-scheme "${scheme}"
wait_for 15 open
# The errors of that throw were made on purpose: the shell's faults are counted from here
SUITE_BEGAN="$(date '+%Y-%m-%d %H:%M:%S')"

# 25: a settings object nobody disposes is never collected: GJS sweeping one while dconf's worker dispatches a change to it
# deadlocks the shell for good (seen in this suite, from GNOME Shell's own). An object beside it is, so that the collector ran
ev "globalThis.vmlabGone = {settings: false, control: false};
    globalThis.vmlabWatch = new FinalizationRegistry(what => { globalThis.vmlabGone[what] = true; });
    (() => { const {Gio} = imports.gi;
        vmlabWatch.register(new Gio.Settings({schema_id: 'org.gnome.desktop.interface'}), 'settings');
        vmlabWatch.register({}, 'control'); })(); 1" >/dev/null
collected() { ev 'imports.system.gc(); globalThis.vmlabGone.control' | grep -q true; }
check "25 the collector ran" wait_for 20 collected
same "25 ...and left the settings object alone" "$(ev 'globalThis.vmlabGone.settings')" false

finish
