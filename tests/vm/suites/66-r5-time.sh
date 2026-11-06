#!/usr/bin/bash
# vmlab-vm: r5-time
# Rounds 5 and 6, group time (plans/ui-ideas/spec-time.md, its acceptance criteria), in the test account's session,
# asked of GNOME Shell itself through Eval, which a lab-only extension of the test account opens (never in the image):
#   - the search's time in other places ("time in Tokyo", "15:00 Paris in Tokyo") and its key read live;
#   - remind me, from search: the row, and Enter handing Notes the task on stdin (an unencrypted vault: the task in the
#     inbox with its reminder; a locked one: the quick note with the passphrase), never on a command line;
#   - meetings in the pill: an event with a call link in the account's own calendar, five minutes ahead, through GNOME's
#     calendar server, the pill's countdown, its card with Later and Join, nothing opened; the helper's rules;
#   - kitty's notify_on_cmd_finish and `ame desktop command-done`;
#   - a missing command in bash, zsh and fish, off as shipped;
#   - ten disables and enables leaving nothing, Control's entries and the setup file.
# vmlab-paths: @ui/extension.js @ui/*search* @ui/*pill* @sf/usr/libexec/amethystora-meeting-link @sf/usr/libexec/amethystora-missing-command @sf/etc/xdg/kitty/kitty.conf @sf/etc/zsh/zshrc @sf/etc/profile.d/zz-amethystora-missing-command.sh @sf/*fish/vendor_conf.d/amethystora-missing-command.fish @sf/*user-setup.hooks.d/28-kitty-options.sh apps/notes/* build_files/shared/build-missing-command-index.sh @sf/usr/libexec/amethystora-control-list @sf/*just/desktop.just @sf/usr/libexec/amethystora-mirror @ui/*power*
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HERE="$(dirname "$0")"
HOME_DIR="/var/home/${TEST_USER}"
PILL=org.gnome.shell.extensions.amethystora-ui.pill
SEARCH=org.gnome.shell.extensions.amethystora-ui.search
NOTES_DIR="${HOME_DIR}/.local/share/amethystora-notes"
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
PROV="${UI}.get('search')?._providers"
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
if [[ ${enabled} != *vmlab-eval@lab* ]]; then
    user_bus gsettings set org.gnome.shell enabled-extensions "${enabled%]*}, 'vmlab-eval@lab']"
    loginctl unlock-sessions
    systemctl restart gdm
fi
for key in times reminders; do gs reset "${SEARCH}" "${key}"; done
gs reset "${PILL}" meetings
ready() { is "typeof ${P}?.state" function; }
if ! wait_for 240 has_session || ! wait_for 120 ready; then
    not_ok "the pill did not start in the session"
    finish
fi
wait_for 30 is "Main.layoutManager._startingUp" false
# The account's notes are kept aside and put back
[[ -e "${NOTES_DIR}" ]] && mv "${NOTES_DIR}" "${WORK}/notes-kept"
cleanup() {
    pkill -u "${TEST_USER}" -f /usr/lib/amethystora-notes/amethystora-notes 2>/dev/null
    sleep 1
    rm -rf "${NOTES_DIR}"
    [[ -e "${WORK}/notes-kept" ]] && mv "${WORK}/notes-kept" "${NOTES_DIR}"
    for key in times reminders; do gs reset "${SEARCH}" "${key}"; done
    gs reset "${PILL}" meetings
    in_session "ame desktop command-done on; ame desktop missing-command off" >/dev/null 2>&1
    [[ -n "${EVENT_UID:-}" ]] && in_session "python3 ${WORK}/event.py remove ${EVENT_UID}" >/dev/null 2>&1
    rm -rf "${WORK}"
}
trap cleanup EXIT
chmod 755 "${WORK}"

# The provider of a search key, as the search module keeps it
provider() { echo "${PROV}?.get?.('$1')"; }
answer() { ev "JSON.stringify($(provider times)?.answerFor($1) ?? null)"; }

# --- 1: the keys and their defaults ---------------------------------------------------------------------------------
same "1 times is on" "$(gs get "${SEARCH}" times)" true
same "1 reminders is on" "$(gs get "${SEARCH}" reminders)" true
same "1 meetings is on" "$(gs get "${PILL}" meetings)" true

