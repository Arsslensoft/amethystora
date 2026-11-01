#!/usr/bin/bash
# vmlab-vm: r4-notify
# Round 4, group notify (plans/beyond/spec-r4-notify.md, its acceptance criteria), in the test account's session, asked
# of GNOME Shell itself through Eval, which a lab-only extension of the test account opens (never in the image):
#   - Later and mute: the banner's buttons after the app's own (none, two and three of them), Later in an hour with its
#     Undo and back with the same buttons, the calendar's groups, a mute holding the shown, the queued and the new
#     banners but the urgent, Unmute's card, When I next unlock through a real lock, the key off forgetting everything,
#     nothing written;
#   - What's new: the image's own file and page, the pill saying Updated once after a version seen before (what the
#     releases newer than it brought), its card's three rows, Read it all opening the Manual, the pill back to the clock
#     after the minute, never again, nothing for a new account or after a rollback;
#   - Getting set in the calendar menu, its lines ticked by what the machine says, Hide for good;
#   - Power you can see: the battery's menu with a stand-in UPower (upower-standin.py), the busy chip from a real busy
#     app scope over two minutes, its card, Not now, said once an hour, and a chip that goes once its card closes;
#   - ten disables and enables leaving nothing, Control's entries and the setup file.
# A software-rendered lab shell leaves a banner queued until it is told to look again (Main.messageTray._updateState()):
# the suite does that (poke) where a banner should show, which a real session does by itself at its next event.
# vmlab-paths: @ui/extension.js @ui/*pill* @ui/*today* @ui/*power* @ui/*quick* @sf/*themed/shell-pill* @sf/*themed/shell-today* @sf/*themed/shell-power* @sf/*manual/whats-new.md build_files/shared/*whats-new* @sf/usr/libexec/amethystora-control-list @sf/*just/desktop.just
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HERE="$(dirname "$0")"
HOME_DIR="/var/home/${TEST_USER}"
PILL=org.gnome.shell.extensions.amethystora-ui.pill
TODAY=org.gnome.shell.extensions.amethystora-ui.today
POWER=org.gnome.shell.extensions.amethystora-ui.power
STATE="${HOME_DIR}/.local/state/amethystora"
WORK="$(mktemp -d)"
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        python3 -c 'import json, sys
from gi.repository import GLib
v = GLib.Variant.parse(None, sys.stdin.read().strip(), None, None)
try:
    x = json.loads(v[1])
except ValueError:
    x = v[1]
