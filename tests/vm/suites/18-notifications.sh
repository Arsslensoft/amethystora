#!/usr/bin/bash
# vmlab-vm: notifications
# Quiet, the digest, the count on a stack of notifications and the volume pill (plans/beyond/spec-pill.md, section
# 7.2), in the test account's session, asked of GNOME Shell itself through Eval, which a lab-only extension of the test
# account opens (never in the image):
#   - quiet never writes GNOME's show-banners;
#   - a fullscreen window holds banners, lets an urgent one through, and its end shows none in a burst: the pill says
#     how many wait in the list;
#   - sharing holds them too; quiet off is GNOME's own behaviour; disabling the extension mid-quiet loses nothing;
#   - quiet at night between Night Light's times;
#   - the digest at the next minute, with a row for each app, Show all and Clear;
#   - "· 3" on a folded stack; the volume pill's size and place, and GNOME's OSD back after a disable.
# vmlab-paths: @ui/extension.js @ui/*pill* @sf/*themed/shell-pill* @sf/*just/desktop.just
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HOME_DIR="/var/home/${TEST_USER}"
PILL=org.gnome.shell.extensions.amethystora-ui.pill
COLOR=org.gnome.settings-daemon.plugins.color
HERE="$(dirname "$(readlink -f "$0")")"
WORK=/run/vmlab-18
# Eval's answer, decoded once: Eval gives JSON of the value, and gdbus prints that as a GVariant string. A string comes
# back as it is, anything else as compact JSON; a throw prints ERROR and fails
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        python3 -c '
import ast, json, sys
text = sys.stdin.read().strip()
if not text.startswith("("):
    print("ERROR")
    sys.exit(1)
ok, value = ast.literal_eval("(" + ("True" if text.startswith("(true") else "False") + text[text.index(","):])
if not ok:
    print("ERROR: " + value, file=sys.stderr)
    print("ERROR")
    sys.exit(1)
try:
    value = json.loads(value) if value else ""
except ValueError:
    pass
print(value if isinstance(value, str) else json.dumps(value, ensure_ascii=False, separators=(",", ":")))'
}
gs() { in_session "gsettings $*" 2>/dev/null; }
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
P="${UI}.get('pill')"
# One field of the pill's state, as JSON ('"idle"', true, 5)
st() { ev "JSON.stringify(${P}.state()$1)"; }
keys() { python3 "${HERE}/keys.py" "$@" >/dev/null 2>&1; }
shell_click() { user_bus python3 "${HERE}/atspi.py" gnome-shell click "$@" >/dev/null 2>&1; }
notify() { in_session "notify-send $*"; }
banners() { gs get org.gnome.desktop.notifications show-banners; }
no_banner() { [[ "$(ev "Main.messageTray._notificationState")" == 0 ]]; }
banner_up() { [[ "$(ev "Main.messageTray._notificationState")" != 0 ]]; }
banner_title() { ev "Main.messageTray._notification?.title ?? ''"; }
# The titles of every notification in the list
listed() { ev "Main.messageTray.getSources().flatMap(s => s.notifications.map(n => n.title)).join('|')"; }
clear_list() { ev "Main.messageTray.getSources().forEach(s => s.destroy()); 'cleared'" >/dev/null; }
# No banner shows for SECONDS
stays_quiet() {
    local i
    for i in $(seq "$1"); do
        banner_up && return 1
        sleep 1
    done
}
fullscreen() {
    user_bus systemd-run --user --collect --unit=vmlab-full kitty --start-as=fullscreen >/dev/null
}
is_fullscreen() { [[ "$(ev "Main.layoutManager.primaryMonitor.inFullscreen")" == true ]]; }
unfullscreen() { user_bus systemctl --user stop vmlab-full >/dev/null 2>&1; }
since="$(date '+%Y-%m-%d %H:%M:%S')"

if ! has_session; then
    bash "$(dirname "$0")/10-account.sh" >/dev/null
fi
if ! wait_for 300 has_session; then
    not_ok "no session for ${TEST_USER}"
    finish
fi
for key in live privacy-dots glow quiet quiet-night digest digest-times; do gs reset "${PILL}" "${key}"; done

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
loginctl unlock-sessions
systemctl restart gdm
pill_ready() { [[ "$(ev "typeof ${P}?.state")" == function ]]; }
if ! wait_for 240 has_session || ! wait_for 120 pill_ready; then
    not_ok "the pill module did not start in the session"
    finish
fi
sleep 8
ev "Main.overview.hide(); 'hidden'" >/dev/null
# The account's own units write their logs here
install -d -m 0755 -o "${TEST_USER}" -g "${TEST_USER}" "${WORK}"
cp "${HERE}/sharecast.py" "${WORK}/"
clear_list