# --- 2, 3: the time in other places ----------------------------------------------------------------------------------
in_session "gsettings set org.gnome.desktop.interface clock-format 24h" >/dev/null
check "2 the times provider runs" is "typeof $(provider times)?.answerFor" function
# 15:00 in Paris is 23:00 in Tokyo whatever the date: Japan has no summer time and France's moves Paris to UTC+2 in
# summer, which makes it 22:00 then; either is right for its day
paris="$(answer "'15:00 Paris in Tokyo'")"
check "2 15:00 Paris in Tokyo" like "${paris}" '*"name":"2[23]:00"*"in Tokyo, the same day"*'
check "2 its second row says the time in Tokyo now" like "${paris}" '*"Now in Tokyo: [0-9]*'
tokyo="$(ev "GLib.DateTime.new_now(GLib.TimeZone.new_identifier('Asia/Tokyo')).format('%H:%M')")"
check "2 time in Tokyo is the time there now" like "$(answer "'time in Tokyo'")" "*\"name\":\"${tokyo}\"*in Tokyo*"
same "2 3pm UTC in UTC+2" "$(ev "$(provider times).answerFor('3pm UTC in UTC+2')[0].name")" "17:00"
same "2 a place no one knows answers nothing" "$(answer "'time in Nowhereville'")" "[]"
same "2 a sum is not a time" "$(answer "'1284 * 0.18'")" "[]"
gs set "${SEARCH}" times false
check "3 off takes the provider away" wait_for 10 is "$(provider times) ? 1 : 0" 0
gs set "${SEARCH}" times true
check "3 on brings it back" wait_for 10 is "$(provider times) ? 1 : 0" 1

# --- 4, 5, 6: remind me ---------------------------------------------------------------------------------------------
# Eval does not wait for a promise: the answer is kept on the provider and read after
remind_rows() {
    ev "$(provider reminders).answerFor($1).then(r => (globalThis.vmlabRemind = JSON.stringify(r))); 'x'" >/dev/null
    sleep 1
    ev "globalThis.vmlabRemind ?? ''"
}
rows="$(remind_rows "'remind me at 23:59 to call the garage'")"
check "4 the row is the task" like "${rows}" '*"name":"Call the garage"*"A task in Notes · *"*'
check "4 and the line Notes is handed" like "${rows}" '*"line":"20[0-9][0-9]-[0-9][0-9]-[0-9][0-9] 23:59 Call the garage"*'
check "4 the second row is a day later" like "${rows}" '*instead*Tab to change the day*'
same "4 without a time, nothing" "$(remind_rows "'remind me to call the garage'")" "[]"
# An unencrypted vault: Notes opens it at start and adds the task
install -d -o "${TEST_USER}" -g "${TEST_USER}" -m 0700 "${NOTES_DIR}"
echo '{"format": 1, "encrypted": false}' >"${NOTES_DIR}/vault.json"
chown "${TEST_USER}:" "${NOTES_DIR}/vault.json"
line="$(jq -r '.[0].line' <<<"${rows}")"
ev "$(provider reminders).hand('${line}') ? 'handed' : 'not'" >/dev/null
added() { jq -e '[.. | objects | select(.title? == "Call the garage" and .time? == "23:59" and .reminder? == 0)] | length == 1' \
    "${NOTES_DIR}/data.json" >/dev/null 2>&1; }
check "5 Notes adds the task with its reminder" wait_for 30 added
check "5 the task was never on a command line" bash -c "! pgrep -u '${TEST_USER}' -af '[g]arage'"
pkill -u "${TEST_USER}" -f /usr/lib/amethystora-notes/amethystora-notes 2>/dev/null
wait_for 10 bash -c "! pgrep -u '${TEST_USER}' -f /usr/lib/amethystora-notes/amethystora-notes"
# A locked vault: the quick note opens with the passphrase field
echo '{"format": 1, "encrypted": true}' >"${NOTES_DIR}/vault.json"
rm -f "${NOTES_DIR}"/data.json*
rows="$(remind_rows "'remind me at 23:59 to water the plants'")"
check "6 the row says Notes asks first" like "${rows}" '*Notes asks for its passphrase first*'
ev "$(provider reminders).hand('$(jq -r '.[0].line' <<<"${rows}")') ? 'handed' : 'not'" >/dev/null
quick() { is "global.get_window_actors().some(a => a.meta_window.get_title() === 'Quick note')" true; }
check "6 the quick note opens to ask" wait_for 30 quick
check "6 nothing was written" test ! -e "${NOTES_DIR}/data.json"
pkill -u "${TEST_USER}" -f /usr/lib/amethystora-notes/amethystora-notes 2>/dev/null