print(x if isinstance(x, str) else json.dumps(x))'
}
gs() { in_session "gsettings $*" 2>/dev/null; }
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
like() { [[ "$1" == $2 ]]; }
is() { [[ "$(ev "$1")" == "$2" ]]; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
P="${UI}.get('pill')"
L="${P}.hold.later"
B="${UI}.get('power').busy"
later() { ev "JSON.stringify(${L}.state()$1)"; }
# A notification from an app, as a transient service of the account's, so that one waiting for its buttons runs on
n=0
notify() {
    n=$((n + 1))
    user_bus systemd-run --user --collect --quiet --unit="vmlab-notify-${n}" -- timeout 600 notify-send "$@" >/dev/null 2>&1
}
poke() { ev "Main.messageTray._updateState(); 'x'" >/dev/null; }
banner_is() { poke; is "Main.messageTray._banner?.notification?.title ?? ''" "$1"; }
banner_not() { poke; ! is "Main.messageTray._banner?.notification?.title ?? ''" "$1"; }
in_list() { is "!!${L}?.find('$1')" true; }
expire() { ev "Main.messageTray._expireNotification(); 'x'" >/dev/null; wait_for 10 is "Main.messageTray._banner ? 1 : 0" 0; }
state_is() { is "Main.extensionManager.lookup('${UUID}').state" "$1"; }
since="$(date '+%Y-%m-%d %H:%M:%S')"

if ! has_session; then
    bash "${HERE}/10-account.sh" >/dev/null
fi
if ! wait_for 300 has_session; then
    not_ok "no session for ${TEST_USER}"
    finish
fi
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab"
cat >"${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab/metadata.json" <<'EOF'
{"uuid": "vmlab-eval@lab", "name": "VM lab", "description": "The lab's Eval", "shell-version": ["49", "50", "51"]}
EOF
cat >"${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab/extension.js" <<'EOF'
export default class { enable() { global.context.unsafe_mode = true; } disable() {} }
EOF
chown -R "${TEST_USER}:" "${HOME_DIR}/.local/share/gnome-shell"
enabled="$(user_bus gsettings get org.gnome.shell enabled-extensions)"
[[ ${enabled} == *vmlab-eval@lab* ]] || user_bus gsettings set org.gnome.shell enabled-extensions "${enabled%]*}, 'vmlab-eval@lab']"
for key in later later-times whats-new; do gs reset "${PILL}" "${key}"; done
gs reset "${TODAY}" getting-set
for key in battery-details busy-app; do gs reset "${POWER}" "${key}"; done
loginctl unlock-sessions
systemctl restart gdm
ready() { is "typeof ${L}?.state" function; }
if ! wait_for 240 has_session || ! wait_for 120 ready; then
    not_ok "Later did not start in the session"
    finish
fi
wait_for 30 is "Main.layoutManager._startingUp" false
ev "Main.overview.hide(); 'hidden'" >/dev/null
wait_for 10 is "Main.overview.visible" false
cleanup() {
    pkill -f upower-standin.py 2>/dev/null
    if [[ -e "${WORK}/upower-masked" ]]; then
        systemctl unmask --runtime upower.service >/dev/null 2>&1
        systemctl start upower.service >/dev/null 2>&1
    fi
    user_bus systemctl --user stop 'vmlab-notify-*' 'app-gnome-org.gnome.TextEditor-4242.scope' >/dev/null 2>&1
    for key in later later-times whats-new; do gs reset "${PILL}" "${key}"; done
    gs reset "${TODAY}" getting-set
    for key in battery-details busy-app; do gs reset "${POWER}" "${key}"; done
    rm -rf "${WORK}"
}
trap cleanup EXIT

# --- 1: the keys and their defaults ---------------------------------------------------------------------------------
same "1 later is on" "$(gs get "${PILL}" later)" true
same "1 later-times" "$(gs get "${PILL}" later-times)" "['18:00', '09:00']"
same "1 whats-new is on" "$(gs get "${PILL}" whats-new)" true
same "1 getting-set is on" "$(gs get "${TODAY}" getting-set)" true
same "1 battery-details is on" "$(gs get "${POWER}" battery-details)" true
same "1 busy-app is on" "$(gs get "${POWER}" busy-app)" true

# --- 2: Later and Mute after the app's own buttons -----------------------------------------------------------------------
# A banner stays until the person has moved since it came, which in the lab is never: one the session's start showed
# (or queued) would stand in front of every banner below, each check then seeing the one before it. Gone first.
no_banner() { poke; is "Main.messageTray._banner || Main.messageTray._notificationQueue.length ? 1 : 0" 0; }
for _ in 1 2 3 4 5 6 7 8; do
    no_banner && break
    ev "Main.messageTray._expireNotification(); 'x'" >/dev/null
    sleep 1
done
check "2 no banner from before" no_banner
notify -a Notes "Pay the electricity bill" "Due today"
check "2 a banner with none of its own" wait_for 20 banner_is "Pay the electricity bill"
same "2 Later and Mute Notes in its row" "$(later .banner)" '["Later","Mute Notes for an hour"]'
expire
notify -a Notes -A one=One -A two=Two "Two buttons" body
check "2 a banner with two of its own" wait_for 20 banner_is "Two buttons"
same "2 the app's buttons first, as the app gave them" "$(later .own)" '["One","Two"]'
same "2 then Later alone: three in the row" "$(later .banner)" '["Later"]'
ev "${L}.openChoicesForTest(); 'open'" >/dev/null
check "2 Later shows its choices" is "${L}.state().choicesShown" true
check "2 Mute is among them" like "$(later .choices)" '*"In an hour"*"Mute Notes for an hour"*'
expire
notify -a Notes -A one=One -A two=Two -A three=Three "Three buttons" body
check "2 a banner with three of its own" wait_for 20 banner_is "Three buttons"
same "2 nothing of ours in the row" "$(later .banner)" '[]'
check "2 the choices show at once instead" is "${L}.state().choicesShown" true

# --- 3-4: Later in an hour, and back with its buttons --------------------------------------------------------------------
expire
notify -a Notes -A one=One -A two=Two "Pay the gas bill" body
check "3 the banner shows" wait_for 20 banner_is "Pay the gas bill"
check "3 Later in an hour takes it" is "${L}.snoozeForTest('Pay the gas bill', 'hour')" true
check "3 the banner goes" wait_for 10 banner_not "Pay the gas bill"
check "3 the pill says when it comes back" like "$(ev "${P}.state().text")" "Back at *"
same "3 with Undo" "$(ev "${P}._notice?._action?.label ?? ''")" Undo
check "3 GNOME's list still holds it, seen" is "${L}.find('Pay the gas bill')?.acknowledged" true
ev "Main.panel.statusArea.dateMenu.menu.open(); 'open'" >/dev/null
check "3 the calendar shows the Later group" wait_for 10 is "${L}.state().list" true
check "3 it says when" like "$(later .listTexts)" '*"LATER · 1"*"Pay the gas bill"*"Back at *"*"Now"*'
ev "Main.panel.statusArea.dateMenu.menu.close(); 'closed'" >/dev/null
check "4 at its time it comes back" is "${L}.dueForTest('Pay the gas bill')" true
check "4 the banner is back" wait_for 20 banner_is "Pay the gas bill"
same "4 with the same buttons" "$(later .own)" '["One","Two"]'
same "4 nothing waits" "$(later .waiting)" "[]"

# --- 5: mute ---------------------------------------------------------------------------------------------------------
# The banner on screen and one already queued are the muted app's: both are held when Mute is pressed
notify -a Notes "Queued from Notes" body
wait_for 10 in_list "Queued from Notes"
check "5 Mute Notes for an hour" is "${L}.muteForTest('Pay the gas bill')" true
check "5 the muted app's banner on screen goes" wait_for 10 banner_not "Pay the gas bill"
notify -a Notes "Second from Notes" body
check "5 a new banner of the muted app is held" wait_for 20 is "${L}.state().muted[0]?.held >= 2" true
check "5 and is in the list" wait_for 10 in_list "Second from Notes"
check "5 nothing of the muted app shows" banner_not "Second from Notes"
check "5 the queued one is held, not shown" banner_not "Queued from Notes"
notify -a Notes -u critical "Urgent from Notes" body
urgent_shows() {
    poke
    is "Main.messageTray._banner?.notification?.title === 'Urgent from Notes' || Main.messageTray._notificationQueue.some(n => n.title === 'Urgent from Notes')" true
}
check "5 an urgent one is never muted" wait_for 20 urgent_shows
expire
ev "Main.panel.statusArea.dateMenu.menu.open(); 'open'" >/dev/null
muted_row() { like "$(later .listTexts)" '*"MUTED"*"Notes"*"Muted until *· * held"*"Unmute"*'; }
check "5 the Muted row says until when and how many" wait_for 10 muted_row
ev "Main.panel.statusArea.dateMenu.menu.close(); ${L}.unmute(${L}.muted[0][0], true); 'x'" >/dev/null
mute_card() {
    like "$(ev "const c = ${P}.hold.digestCard?.actor; const t = []; const v = a => { if (a.text) t.push(a.text); a.get_children().forEach(v); }; if (c) v(c); JSON.stringify(t)")" \
        '*"Held while Notes was muted"*'
}
check "5 Unmute shows what it held together, under the mute's own title" wait_for 10 mute_card
ev "${P}.hold._closeDigest(); 'x'" >/dev/null

# --- 6: When I next unlock, through a real lock -------------------------------------------------------------------------
notify -a Notes "At the next unlock" body
wait_for 10 in_list "At the next unlock"
ev "${L}.snoozeForTest('At the next unlock', 'unlock'); 'x'" >/dev/null
# The graphical session: systemd 256 and later list the account's user manager as a session of its own too, which
# never locks
session_id="$(loginctl show-user "${TEST_USER}" -p Display --value)"
locked() { [[ "$(loginctl show-session "${session_id}" -p LockedHint --value)" == yes ]]; }
unlocked() { ! locked; }
loginctl lock-sessions
check "6 the screen locks" wait_for 60 locked
loginctl unlock-sessions
check "6 and unlocks" wait_for 60 unlocked
wait_for 60 ready
back() { poke; [[ "$(later .waiting)" == "[]" ]] && { is "${L}.find('At the next unlock')?.acknowledged" false || banner_is "At the next unlock"; }; }
check "6 it came back at the unlock" wait_for 30 back
expire

# --- 7: off forgets; nothing was ever written ---------------------------------------------------------------------------
notify -a Notes "Forgotten" body
wait_for 10 in_list "Forgotten"
ev "${L}.snoozeForTest('Forgotten', 'morning'); ${L}.muteForTest('Forgotten'); 'x'" >/dev/null
gs set "${PILL}" later false
check "7 off: no Later" wait_for 10 is "${P}.hold?.later ? 'on' : 'off'" off
notify -a Chat -A open=Open "After off" body
check "7 a banner after off" wait_for 20 banner_is "After off"
same "7 has its own button and none of ours" \
    "$(ev "JSON.stringify(Main.messageTray._banner._buttonBox?.get_children().map(b => b.label) ?? [])")" '["Open"]'
expire
gs set "${PILL}" later true
check "7 on again" wait_for 10 ready
same "7 nothing waits" "$(later .waiting)" "[]"
same "7 nothing is muted" "$(later .muted)" "[]"
if grep -rlsF -e "Pay the gas bill" -e "Second from Notes" "${HOME_DIR}/.local/state" "${HOME_DIR}/.config/amethystora" \
    "${HOME_DIR}/.cache" 2>/dev/null; then
    not_ok "7 a notification's words were written"
else
    ok "7 no notification's words on disk"
fi

# --- 8: What's new --------------------------------------------------------------------------------------------------
check "8 the image has its file" jq -e '(.version | type == "string") and (.releases | type == "array")' \
    /usr/share/amethystora/whats-new.json
check "8 and the Manual its page" grep -qx "# What's new" /usr/share/amethystora/manual/whats-new.md
VERSION="$(. /usr/lib/os-release && echo "${IMAGE_VERSION}")"
same "8 the account saw this version (recorded at login)" "$(jq -r .seen "${STATE}/whats-new.json" 2>/dev/null)" "${VERSION}"
INFO='{version: "99.20991231", releases: [
    {version: "99.20991231", items: [{title: "The dock grows as you pass", area: "desktop"}, {title: "A fourth", area: "apps"}]},
    {version: "99.20991230", items: [{title: "Four security fixes", area: "security"}, {title: "Snooze a notification", area: "desktop"}]},
    {version: "11.20000101", items: [{title: "Seen long ago", area: "desktop"}]}]}'