# --- 1-2: fullscreen holds, and lets go without a burst ------------------------------------------------------------------
same "1 show-banners is true before" "$(banners)" true
fullscreen
check "2 kitty is fullscreen on the main monitor" wait_for 15 is_fullscreen
quiet_full() { [[ "$(st .quiet.reason)" == '"fullscreen"' ]]; }
check "2 quiet, for the fullscreen window" wait_for 3 quiet_full
notify '"vmlab one"'
notify '"vmlab two"'
check "2 no banner for 5 s" stays_quiet 5
same "2 two held" "$(st .quiet.held)" 2
same "1 show-banners is true while quiet" "$(banners)" true
notify -u critical '"vmlab urgent"'
urgent() { [[ "$(banner_title)" == "vmlab urgent" ]]; }
check "2 an urgent one comes through within 3 s" wait_for 3 urgent
ev "Main.messageTray._notification?.destroy(); 'gone'" >/dev/null
unfullscreen
waiting() { [[ "$(st .activity)" == '"waiting"' && "$(st .text)" == '"2 waiting in the list"' ]]; }
if wait_for 3 waiting; then ok "2 the pill says 2 waiting in the list"; else
    not_ok "2 the pill says 2 waiting in the list" "it says $(st .activity) $(st .text)"; fi
check "2 no banner shows for them for 6 s" stays_quiet 6
[[ "$(listed)" == *"vmlab one"* && "$(listed)" == *"vmlab two"* ]] && ok "2 both are in the list" || not_ok "2 both in the list" "$(listed)"
clear_list

# --- 3: sharing holds ---------------------------------------------------------------------------------------------------
user_bus systemd-run --user --collect --unit=vmlab-share --property=StandardOutput=file:"${WORK}/share.log" \
    python3 "${WORK}/sharecast.py" 15 >/dev/null
sleep 3
if grep -q started "${WORK}/share.log" 2>/dev/null; then
    notify '"vmlab three"'
    check "3 no banner while sharing" stays_quiet 3
    same "3 one held" "$(st .quiet.held)" 1
    same "3 the pill says so" "$(st .text)" '"Quiet while sharing · 1 held"'
    user_bus systemctl --user stop vmlab-share >/dev/null 2>&1
    one_waiting() { [[ "$(st .text)" == '"1 waiting in the list"' ]]; }
    check "3 its end: 1 waiting in the list" wait_for 8 one_waiting
else
    skip "3 Mutter refused the screen cast: $(cat "${WORK}/share.log" 2>/dev/null)"
fi
same "1 show-banners is true after sharing" "$(banners)" true
clear_list

# --- 4-5: quiet off is GNOME's own; a disable mid-quiet loses nothing ---------------------------------------------------
check "4 ame desktop quiet off" in_session "ame desktop quiet off"
fullscreen
wait_for 15 is_fullscreen
notify '"vmlab four"'
sleep 2
same "4 quiet is off" "$(st .quiet.on)" false
unfullscreen
four() { [[ "$(banner_title)" == "vmlab four" ]]; }
check "4 GNOME shows its banner after the fullscreen window, as it always has" wait_for 5 four
ev "Main.messageTray._notification?.destroy(); 'gone'" >/dev/null
in_session "ame desktop quiet on" >/dev/null
clear_list

fullscreen
wait_for 15 is_fullscreen
wait_for 3 quiet_full
notify '"vmlab five"'
sleep 1
ev "Main.extensionManager.disableExtension('${UUID}'); 'off'" >/dev/null
same "5 show-banners is still true with the extension off" "$(banners)" true
unfullscreen
any_banner() { banner_up; }
# What was held stays in the list, unseen; a notification that comes now is GNOME's alone
notify '"vmlab six"'
check "5 GNOME shows banners again at once" wait_for 5 any_banner
[[ "$(listed)" == *"vmlab five"* ]] && ok "5 the held one is in the list" || not_ok "5 the held one in the list" "$(listed)"
ev "Main.extensionManager.enableExtension('${UUID}'); 'on'" >/dev/null
wait_for 30 pill_ready
clear_list

