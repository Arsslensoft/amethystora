#!/usr/bin/bash
# Today in the calendar menu, the quick note, the rest reminders, the weather and the bar over fullscreen on one screen
# (plans/beyond/spec-quick.md 1.3 to 1.7 and 2; criteria 13 to 23 of its section 7, the bars' and Today's part), asked of
# the session through GNOME Shell's Eval, which a lab-only extension of the test account opens (never in the image):
#   - Notes' tasks due today and overdue, in the order Notes lists them, while Notes runs open; a title is text;
#   - Notes' Today object only while the notes are open, and only for GNOME Shell;
#   - Super+Alt+N: the quick note floats, saves a task to the inbox, asks for the passphrase when the notes are
#     locked, locks them again after, and leaves an open Notes as it was;
#   - ame desktop breaks writes GNOME's own key; the weather is off, listed among what the machine sends;
#   - pushing against the top of a fullscreen window brings the bar down; ten cycles leave nothing behind.
# The VM draws in software, where GNOME turns its animations off: the peek is checked by its state, not its slide.
# vmlab-paths: @ui/extension.js @ui/*today* @ui/*bars* @sf/*themed/shell-today* apps/notes/* @sf/*just/desktop.just @sf/usr/libexec/amethystora-your-year
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HOME_DIR="/var/home/${TEST_USER}"
NOTES_DIR="${HOME_DIR}/.local/share/amethystora-notes"
# The encrypted fixture's passphrase (apps/notes/tests/compat.rs)
FIXTURE_PASSPHRASE="correct horse battery staple"
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        sed -E -e "s/^\(true, '(.*)'\)\$/\1/" -e 's/^"(.*)"$/\1/'
}
ev_is() { [[ "$(ev "$1")" == "$2" ]]; }
keys() { python3 "$(dirname "$0")/keys.py" "$@"; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
T="${UI}.get('today')"
B="${UI}.get('bars')"
since="$(date '+%Y-%m-%d %H:%M:%S')"
today="$(date +%F)"
yesterday="$(date -d yesterday +%F)"
tomorrow="$(date -d tomorrow +%F)"

if ! has_session; then
    bash "$(dirname "$0")/10-account.sh" >/dev/null
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
if [[ ${enabled} != *vmlab-eval@lab* ]]; then
    user_bus gsettings set org.gnome.shell enabled-extensions "${enabled%]*}, 'vmlab-eval@lab']"
    loginctl unlock-sessions
    sleep 2
    systemctl restart gdm
    wait_for 240 has_session
    sleep 15
fi
ev "Main.overview.hide(); 1" >/dev/null

check "the bars and today modules run" ev_is "['bars', 'today'].map(n => ${UI}.has(n)).join()" "true,true"

# ---- Notes and its vaults: a plain one with four tasks, and the encrypted fixture of apps/notes/tests
notes_pids() { pgrep -u "${TEST_USER}" -f '[/]usr/lib/amethystora-notes/amethystora-notes'; }
notes_gone() { [[ -z "$(notes_pids)" ]]; }
notes_stop() {
    pkill -u "${TEST_USER}" -f '[/]usr/lib/amethystora-notes/amethystora-notes' 2>/dev/null
    wait_for 20 notes_gone
}
notes_start() { user_bus systemd-run --user --collect --unit="vmlab-notes-${RANDOM}" /usr/bin/amethystora-notes "$@" >/dev/null; }
notes_running() { user_bus busctl --user status org.amethystora.Notes >/dev/null 2>&1; }
today_object() {
    user_bus gdbus introspect --session --dest org.amethystora.Notes --object-path /org/amethystora/Notes/Today 2>/dev/null
}
has_today() { today_object | grep -q 'GetToday'; }
no_today() { ! has_today; }
id32() { head -c16 /dev/urandom | od -An -tx1 | tr -d ' \n'; }
task() {
    printf '"%s":{"id":"%s","title":"%s","notes":"","project":null,"section":null,"parent":null,"due":"%s","time":null,"repeat":null,"reminder":null,"priority":4,"labels":[],"pinned":false,"done":%s,"created":%s,"order":%s}' \
        "$1" "$1" "$2" "$3" "$4" "$5" "$5"
}
plain_vault() {
    rm -rf "${NOTES_DIR}"
    install -d -m 0700 -o "${TEST_USER}" -g "${TEST_USER}" "${NOTES_DIR}" "${NOTES_DIR}/resources"
    echo '{"format":1,"encrypted":false}' >"${NOTES_DIR}/vault.json"
    local a b c d n tasks
    a=$(id32) b=$(id32) c=$(id32) d=$(id32) n=$(id32)
    tasks="$(task "$a" "Water the plants" "${today}" null 2),$(task "$b" "Call the <b>bakery</b>" "${yesterday}" null 1)"
    tasks+=",$(task "$c" "Later thing" "${tomorrow}" null 3),$(task "$d" "Done thing" "${today}" 1761600000000 4)"
    printf '{"version":1,"settings":{},"resources":{},"notebooks":{"%s":{"id":"%s","title":"Notes","parent":null,"created":1,"updated":1}},"notes":{},"tags":{},"revisions":{},"projects":{},"sections":{},"tasks":{%s},"labels":{}}\n' \
        "$n" "$n" "${tasks}" >"${NOTES_DIR}/data.json"
    chown -R "${TEST_USER}:" "${NOTES_DIR}"
}
encrypted_vault() {
    rm -rf "${NOTES_DIR}"
    install -d -m 0700 -o "${TEST_USER}" -g "${TEST_USER}" "${NOTES_DIR}" "${NOTES_DIR}/resources"
    # apps/notes/tests/fixtures/encrypted, vault.json and data.bin, as a gzip'd tar
    base64 -d <<'FIXTURE' | tar -xz -C "${NOTES_DIR}"
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
FIXTURE
    chown -R "${TEST_USER}:" "${NOTES_DIR}"
}
# GetToday as the shell asks it
shell_today() {
    ev "(() => { const G = imports.gi; try { return JSON.stringify(G.Gio.DBus.session.call_sync('org.amethystora.Notes', '/org/amethystora/Notes/Today', 'org.amethystora.Notes.Today1', 'GetToday', null, new G.GLib.VariantType('(suas)'), G.Gio.DBusCallFlags.NO_AUTO_START, 2000, null).deepUnpack()); } catch (e) { return 'error'; } })()" |
        sed -E 's/[\\]+"/"/g'
}
notes_windows() {
    ev "global.display.list_all_windows().filter(w => w.get_wm_class() === 'org.amethystora.Notes' && w.showing_on_its_workspace()).map(w => w.get_title()).join('|')"
}
QN="global.display.list_all_windows().find(w => w.get_wm_class() === 'org.amethystora.Notes' && !w.get_gtk_window_object_path())"
capture_up() { [[ "$(ev "Boolean(${QN}?.has_focus())")" == true ]]; }
capture_gone() { [[ "$(ev "Boolean(${QN})")" == false ]]; }
saved() { jq -r --arg t "$1" '.tasks[] | select(.title == $t) | [.due, (.project // "inbox")] | join(" ")' "${NOTES_DIR}/data.json" 2>/dev/null; }
backup="${NOTES_DIR}.vmlab-$$"
[[ -e "${NOTES_DIR}" ]] && mv "${NOTES_DIR}" "${backup}"

# 13. Today's tasks while Notes runs open
notes_stop
plain_vault
notes_start
wait_for 60 notes_running
wait_for 20 has_today
ev "Main.panel.statusArea.dateMenu.menu.open(); 1" >/dev/null
# An empty day reads like the card around it: GNOME's italic would be a sheared Quicksand, at the largest size in it
PLACEHOLDER="const find = a => a.style_class?.includes?.('event-placeholder') ? a : a.get_children().map(find).find(c => c) ?? null; const font = find(Main.panel.statusArea.dateMenu.menu.actor)?.get_theme_node().get_font(); font.get_style() + ' ' + font.get_size() / 1024"
check "No Events is upright and as large as an event's title" ev_is "${PLACEHOLDER}" "0 13.5"
titles() { ev "${T}.taskTitles().join('|')"; }
has_titles() { [[ -n "$(titles)" ]]; }
no_titles() { [[ -z "$(titles)" ]]; }
# A row opens the tasks through the Notes that is open: it must take that without aborting (the real machine, 2026-10-30)
before_page="$(notes_pids)"
notes_start tasks
sleep 3
check "a launch asking the open Notes for its tasks leaves it running" test "$(notes_pids)" == "${before_page}"
wait_for 10 has_titles
check "the calendar lists the overdue task, then today's" test "$(titles)" == "Call the <b>bakery</b>|Water the plants"
check "...each title as its own text, never markup" ev_is \
    "${T}._tasks._list.get_children().every(r => !r.get_last_child().clutter_text.use_markup)" true
ev "Main.panel.statusArea.dateMenu._calendar.setDate(new Date(Date.now() + 86400000)); 1" >/dev/null
wait_for 5 no_titles
check "on tomorrow's page there is no tasks section" no_titles
ev "Main.panel.statusArea.dateMenu._calendar.setDate(new Date()); 1" >/dev/null
wait_for 5 has_titles
check "...and back on today's, it is there again" test "$(titles)" == "Call the <b>bakery</b>|Water the plants"
ev "Main.panel.statusArea.dateMenu.menu.close(); 1" >/dev/null

# Notes says Changed, and the open menu follows it: with the menu open, Notes stops (the list stays as it was) and
# starts again, and the tasks come back by Notes' word alone
notes_stop
ev "Main.panel.statusArea.dateMenu.menu.open(); 1" >/dev/null
wait_for 5 no_titles
notes_start
wait_for 60 notes_running
wait_for 10 has_titles
check "with the menu open, Notes' Changed brings the tasks in" test "$(titles)" == "Call the <b>bakery</b>|Water the plants"
ev "Main.panel.statusArea.dateMenu.menu.close(); 1" >/dev/null
check "closed, the menu keeps nothing of them" no_titles

# 15. Only the shell may ask
denied="$(user_bus gdbus call --session --dest org.amethystora.Notes --object-path /org/amethystora/Notes/Today \
    --method org.amethystora.Notes.Today1.GetToday 2>&1)"
# Refused as if there were no such object, so that it cannot tell open notes from locked ones by the answer
if [[ ${denied} == *UnknownMethod* ]]; then ok "a program of the account other than the shell is refused"; else not_ok "gdbus was answered" "${denied}"; fi
check "the shell is answered, for today" test "$(shell_today | cut -d, -f1)" == "[\"${today}\""

# 16. The quick note into the inbox, Notes not running, plain vault
notes_stop
keys super+alt+n
wait_for 30 capture_up
check "Super+Alt+N opens the quick note with the keyboard" capture_up
check "...and not Notes' own window" test "$(notes_windows)" == "Quick note"
check "...floating over the strip" ev_is "!${UI}.get('tiling')?.engine?.isTiled(${QN})" true
check "...in the middle of the screen" ev_is "(() => { const r = ${QN}.get_frame_rect(); const a = Main.layoutManager.getWorkAreaForMonitor(${QN}.get_monitor()); return Math.abs(r.x + r.width / 2 - (a.x + a.width / 2)) <= 2 && Math.abs(r.y + r.height / 2 - (a.y + a.height / 2)) <= 2; })()" true
keys "text:ask sam about trains tomorrow" return
wait_for 10 capture_gone
check "Enter saves and closes it" capture_gone
has_saved() { [[ -n "$(saved 'ask sam about trains')" ]]; }
wait_for 10 has_saved
check "...as a task in the inbox, due tomorrow" test "$(saved 'ask sam about trains')" == "${tomorrow} inbox"
wait_for 30 notes_gone
check "...and Notes, started for it, leaves again" notes_gone

# 18. Over an open Notes, Esc closes it and saves nothing
notes_start
wait_for 60 notes_running
sleep 3
count="$(jq '.tasks | length' "${NOTES_DIR}/data.json")"
keys super+alt+n
wait_for 30 capture_up
keys "text:not this one" esc
wait_for 10 capture_gone
check "Esc closes the quick note" capture_gone
sleep 2
check "...without saving" test "$(jq '.tasks | length' "${NOTES_DIR}/data.json")" == "${count}"
check "...and Notes' own window is still there" test "$(notes_windows)" == "Notes"
check "...and its notes still open" has_today

# 14. The Today object only while the notes are open
notes_stop
encrypted_vault
notes_start
wait_for 60 notes_running
sleep 3
check "locked: no Today object" no_today
keys "text:${FIXTURE_PASSPHRASE}" return
wait_for 30 has_today
check "unlocked: the Today object, with GetToday" has_today
has_changed() { today_object | grep -q 'Changed'; }
check "...and Changed" has_changed
keys ctrl+l
wait_for 5 no_today
check "locked again: gone" no_today

# 17. The quick note with the notes locked, Notes not running
notes_stop
keys super+alt+n
wait_for 30 capture_up
keys "text:fix the oven today" return "text:wrong pass" return
sleep 4
check "a wrong passphrase leaves the quick note open" capture_up
keys "text:${FIXTURE_PASSPHRASE}" return
wait_for 30 capture_gone
check "the right one saves and closes it" capture_gone
wait_for 30 notes_gone
check "...and Notes, started for it, locks and leaves" notes_gone
notes_start
wait_for 60 notes_running
sleep 3
keys "text:${FIXTURE_PASSPHRASE}" return
wait_for 30 has_today
oven_due() { [[ "$(shell_today)" == *"fix the oven"* ]]; }
check "the saved task is in the vault, due today" oven_due

# 17b. Notes in the background with a reminder to come (closed, so locked): the quick note unlocks, saves, and locks
# again, and Notes stays; nothing but Notes' own files changed in the home meanwhile
keys ctrl+2 ctrl+n "text:dentist tomorrow 9am" return
sleep 2
# The image closes a window with Super+W (Alt+F4 is not bound)
keys super+w
wait_for 20 no_today
check "Notes' window closed with a reminder to come: locked" no_today
check "...and still running" test -n "$(notes_pids)"
marker="$(mktemp)"
touch "${marker}"
sleep 1
keys super+alt+n
wait_for 30 capture_up
keys "text:water the fern today" return "text:${FIXTURE_PASSPHRASE}" return
wait_for 30 capture_gone
wait_for 5 no_today
check "after the save, the notes are locked again" no_today
check "...and Notes stays for its reminder" test -n "$(notes_pids)"
written="$(find "${HOME_DIR}" -newer "${marker}" -type f -not -path "${NOTES_DIR}/*" -not -path '*/.cache/*' \
    -not -path '*/.config/dconf/*' -not -path '*/.local/share/gnome-shell/*' -not -path '*/.local/state/*' \
    -not -name 'recently-used.xbel*' 2>/dev/null | head -n 5)"
if [[ -z "${written}" ]]; then ok "nothing outside Notes' folder was written"; else not_ok "files written meanwhile" "${written}"; fi
rm -f "${marker}"
notes_stop
rm -rf "${NOTES_DIR}"
[[ -e "${backup}" ]] && mv "${backup}" "${NOTES_DIR}"

# 19. Super+Alt+N is the image's
C30="org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom30/"
check "custom30 binds Super+Alt+N" test "$(user_bus gsettings get "${C30}" binding)" == "'<Super><Alt>n'"
check "...to amethystora-notes capture" test "$(user_bus gsettings get "${C30}" command)" == "'amethystora-notes capture'"

# 20. Rest reminders are GNOME's own key
breaks() { user_bus gsettings get org.gnome.desktop.break-reminders selected-breaks; }
in_session "ame desktop breaks on" >/dev/null
check "breaks on: the eyesight break" test "$(breaks)" == "['eyesight']"
check "...and GNOME's break manager runs" test "$(ev 'Main.breakManager.state')" != 0
in_session "ame desktop breaks both" >/dev/null
check "breaks both" test "$(breaks)" == "['eyesight', 'movement']"
in_session "ame desktop breaks off" >/dev/null
check "breaks off" test "$(breaks)" == "@as []"
check "...and GNOME's break manager stops" ev_is "Main.breakManager.state" 0

# 21. The weather is off until chosen
TODAY_SCHEMA=org.gnome.shell.extensions.amethystora-ui.today
check "the weather is off as the image ships" test "$(user_bus gsettings get "${TODAY_SCHEMA}" weather)" == false
ev "Main.panel.statusArea.dateMenu.menu.open(); 1" >/dev/null
sleep 1
check "GNOME's weather section is hidden" ev_is "Main.panel.statusArea.dateMenu._weatherItem.visible" false
if user_bus flatpak info org.gnome.Weather >/dev/null 2>&1; then
    check "...and the card says it is off" ev_is "${T}.weatherCard()" true
else
    skip "GNOME Weather is not installed: no card"
fi
check "...nothing beside the clock" ev_is "${T}.weatherPill()" false
ev "Main.panel.statusArea.dateMenu.menu.close(); 1" >/dev/null
sends() { in_session "/usr/libexec/amethystora-sends --json" | jq -r '.[] | select(.id == "sends-weather") | .state'; }
check "What this machine sends lists the weather, off" test "$(sends)" == off
in_session "ame desktop weather on" >/dev/null
check "ame desktop weather on: on, and listed so" test "$(user_bus gsettings get "${TODAY_SCHEMA}" weather) $(sends)" == "true on"
in_session "ame desktop weather off" >/dev/null
check "...and off again" test "$(user_bus gsettings get "${TODAY_SCHEMA}" weather)" == false

# 22. The bar over a fullscreen window
# The peek, timed in the shell: when the panel started sliding down and reached its place, when the pointer left
# the bar, and when GNOME hid the bar again (ms). $1 is the box GNOME tracks, $2 the panel that slides in it.
peek_record() {
    ev "(() => { const G = imports.gi.GLib; const box = $1, panel = $2; const r = globalThis.__peekRec = {}; const now = () => G.get_monotonic_time() / 1000; globalThis.__peekUndo = [[panel, panel.connect('notify::translation-y', p => { if (p.translation_y < 0 && r.start === undefined) r.start = now(); if (p.translation_y === 0 && r.start !== undefined && r.down === undefined) r.down = now(); })], [box, box.connect('notify::hover', b => { if (!b.hover && r.down !== undefined && r.left === undefined) r.left = now(); })], [box, box.connect('notify::visible', b => { if (!b.visible && r.left !== undefined && r.gone === undefined) r.gone = now(); })]]; return 1; })()" >/dev/null
}
peek_times() { ev "(() => { const r = globalThis.__peekRec ?? {}; return [Math.round(r.down - r.start), Math.round(r.gone - r.left)].join(' '); })()"; }
peek_stop() { ev "(() => { (globalThis.__peekUndo ?? []).forEach(([a, id]) => a.disconnect(id)); globalThis.__peekUndo = []; return 1; })()" >/dev/null; }
# The VM draws in software, where GNOME turns its animations off: the peek's slides are forced on while they are timed
animations() { ev "global.force_animations = $1; 1" >/dev/null; }
trap 'animations false' EXIT
user_bus systemd-run --user --collect --unit="vmlab-today-files-$$" nautilus --new-window >/dev/null
wait_for 60 ev_is "global.get_window_actors().some(a => a.meta_window.get_wm_class() === 'org.gnome.Nautilus')" true
files="global.get_window_actors().map(a => a.meta_window).find(w => w.get_wm_class() === 'org.gnome.Nautilus')"
ev "${files}.make_fullscreen(); 1" >/dev/null
wait_for 10 ev_is "Main.layoutManager.panelBox.visible" false
check "a fullscreen window hides the bar" ev_is "Main.layoutManager.panelBox.visible" false
P="Main.layoutManager.primaryIndex"
# A push against the top edge: GNOME's pressure barrier counts at most 15 px of each step, and wants 100 within a
# second, so it takes at least seven steps against the edge (and one to reach it)
PUSH=(move:0.5,0.05)
for _ in $(seq 12); do PUSH+=("rel:0,-30"); done
animations true
peek_record Main.layoutManager.panelBox Main.panel
keys "${PUSH[@]}"
wait_for 5 ev_is "${B}.peeking(${P})" true
check "pushing against the top edge brings it down" ev_is "${B}.peeking(${P}) && Main.layoutManager.panelBox.visible" true
wait_for 2 ev_is "Main.panel.translation_y" 0
check "...sliding into its place" ev_is "Main.panel.translation_y" 0
keys move:0.5,0.5
# How long it stayed is timed in the shell (peek_times): an Eval takes too long to time it from here
sleep 1.5
check "moving away, it goes within a second and a half" ev_is "${B}.peeking(${P}) || Main.layoutManager.panelBox.visible" false
read -r down gone <<<"$(peek_times)"
check "it slid down within 400 ms (${down} ms)" test "${down:-999}" -le 400
check "it went 700 ms after the pointer left, and its slide (${gone} ms)" test "${gone:-0}" -ge 700 -a "${gone:-0}" -le 1300
peek_stop
# Pushed again while it goes, it comes back from where it is
# The second push is made by the barrier's own code (_peek, what its trigger runs) from the shell, at the moment the bar
# has started up, which no push from here could time; the bar must turn back without having been hidden
keys "${PUSH[@]}"
wait_for 5 ev_is "${B}.peeking(${P})" true
wait_for 2 ev_is "Main.panel.translation_y" 0
ev "(() => { const bars = ${B}, P = ${P}, box = Main.layoutManager.panelBox; const r = globalThis.__again = {during: false, hid: false}; const a = Main.panel.connect('notify::translation-y', p => { if (r.during && p.translation_y === 0) r.back = true; if (!r.during && bars._peeks.get(P)?.going && p.translation_y < 0) { r.during = true; r.from = p.translation_y; bars._peek(P); } }); const b = box.connect('notify::visible', x => { if (!x.visible) r.hid = true; }); globalThis.__againUndo = () => { Main.panel.disconnect(a); box.disconnect(b); }; return 1; })()" >/dev/null
keys move:0.5,0.5
wait_for 5 ev_is "Boolean(__again.back)" true
check "pushed again as it goes, it comes back from where it was, never hidden" ev_is \
    "__again.during && __again.back && !__again.hid && __again.from > -Main.layoutManager.panelBox.height" true
ev "__againUndo(); 1" >/dev/null
keys move:0.5,0.5
wait_for 5 ev_is "${B}.peeking(${P})" false
animations false
in_session "ame desktop top-bar peek off" >/dev/null
keys "${PUSH[@]}"
sleep 2
check "with the peek off, the same push leaves it hidden" ev_is "${B}.peeking(${P}) || Main.layoutManager.panelBox.visible" false
in_session "ame desktop top-bar peek on" >/dev/null
keys move:0.5,0.05 rel:0,-3 rel:0,-3 rel:0,-3 rel:0,-3 rel:0,-3
sleep 2
check "a soft push does nothing" ev_is "${B}.peeking(${P})" false
ev "${files}.unmake_fullscreen(); 1" >/dev/null
user_bus systemctl --user stop "vmlab-today-files-$$" >/dev/null 2>&1

# 23. Ten disable/enable cycles
# One step an Eval: GNOME finishes each before the next
for i in $(seq 10); do
    ev "Main.extensionManager.disableExtension('${UUID}'); 1" >/dev/null
    ev "Main.extensionManager.enableExtension('${UUID}'); 1" >/dev/null
done
sleep 3
ours="(box => [box.get_children().filter(c => c.has_style_class_name?.('amethystora-today-tasks')).length, box.get_children().filter(c => c.has_style_class_name?.('amethystora-today-weather-off')).length].join())(Main.panel.statusArea.dateMenu._eventsItem.get_parent())"
check "ten cycles: one tasks section and one weather card in the menu" ev_is "${ours}" "1,1"
# GNOME 51 turns an extension off once the Eval that asked has returned (its manager awaits a lock): one step each
ev "Main.extensionManager.disableExtension('${UUID}'); 1" >/dev/null
sleep 1
check "...and nothing of theirs while disabled" ev_is "${ours} === '0,0' && !Object.prototype.hasOwnProperty.call(Main.panel.statusArea.dateMenu._weatherItem._weatherClient, 'update') && !Object.prototype.hasOwnProperty.call(Main.layoutManager, '_updateActorVisibility')" true
ev "Main.extensionManager.enableExtension('${UUID}'); 1" >/dev/null
sleep 2

log="$(journalctl --since "${since}" "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --no-pager -o cat 2>/dev/null)"
errors="$(grep -E 'JS ERROR|Amethystora UI: the [a-z]+ module did not' <<<"${log}" | head -n 10)"
if [[ -z "${errors}" ]]; then ok "nothing threw in the shell meanwhile"; else not_ok "the shell's log has errors" "${errors}"; fi
finish