# Step 7's "Notes muted until …" says itself for five seconds, in a place before What's new's
said_out() { is "${P}.state().activity === 'waiting'" false; }
wait_for 10 said_out
check "8 after updates the pill says Updated" is "${P}.newsForTest(${INFO}, '50.20500101')" true
same "8 its words" "$(ev "${P}.state().text")" "Updated · what’s new"
same "8 what the releases since the version seen brought" "$(ev "JSON.stringify(${P}.state().news.items.map(i => i.title))")" \
    '["The dock grows as you pass","A fourth","Four security fixes","Snooze a notification"]'
ev "${P}.clock.openCard('news'); 'x'" >/dev/null
rows() { is "const c = ${P}.clock._card?.box; let n = 0; const v = a => { if (a.has_style_class_name?.('amethystora-pill-news-row')) n++; a.get_children().forEach(v); }; if (c) v(c); n" 3; }
check "8 the card lists three of them" wait_for 10 rows
ev "${P}.clock._closeCard(); 'x'" >/dev/null
same "8 the version is recorded" "$(jq -r .seen "${STATE}/whats-new.json")" "99.20991231"
check "8 a copy beside it" test -s "${STATE}/whats-new.json.bak"
manual_open() { is "global.get_window_actors().some(a => a.meta_window.get_wm_class() === 'org.amethystora.Manual')" true; }
ev "${P}.news.readAll(); 'x'" >/dev/null
check "8 Read it all opens the Manual" wait_for 30 manual_open
ev "global.get_window_actors().filter(a => a.meta_window.get_wm_class() === 'org.amethystora.Manual').forEach(a => a.meta_window.delete(global.get_current_time())); 'x'" >/dev/null
check "8 said once: not again" is "${P}.newsForTest(${INFO}, '99.20991231')" false
check "8 shown again for the minute" is "${P}.newsForTest(${INFO}, '50.20500101')" true
check "8 after its minute the pill is the clock" wait_for 90 is "${P}.state().activity === 'news'" false
check "8 nothing after a rollback" is "${P}.newsForTest({version: '50.20500101', releases: []}, '99.20991231')" false
same "8 and the newer version stays the one seen" "$(jq -r .seen "${STATE}/whats-new.json")" "99.20991231"
rm -f "${STATE}/whats-new.json" "${STATE}/whats-new.json.bak"
check "8 nothing for an account that never saw a version" is "${P}.newsForTest(${INFO}, undefined)" false
in_session "printf '{\"version\":1,\"seen\":\"%s\"}\n' '${VERSION}' > ~/.local/state/amethystora/whats-new.json"