# --- 6: quiet at night -------------------------------------------------------------------------------------------------
hour="$(date +%-H)"
gs set "${COLOR}" night-light-schedule-automatic false
gs set "${COLOR}" night-light-schedule-from "$(((hour + 23) % 24)).0"
gs set "${COLOR}" night-light-schedule-to "$(((hour + 1) % 24)).0"
check "6 ame desktop quiet night on" in_session "ame desktop quiet night on"
night() { [[ "$(st .quiet.reason)" == '"night"' ]]; }
check "6 quiet, for the night, within 70 s" wait_for 70 night
notify '"vmlab night"'
check "6 no banner at night" stays_quiet 4
minute_ago="$(awk -v h="$(date +%-H)" -v m="$(date +%-M)" 'BEGIN { t = h + (m - 1) / 60; if (t < 0) t += 24; printf "%.4f", t }')"
gs set "${COLOR}" night-light-schedule-to "${minute_ago}"
not_night() { [[ "$(st .quiet.reason)" != '"night"' ]]; }
check "6 quiet ends when the night does, within 70 s" wait_for 70 not_night
in_session "ame desktop quiet night off" >/dev/null
for key in night-light-schedule-automatic night-light-schedule-from night-light-schedule-to; do gs reset "${COLOR}" "${key}"; done
clear_list

# --- 7: the digest -----------------------------------------------------------------------------------------------------
check "7 ame desktop digest on" in_session "ame desktop digest on"
# Far enough from the minute's end for three notifications to arrive before it
(($(date +%-S) > 45)) && sleep 20
next="$(date -d '+1 minute' +%H:%M)"
check "7 ame desktop digest times ${next}" in_session "ame desktop digest times ${next}"
notify -a vmlab-a '"First from a"'
notify -a vmlab-a '"Second from a"'
notify -a vmlab-b '"Only from b"'
check "7 no banners" stays_quiet 2
same "7 three held for the digest" "$(st .digest.held)" 3
notify -u critical '"vmlab urgent two"'
urgent2() { [[ "$(banner_title)" == "vmlab urgent two" ]]; }
check "7 an urgent one comes through" wait_for 3 urgent2
ev "Main.messageTray._notification?.destroy(); 'gone'" >/dev/null
digest_shown() { [[ "$(st .digest.shown)" == true ]]; }
check "7 the digest card at the minute" wait_for 75 digest_shown
card="$(ev "(() => { const c = ${P}.hold.digestCard.actor; const out = []; const walk = a => { if (a.text) out.push(a.text); a.get_children().forEach(walk); }; walk(c); return out.join('|'); })()")"
[[ "${card}" == *"${next}"*"3 notifications"* ]] && ok "7 its heading has the time and 3 notifications" || not_ok "7 the heading" "${card}"
[[ "${card}" == *"vmlab-a| · 2 notifications"* ]] && ok "7 a row for vmlab-a with 2 notifications" || not_ok "7 vmlab-a's row" "${card}"
[[ "${card}" == *"vmlab-b| · Only from b"* ]] && ok "7 a row for vmlab-b with its title" || not_ok "7 vmlab-b's row" "${card}"
# A pointer click, as a person makes one (the card has a click of its own, which must not take the button's), and the
# button is named for screen readers too
digest_button() {
    ev "(() => { const out = []; const walk = a => { if (a instanceof imports.gi.St.Button && a.label === '$1') out.push(a); a.get_children().forEach(walk); };
        walk(${P}.hold.digestCard.actor); const e = out[0].get_transformed_extents();
        return ((e.get_x() + e.get_width() / 2) / global.stage.width).toFixed(4) + ',' + ((e.get_y() + e.get_height() / 2) / global.stage.height).toFixed(4); })()"
}
check "7 the card names Show all for screen readers" user_bus python3 "${HERE}/atspi.py" gnome-shell has "Show all"
keys "click:$(digest_button "Show all")"
calendar_open() { [[ "$(ev "Main.panel.statusArea.dateMenu.menu.isOpen")" == true && "$(st .digest.shown)" == false ]]; }
check "7 Show all opens the calendar, and the card goes" wait_for 3 calendar_open
keys esc
(($(date +%-S) > 45)) && sleep 20
next="$(date -d '+1 minute' +%H:%M)"
in_session "ame desktop digest times ${next}" >/dev/null
notify -a vmlab-c '"To be cleared"'
check "7 the next digest" wait_for 75 digest_shown
keys "click:$(digest_button Clear)"
cleared() { [[ "$(listed)" != *"To be cleared"* ]]; }
check "7 Clear takes them out of the list" wait_for 3 cleared
before="$(gs get "${PILL}" digest-times)"
in_session "ame desktop digest times 25:00" >/dev/null 2>&1
code=$?
same "7 digest times 25:00 is refused" "${code}" 64
same "7 ...and changes nothing" "$(gs get "${PILL}" digest-times)" "${before}"
in_session "ame desktop digest off" >/dev/null
gs reset "${PILL}" digest-times
clear_list