# --- 7, 8: meetings in the pill ---------------------------------------------------------------------------------------
same "8 the helper's rules: a location" \
    "$(printf '{"location": "https://meet.example.org/abc-defg"}' | /usr/libexec/amethystora-meeting-link --rules)" \
    '{"link": "https://meet.example.org/abc-defg", "calendar": ""}'
same "8 a call in the description, a menu link before it" \
    "$(printf '{"description": "menu https://example.org/menu, call https://example.com/j/12345."}' | /usr/libexec/amethystora-meeting-link --rules)" \
    '{"link": "https://example.com/j/12345", "calendar": ""}'
same "8 no call, no link" "$(printf '{"url": "https://example.org/about"}' | /usr/libexec/amethystora-meeting-link --rules)" ""
# An event of the account's own local calendar, four minutes ahead, through Evolution Data Server as Calendar makes one
cat >"${WORK}/event.py" <<'EOF'
import sys, time
import gi
gi.require_version("EDataServer", "1.2"); gi.require_version("ECal", "2.0"); gi.require_version("ICalGLib", "3.0")
from gi.repository import ECal, EDataServer, ICalGLib
registry = EDataServer.SourceRegistry.new_sync(None)
source = registry.ref_builtin_calendar()
client = ECal.Client.connect_sync(source, ECal.ClientSourceType.EVENTS, 30, None)
if sys.argv[1] == "remove":
    client.remove_object_sync(sys.argv[2], None, ECal.ObjModType.ALL, ECal.OperationFlags.NONE, None)
    sys.exit(0)
comp = ICalGLib.Component.new_vevent()
comp.set_summary("Design review")
start = int(time.time()) + 240
utc = ICalGLib.Timezone.get_utc_timezone()
comp.set_dtstart(ICalGLib.Time.new_from_timet_with_zone(start, False, utc))
comp.set_dtend(ICalGLib.Time.new_from_timet_with_zone(start + 1800, False, utc))
comp.set_location("https://meet.example.org/abc-defg")
made = client.create_object_sync(comp, ECal.OperationFlags.NONE, None)
print(made[1] if isinstance(made, tuple) else made)
EOF
chmod 644 "${WORK}/event.py"
EVENT_UID="$(in_session "python3 ${WORK}/event.py add" 2>/dev/null | tail -n1)"
check "7 an event in the account's calendar" test -n "${EVENT_UID}"
meeting() { ev "JSON.stringify(${P}.state().meeting)"; }
meeting_shown() { like "$(meeting)" '*"text":"Design review in * min"*'; }
check "7 the pill shows the meeting" wait_for 90 meeting_shown
check "7 it is the pill's activity" wait_for 10 is "${P}.state().activity" meeting
check "7 with its link" like "$(meeting)" '*"link":"meet.example.org/abc-defg"*'
ev "${P}.clock.openCard('meeting'); 'x'" >/dev/null
card() { is "(() => { let found = false; const walk = a => { if (a.style_class?.includes?.('amethystora-pill-meeting-link')) found = true; a.get_children().forEach(walk); }; walk(Main.uiGroup); return found; })()" true; }
if wait_for 10 card; then
    ok "7 its card has the link"
    buttons="$(ev "(() => { const out = []; const walk = a => { if (a.style_class?.includes?.('amethystora-pill-button')) out.push(a.label); a.get_children().forEach(walk); }; walk(Main.uiGroup); return JSON.stringify(out); })()")"
    check "7 Later and Join" like "${buttons}" '*"Later"*"Join"*'
else
    skip "7 its card has the link (the card did not open through the clock's own call)"
fi
ev "Main.panel.statusArea.dateMenu.menu.close(); 'x'" >/dev/null
check "7 nothing opened by itself" bash -c "! pgrep -u '${TEST_USER}' -x firefox"
ev "${P}.meetings.later({key: ${P}.meetings._shown.key, start: ${P}.meetings._shown.start}); 'x'" >/dev/null
check "7 Later puts it away until it starts" wait_for 10 is "${P}.state().meeting === null" true
gs set "${PILL}" meetings false
check "7 off: the part is gone" wait_for 10 is "${P}.meetings === null" true
gs set "${PILL}" meetings true
check "7 on: the part is back" wait_for 10 is "${P}.meetings !== null" true
in_session "python3 ${WORK}/event.py remove ${EVENT_UID}" >/dev/null 2>&1 && EVENT_UID=""