# --- 9: Getting set ---------------------------------------------------------------------------------------------------
G="${UI}.get('today').getting"
ev "${G}.forceYoungForTest(true); Main.panel.statusArea.dateMenu.menu.open(); 'x'" >/dev/null
answered() { is "${G}.state().lines.filter(l => l.state === 'wait').length" 0; }
check "9 every line answers within the helpers' minute" wait_for 90 answered
check "9 Getting set shows" is "${G}.state().shown" true
check "9 it counts what is done" like "$(ev "${G}.state().progress")" "* of * done"
same "9 automatic updates ticked as systemd says (uupd.timer)" \
    "$(ev "${G}.state().lines.find(l => l.id === 'updates').state")" \
    "$([[ "$(systemctl is-enabled uupd.timer 2>/dev/null)" == enabled ]] && echo done || echo todo)"
same "9 the backup line as Backups says" "$(ev "${G}.state().lines.find(l => l.id === 'backup').state")" \
    "$(in_session "/usr/libexec/amethystora-restore status --json" 2>/dev/null |
        jq -r 'if .backup.set_up and (.backup.last_success // 0) > 0 then "done" else "todo" end')"
ev "Main.panel.statusArea.dateMenu.menu.close(); 'x'" >/dev/null
gs set "${TODAY}" getting-set false
ev "Main.panel.statusArea.dateMenu.menu.open(); 'x'" >/dev/null
check "9 Hide is for good" wait_for 10 is "${G}.state().shown" false
ev "Main.panel.statusArea.dateMenu.menu.close(); ${G}.forceYoungForTest(false); 'x'" >/dev/null
gs reset "${TODAY}" getting-set