# --- 8: the count on a stack ------------------------------------------------------------------------------------------
# One program sends the three, so that GNOME keeps them in one stack (notify-send would be three programs)
in_session "python3 -c 'from gi.repository import Gio, GLib
c = Gio.bus_get_sync(Gio.BusType.SESSION)
for t in (\"n1\", \"n2\", \"n3\"):
    c.call_sync(\"org.freedesktop.Notifications\", \"/org/freedesktop/Notifications\", \"org.freedesktop.Notifications\", \"Notify\",
        GLib.Variant(\"(susssasa{sv}i)\", (\"vmlab-stack\", 0, \"\", t, \"\", [], {}, -1)), None, 0, -1)
'" >/dev/null 2>&1
# The banner on screen is left alone: destroying it would take a notification out of the stack
sleep 2
ev "Main.panel.statusArea.dateMenu.menu.open(); 'open'" >/dev/null
sleep 1.5
COUNT="(() => { const v = Main.panel.statusArea.dateMenu._messageList._messageView; const found = []; const walk = a => { for (const c of a.get_children()) { if (c.name === 'amethystora-pill-stack-count' && c.visible) found.push(c.get_parent().get_children().map(x => x.text ?? '').join('')); walk(c); } }; walk(v); return found.join('|'); })()"
count="$(ev "${COUNT}")"
[[ "${count}" == *"vmlab-stack· 3"* ]] && ok "8 the folded stack says · 3 after vmlab-stack" || not_ok "8 the stack's count" "${count}"
ev "(() => { const v = Main.panel.statusArea.dateMenu._messageList._messageView; let g = null; const walk = a => { for (const c of a.get_children()) { if (!g && c.has_style_class_name?.('message-notification-group')) g = c; walk(c); } }; walk(v); g?.expand(); return 'expanded'; })()" >/dev/null
sleep 1
same "8 expanded, the count goes" "$(ev "${COUNT}")" ""
ev "Main.panel.statusArea.dateMenu.menu.close(); 'closed'" >/dev/null
clear_list

# --- 9-10: the volume pill ---------------------------------------------------------------------------------------------
keys volumeup
osd_shown() { [[ "$(st .osd.visible)" == true ]]; }
if ! wait_for 2 osd_shown; then
    in_session "gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.ShowOSD \"{'icon': <'audio-volume-medium-symbolic'>, 'level': <0.62>}\"" >/dev/null
fi
sleep 0.3
geometry="$(ev "(() => { const o = ${P}.state().osd; const m = Main.layoutManager.primaryMonitor; const wa = Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex);
    const near = (a, b) => Math.abs(a - b) <= 1; return [o.class, near(o.height, 36), near(o.width, 180), near(o.y, wa.y + 8), near(o.x + o.width / 2, m.x + m.width / 2)].join(' '); })()")"
same "9 the volume pill: its class, 36 high, 180 wide, 8 under the work area's top, centred" "${geometry}" "true true true true true"
hidden() { [[ "$(st .osd.visible)" == false ]]; }
sleep 2
check "9 it hides after GNOME's time" hidden
in_session "pactl set-sink-volume @DEFAULT_SINK@ +5%" >/dev/null 2>&1
sleep 0.5
check "9 pactl shows no OSD (GNOME's own behaviour)" hidden
in_session "gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.ShowOSD \"{'icon': <'input-keyboard-symbolic'>, 'label': <'Caps Lock on'>}\"" >/dev/null
sleep 0.3
label="$(ev "(() => { const o = ${P}.state().osd; return (Math.abs(o.height - 36) <= 1) + ' ' + (o.width >= 179 && o.width <= 361); })()")"
same "9 a label OSD: 36 high, 180 to 360 wide" "${label}" "true true"
sleep 2

ev "Main.extensionManager.disableExtension('${UUID}'); 'off'" >/dev/null
in_session "gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.ShowOSD \"{'icon': <'audio-volume-medium-symbolic'>, 'level': <0.5>}\"" >/dev/null
sleep 0.3
same "10 disabled: GNOME's OSD as it was" \
    "$(ev "(() => { const o = Main.osdWindowManager._osdWindows[0]; return o.y_align + ' ' + o._hbox.has_style_class_name('amethystora-pill-osd') + ' ' + o.translation_y; })()")" \
    "3 false 0"
ev "Main.extensionManager.enableExtension('${UUID}'); 'on'" >/dev/null
wait_for 30 pill_ready
same "1 show-banners is true at the end" "$(banners)" true

errors="$(journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --since "${since}" --no-pager -o cat 2>/dev/null |
    grep -E "JS ERROR|Amethystora UI: the pill module" | head -n 20)"
if [[ -z ${errors} ]]; then ok "nothing of the pill threw"; else not_ok "the pill threw" "${errors}"; fi
for key in live privacy-dots glow quiet quiet-night digest digest-times; do gs reset "${PILL}" "${key}"; done
finish