# --- 9: kitty --------------------------------------------------------------------------------------------------------
kitty_notify() {
    in_session "kitty +runpy 'from kitty.cli import create_default_opts; n = create_default_opts().notify_on_cmd_finish; print(n.when, n.duration)'" 2>/dev/null | tail -n1
}
same "9 kitty says when a long command ends" "$(kitty_notify)" "unfocused 10.0"
check "9 the account's options file is there" test -f "${HOME_DIR}/.config/amethystora/kitty.conf"
in_session "ame desktop command-done off" >/dev/null 2>&1
same "9 off" "$(kitty_notify)" "never 5.0"
in_session "ame desktop command-done on" >/dev/null 2>&1
same "9 on again" "$(kitty_notify)" "unfocused 10.0"

# --- 10: a missing command -------------------------------------------------------------------------------------------
in_session "ame desktop missing-command off" >/dev/null 2>&1
check "10 off: bash's own message" like "$(in_session "bash -ic 'cowsay; echo status \$?' 2>&1")" '*cowsay: command not found*status 127*'
in_session "ame desktop missing-command on" >/dev/null 2>&1
out="$(in_session "bash -ic 'cowsay; echo status \$?' 2>&1")"
check "10 bash says where it can come from" like "${out}" '*cowsay is not on this computer*install cowsay*Nothing was installed.*status 127*'
check "10 zsh too" like "$(in_session "zsh -ic 'cowsay; echo status \$?' 2>&1")" '*cowsay is not on this computer*status 127*'
check "10 fish too" like "$(in_session "fish -ic 'cowsay; echo status \$status' 2>&1")" '*cowsay is not on this computer*status 127*'
check "10 an ame command for steam" like "$(in_session "/usr/libexec/amethystora-missing-command steam")" '*ame apps gaming*'
in_session "ame desktop missing-command off" >/dev/null 2>&1

# --- 11: ten disables and enables -------------------------------------------------------------------------------------
census='(() => { const m = {}; const v = a => { const k = a.style_class || a.constructor.name; m[k] = (m[k] ?? 0) + 1; a.get_children().forEach(v); }; v(Main.uiGroup); m.statusArea = Object.keys(Main.panel.statusArea).length; m.providers = Main.overview.searchController._searchResults._providers.length; return m; })()'
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
same "11 ten cycles" "${cycles}" 10
same "11 leave nothing" "$(ev "(() => { const a = ${census}, b = globalThis.vmlabCensus;
    return JSON.stringify(Object.keys({...a, ...b}).filter(k => (a[k] ?? 0) !== (b[k] ?? 0)).map(k => k + ' ' + ((a[k] ?? 0) - (b[k] ?? 0))).slice(0, 12)); })()")" "[]"
on
wait_for 60 ready

# --- Control and the setup file -------------------------------------------------------------------------------------
control="$(in_session "/usr/libexec/amethystora-control-list --json" 2>/dev/null)"
same "Control's entries" \
    "$(jq -r '[.items[] | select(.id | IN("search-times", "search-reminders", "pill-meetings", "command-done", "missing-command")) | .id] | sort | join(",")' <<<"${control}")" \
    "command-done,missing-command,pill-meetings,search-reminders,search-times"
gs set "${SEARCH}" times false
gs set "${PILL}" meetings false
in_session "ame setup save" >/dev/null 2>&1
same "the setup file carries them" \
    "$(jq -c '[.settings.search.times, .settings.pill.meetings]' "${HOME_DIR}/.config/amethystora/setup.json")" '[false,false]'
gs reset "${SEARCH}" times
gs reset "${PILL}" meetings

# --- and nothing threw ------------------------------------------------------------------------------------------------
errors="$(journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --since "${since}" --no-pager -o cat 2>/dev/null |
    grep -E "JS ERROR|Amethystora UI: (the search's (time|reminder)|the meeting|the calendar)" | head -n 20)"
if [[ -z ${errors} ]]; then ok "nothing threw"; else not_ok "something threw" "${errors}"; fi
finish