# --- 10: the battery, explained (a stand-in UPower) -----------------------------------------------------------------------
cp "${HERE}/upower-standin.py" "${WORK}/"
if systemctl is-active --quiet upower.service; then
    systemctl stop upower.service
    systemctl mask --runtime upower.service >/dev/null 2>&1
    touch "${WORK}/upower-masked"
fi
systemd-run --collect --unit=vmlab-upower-notify python3 "${WORK}/upower-standin.py" >/dev/null
upower_up() { busctl --system status org.freedesktop.UPower >/dev/null 2>&1; }
wait_for 20 upower_up
gs set "${POWER}" battery-details false
gs set "${POWER}" battery-details true
BAT="${UI}.get('power').battery"
check "10 a battery is seen" wait_for 20 is "${BAT}?.state().present" true
pkill -USR1 -f upower-standin.py
check "10 plugging in is seen" wait_for 20 is "${BAT}.state().kept?.charging" true
pkill -USR2 -f upower-standin.py
check "10 unplugging looks at the processes once" wait_for 30 \
    is "${BAT}.state().kept?.charging === false && ${BAT}.state().kept?.measured" true
ev "Main.panel.statusArea.quickSettings.menu.open(); 'x'" >/dev/null
wait_for 10 is "Main.panel.statusArea.quickSettings.menu.isOpen" true
ev "${UI}.get('quick').batteryMenu().open(); 'x'" >/dev/null
# The heading shows at once over "Measuring…"; the apps and the note once the second look at the processes is done
measured_texts() { like "$(ev "JSON.stringify(${BAT}.state().texts)")" '*USED THE MOST SINCE*Worked out from processor time*'; }
check "10 it says what used the most" wait_for 30 measured_texts
texts="$(ev "JSON.stringify(${BAT}.state().texts)")"
check "10 the menu says the charge" like "${texts}" '*"86%"*'
check "10 and says it is an estimate" like "${texts}" '*Worked out from processor time*'
check "10 the watts and when it was unplugged" like "${texts}" '*Using 9.8 W now · unplugged at *'
check "10 the battery's health" like "${texts}" '*88% of new · 412 cycles*'
ev "Main.panel.statusArea.quickSettings.menu.close(); 'x'" >/dev/null

# --- 11: a busy app ---------------------------------------------------------------------------------------------------
# A busy loop for each processor, as an app GNOME would start (its scope's name), for two minutes and a half
cores="$(nproc)"
user_bus systemd-run --user --scope --unit=app-gnome-org.gnome.TextEditor-4242.scope -- \
    bash -c "for i in \$(seq ${cores}); do timeout 170 sh -c 'while :; do :; done' & done; wait" >/dev/null 2>&1 &
chip_named() { like "$(ev "${B}.state().chip ?? ''")" "* is busy"; }
check "11 after two minutes the chip names the app" wait_for 200 chip_named
ev "${B}._chip.menu.open(); 'x'" >/dev/null
check "11 its card opens" wait_for 10 is "${B}.state().open" true
check "11 with Show it, Not now and Quit" like "$(ev "JSON.stringify(${B}.state().buttons)")" '\["Show it","Not now","Quit *"\]'
ev "${B}._chip.menu.close(); ${B}.dismiss(); 'x'" >/dev/null
same "11 Not now takes it away" "$(ev "${B}.state().chip")" null
wait
APP_ID=org.mozilla.firefox.desktop
check "11 a test app is named" is "${B}.nameForTest('${APP_ID}')" true
ev "${B}.dismiss(); 'x'" >/dev/null
same "11 not said again within the hour" "$(ev "${B}.nameForTest('${APP_ID}', true)")" false
# The processor calms down while the card is open: the chip goes once the card closes
ev "${B}.nameForTest('${APP_ID}'); ${B}._chip.menu.open(); 'x'" >/dev/null
wait_for 10 is "${B}.state().open" true
ev "${B}.feedForTest(0.1, 1); 'x'" >/dev/null
check "11 the open card stays" like "$(ev "${B}.state().chip ?? ''")" "* is busy"
ev "${B}._chip.menu.close(); 'x'" >/dev/null
check "11 and the chip goes when it closes" wait_for 10 is "${B}.state().chip" null
ev "${B}.feedForTest(null, 0); 'x'" >/dev/null

# --- 12: ten disables and enables -------------------------------------------------------------------------------------
# GNOME 51 disables and enables an extension asynchronously: each step waits for the state GNOME gives it. What is
# left is counted with the extension off, before the cycles and after them (on, what a module builds only once it is
# first needed would differ), by kind (an actor's style class, else its type), so that what grew is named
census='(() => { const m = {}; const v = a => { const k = a.style_class || a.constructor.name; m[k] = (m[k] ?? 0) + 1; a.get_children().forEach(v); }; v(Main.uiGroup); m.statusArea = Object.keys(Main.panel.statusArea).length; return m; })()'
off() { ev "Main.extensionManager.disableExtension('${UUID}'); 'x'" >/dev/null; wait_for 20 state_is 2 && sleep 1; }
on() { ev "Main.extensionManager.enableExtension('${UUID}'); 'x'" >/dev/null; wait_for 20 state_is 1; }
off
ev "globalThis.vmlabCensus = ${census}; 'x'" >/dev/null
cycles=0
for _ in 1 2 3 4 5 6 7 8 9 10; do
    on || break
    off || break
    cycles=$((cycles + 1))
done
same "12 ten cycles" "${cycles}" 10
same "12 leave nothing" "$(ev "(() => { const a = ${census}, b = globalThis.vmlabCensus;
    return JSON.stringify(Object.keys({...a, ...b}).filter(k => (a[k] ?? 0) !== (b[k] ?? 0)).map(k => k + ' ' + ((a[k] ?? 0) - (b[k] ?? 0))).slice(0, 12)); })()")" "[]"
same "12 off, banners are GNOME's own" "$(ev "Main.messageTray.constructor.prototype._showNotification.toString().includes('hold')")" false
on
wait_for 60 ready

# --- 13: Control and the setup file -------------------------------------------------------------------------------------
control="$(in_session "/usr/libexec/amethystora-control-list --json" 2>/dev/null)"
same "13 Control's entries" \
    "$(jq -r '[.items[] | select(.id | IN("later", "later-times", "whats-new", "getting-set", "battery-details", "busy-app")) | .id] | sort | join(",")' <<<"${control}")" \
    "battery-details,busy-app,getting-set,later,later-times,whats-new"
in_session "ame desktop power busy off" >/dev/null
in_session "ame desktop notifications later-times 19:00 08:00" >/dev/null
in_session "ame setup save" >/dev/null 2>&1
same "13 the setup file carries them" \
    "$(jq -c '[.settings.power, .settings.pill["later-times"]]' "${HOME_DIR}/.config/amethystora/setup.json")" \
    '[{"busy-app":false},["19:00","08:00"]]'
gs reset "${POWER}" busy-app
gs reset "${PILL}" later-times

# --- and nothing threw ---------------------------------------------------------------------------------------------------
errors="$(journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --since "${since}" --no-pager -o cat 2>/dev/null |
    grep -E "JS ERROR|Amethystora UI: (Later|the power|what's new|the battery|the busy)" | head -n 20)"
if [[ -z ${errors} ]]; then ok "nothing threw"; else not_ok "something threw" "${errors}"; fi
finish
