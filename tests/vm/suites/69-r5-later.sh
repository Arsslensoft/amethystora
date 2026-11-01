#!/usr/bin/bash
# Rounds 5 and 6, group later (plans/ui-ideas/spec-later.md): the seven ideas built offered and off, each checked as a
# person would meet it, through GNOME Shell's Eval (the lab-only extension of the test account opens it, never the image):
#   C  captions in more languages (AC1-AC4): ten pinned languages, a model fetched by its checksum, the module on it
#   D  dictation (AC5-AC9): its key only while on, a sentence played into a virtual microphone typed into a GTK field,
#      never into a password field, stopped when the focus leaves
#   B  bookmarks in search (AC10-AC12): a planted profile's bookmark answers, a page only visited never does, the copy gone
#   N  Notes in search (AC13-AC15): titles while Notes is open, nothing while locked or not running, the shell alone asks
#   M  files by meaning (AC16-AC19): a planted file in Documents, read by localsearch, answers a paraphrase
#   K  click with the keyboard (AC20-AC23): letters on a GTK window's buttons, typed, clicked
#   G  the guest login (AC24-AC28): a Security switch, PAM refusing it but at the login screen, the home a tmpfs
# The lab renders in software: the dictation pill's breath and the letters' fade are not checked, only their state.
# vmlab-paths: @ui/*captions* @ui/*search* @ui/*access* @sf/usr/libexec/amethystora-captions @sf/usr/libexec/amethystora-bookmarks @sf/usr/libexec/amethystora-file-meaning @sf/usr/libexec/amethystora-letters @sf/usr/libexec/amethystora-guest @sf/usr/libexec/amethystora-hardening @sf/usr/libexec/amethystora-security-status @sf/usr/libexec/amethystora-sends @sf/usr/libexec/amethystora-control-list @sf/*just/desktop.just @sf/*just/security.just @sf/*systemd/user/amethystora-file-meaning* apps/notes/* apps/agent-tools/*
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@iarsslen.github.io
HOME_DIR="/var/home/${TEST_USER}"
WORK=/run/vmlab-69
CAPTIONS=/usr/libexec/amethystora-captions
CAPTIONS_SCHEMA=org.gnome.shell.extensions.amethystora-ui.captions
SEARCH_SCHEMA=org.gnome.shell.extensions.amethystora-ui.search
ACCESS_SCHEMA=org.gnome.shell.extensions.amethystora-ui.access
NOTES_DIR="${HOME_DIR}/.local/share/amethystora-notes"
# Eval's answer is JSON inside a GVariant, which gdbus quotes with ' or " as the text needs: parsed as both, as
# 67-r4-later.sh does; a string printed bare, anything else as JSON
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
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
gs() { in_session "gsettings $*" 2>/dev/null; }
keys() { python3 "$(dirname "$0")/keys.py" "$@"; }
MODULE="Main.extensionManager.lookup('${UUID}').stateObj.modules"
cstate() { ev "JSON.stringify(${MODULE}.get('captions').state())"; }
# A provider of the search, asked as the search box asks it: its ids for the words, then their rows, as JSON
ask_provider() { # KEY WORDS...
    local key="$1"
    shift
    ev "global._v69 = null; (async () => { const p = ${MODULE}.get('search')._providers.get('${key}');
        if (!p) { global._v69 = 'none'; return; }
        const ids = await p.getInitialResultSet($(jq -cn '$ARGS.positional' --args "$@"), null);
        global._v69 = JSON.stringify(await p.getResultMetas(ids)); })(); 1" >/dev/null
    answered() { [[ "$(ev "global._v69 === null ? 'waiting' : 'in'")" == in ]]; }
    wait_for 20 answered
    ev "global._v69"
}

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
    systemctl restart gdm
fi
ready() { [[ "$(ev "typeof ${MODULE}?.get('captions')")" == object ]]; }
if ! wait_for 240 has_session || ! wait_for 120 ready; then
    not_ok "Amethystora UI did not start in the session"
    finish
fi
ev "Main.overview.hide(); 'hidden'" >/dev/null
IDLE_BEFORE="$(gs get org.gnome.desktop.session idle-delay)"
gs set org.gnome.desktop.session idle-delay 0
install -d -m 0755 "${WORK}"
install -d -m 0755 -o "${TEST_USER}" -g "${TEST_USER}" "${WORK}/own"
cleanup() {
    [[ -n "${IDLE_BEFORE:-}" ]] && gs set org.gnome.desktop.session idle-delay "${IDLE_BEFORE#uint32 }"
    in_session "ame desktop dictation off" >/dev/null 2>&1
    in_session "ame desktop captions remove" >/dev/null 2>&1
    for part in bookmarks notes meaning; do in_session "ame desktop search ${part} off" >/dev/null 2>&1; done
    in_session "ame desktop access letters off" >/dev/null 2>&1
    [[ -n "${MIC_MODULES:-}" ]] && for module in ${MIC_MODULES}; do in_session "pactl unload-module ${module}" >/dev/null 2>&1; done
    user_bus systemctl --user stop vmlab-69-fields.service >/dev/null 2>&1
    pkill -u "${TEST_USER}" -f '[/]usr/lib/amethystora-notes/amethystora-notes' 2>/dev/null
    /usr/libexec/amethystora-hardening set guest-login off >/dev/null 2>&1
    loginctl unlock-sessions 2>/dev/null
    user_bus gnome-extensions enable "${UUID}" 2>/dev/null
}
trap cleanup EXIT

# A GTK 4 window with a field, a password field and three buttons, each logging what happens to it
cat >"${WORK}/own/fields.js" <<'EOF'
import Gtk from 'gi://Gtk?version=4.0';
import GLib from 'gi://GLib';
import Gio from 'gi://Gio';

const here = GLib.path_get_dirname(import.meta.url.replace(/^file:\/\//, ''));
Gtk.init();
const loop = new GLib.MainLoop(null, false);
const log = (name, text) => {
    const stream = Gio.File.new_for_path(`${here}/fields.log`).append_to(Gio.FileCreateFlags.NONE, null);
    stream.write_all(new TextEncoder().encode(`${name}\t${JSON.stringify(text)}\n`), null);
    stream.close(null);
};
const win = new Gtk.Window({title: 'vmlab-69-fields', default_width: 640, default_height: 420});
const box = new Gtk.Box({orientation: Gtk.Orientation.VERTICAL, spacing: 8, margin_top: 20, margin_start: 20, margin_end: 20});
win.set_child(box);
const fields = {plain: new Gtk.Entry(), password: new Gtk.PasswordEntry()};
for (const [name, field] of Object.entries(fields)) {
    box.append(field);
    field.connect('notify::text', () => log(name, field.text));
}
for (const name of ['Output', 'Input', 'Test speakers']) {
    const button = new Gtk.Button({label: name});
    button.connect('clicked', () => log('clicked', name));
    box.append(button);
}
const focus = Gio.File.new_for_path(`${here}/focus.txt`);
const monitor = focus.monitor_file(Gio.FileMonitorFlags.NONE, null);
monitor.connect('changed', () => {
    try {
        fields[new TextDecoder().decode(focus.load_contents(null)[1]).trim()]?.grab_focus_without_selecting();
    } catch {}
});
win.connect('close-request', () => loop.quit());
win.present();
loop.run();
EOF
chown "${TEST_USER}:" "${WORK}/own/fields.js"
fields_up() {
    rm -f "${WORK}/own/fields.log"
    user_bus systemd-run --user --collect --unit=vmlab-69-fields gjs -m "${WORK}/own/fields.js" >/dev/null 2>&1
}
fields_here() {
    ev "global.display.get_tab_list(0, null).find(w => w.get_title() === 'vmlab-69-fields')?.activate(global.get_current_time()); 'back'" >/dev/null
    [[ "$(ev "global.display.focus_window?.get_title()")" == vmlab-69-fields ]]
}
field_text() { grep "^$1	" "${WORK}/own/fields.log" 2>/dev/null | tail -n 1 | cut -f2- | jq -r .; }
focus_field() { printf '%s\n' "$1" >"${WORK}/own/focus.txt"; sleep 1; }

# === C. Captions in more languages =====================================================================================

same "C1 ten languages, English first, none here for a fresh account" \
    "$(in_session "${CAPTIONS} remove; ${CAPTIONS} languages" 2>/dev/null | jq -r '[.[0].code, length, (map(select(.here)) | length)] | join(" ")')" \
    "en 10 0"
same "C1 the key is English" "$(gs get "${CAPTIONS_SCHEMA}" language)" "'en'"
printf 'not a model' >"${WORK}/fake.zip"
chmod 0644 "${WORK}/fake.zip"
if in_session "AMETHYSTORA_CAPTIONS_URL=file://${WORK}/fake.zip ${CAPTIONS} fetch fr" >/dev/null 2>&1; then
    not_ok "C2 a download that is not the French model was taken"
else
    ok "C2 a download that is not the French model is refused"
fi
same "C2 ...and nothing is kept" "$(ls -A "${HOME_DIR}/.local/share/amethystora/captions" 2>/dev/null)" ""
# The French model: the lab's copy (/var/tmp/vmlab-captions-fr.zip, if it keeps one) or alphacephei.com
FR_ZIP="${VMLAB_CAPTIONS_FR_ZIP:-/var/tmp/vmlab-captions-fr.zip}"
if [[ -s "${FR_ZIP}" ]]; then
    install -m 0644 "${FR_ZIP}" "${WORK}/fr.zip"
    fetch_fr="AMETHYSTORA_CAPTIONS_URL=file://${WORK}/fr.zip ${CAPTIONS} fetch fr"
elif curl -sfI --max-time 15 https://alphacephei.com/vosk/models/vosk-model-small-fr-0.22.zip >/dev/null; then
    fetch_fr="${CAPTIONS} fetch fr"
else
    fetch_fr=""
fi
if [[ -n "${fetch_fr}" ]]; then
    if in_session "${fetch_fr}" >"${WORK}/fetch.out" 2>&1; then ok "C2 the French model is fetched by its checksum"; else not_ok "C2 the French model could not be fetched" "$(tail -n 5 "${WORK}/fetch.out")"; fi
    same "C2 ...into captions/model-fr" "$(ls -A "${HOME_DIR}/.local/share/amethystora/captions")" model-fr
    in_session "ame desktop captions language fr" >/dev/null 2>&1
    same "C3 ame desktop captions language fr, with the model here, asks nothing" "$(gs get "${CAPTIONS_SCHEMA}" language)" "'fr'"
    same "C3 the module follows the language" "$(cstate | jq -r .language)" fr
    same "C4 what the machine sends names the language" \
        "$(user_bus /usr/libexec/amethystora-sends --json 2>/dev/null | jq -r '.[] | select(.id == "sends-captions") | .text' | grep -o 'chosen: fr')" "chosen: fr"
    same "C3 Control offers French at once, Spanish in a terminal" \
        "$(user_bus /usr/libexec/amethystora-control-list --json 2>/dev/null | jq -r '.items[] | select(.id == "captions-language") |
            [.state, (.choices | map(select(.value == "fr" or .value == "es")) | map("\(.value):\(.terminal)") | join(","))] | join(" ")')" \
        "fr fr:false,es:true"
    in_session "ame desktop captions language en" >/dev/null 2>&1 </dev/null
else
    skip "C2-C4 need the French model: the lab has no copy and this machine cannot reach alphacephei.com"
fi

# === D. Dictation ======================================================================================================

same "D5 dictation is off as the image ships" "$(gs get "${CAPTIONS_SCHEMA}" dictation)" false
same "D5 ...and its key is not taken" "$(cstate | jq -c '[.dictation.bound, .dictation.helper]')" "[false,null]"
# English, the language dictation hears: the lab's copy of the captions' model, or alphacephei.com
EN_ZIP="${VMLAB_CAPTIONS_ZIP:-/var/tmp/vmlab-captions.zip}"
if [[ -s "${EN_ZIP}" ]]; then
    install -m 0644 "${EN_ZIP}" "${WORK}/en.zip"
    in_session "AMETHYSTORA_CAPTIONS_URL=file://${WORK}/en.zip ${CAPTIONS} fetch" >/dev/null 2>&1
elif curl -sfI --max-time 15 https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip >/dev/null; then
    in_session "${CAPTIONS} fetch" >/dev/null 2>&1
fi
if [[ "$(in_session "${CAPTIONS} status en" 2>/dev/null)" != ready ]]; then
    skip "D6-D9 need the English model: the lab has no copy and this machine cannot reach alphacephei.com"
else
    in_session "ame desktop dictation on" >/dev/null 2>&1 </dev/null
    same "D6 ame desktop dictation on, with the model here" "$(gs get "${CAPTIONS_SCHEMA}" dictation)" true
    bound() { [[ "$(cstate | jq -r .dictation.bound)" == true ]]; }
    check "D6 its key is bound" wait_for 5 bound
    # A microphone of the lab's own: a null sink whose monitor is the default input, fed a sentence espeak-ng says
    MIC_MODULES="$(in_session "pactl load-module module-null-sink sink_name=vmlab69 sink_properties=device.description=vmlab69")"
    MIC_MODULES+=" $(in_session "pactl load-module module-remap-source master=vmlab69.monitor source_name=vmlab69_mic")"
    in_session "pactl set-default-source vmlab69_mic" >/dev/null 2>&1
    espeak-ng -s 140 -w "${WORK}/own/sentence.wav" "buy bread and call the garage" 2>/dev/null
    chown "${TEST_USER}:" "${WORK}/own/sentence.wav"
    fields_up
    if wait_for 30 fields_here; then
        focus_field plain
        ev "${MODULE}.get('captions')._dictation.toggle(); 1" >/dev/null
        listening() { [[ "$(cstate | jq -r '.dictation.helper | tostring')" =~ ^[0-9]+$ && "$(cstate | jq -r .dictation.pill)" == true ]]; }
        check "D6 the key starts the recognizer on the microphone, and the pill shows" wait_for 10 listening
        check "D6 ...as amethystora-captions dictate en" pgrep -u "${TEST_USER}" -f "amethystora-captions dictate en"
        sleep 3
        in_session "paplay --device=vmlab69 ${WORK}/own/sentence.wav" >/dev/null 2>&1
        sleep 2
        ev "${MODULE}.get('captions')._dictation.toggle(); 1" >/dev/null
        typed() { [[ "$(field_text plain)" == *garage* ]]; }
        if wait_for 15 typed; then ok "D7 the sentence is typed into the field: $(field_text plain)"; else not_ok "D7 nothing reached the field" "$(cat "${WORK}/own/fields.log" 2>/dev/null)"; fi
        ended() { [[ "$(cstate | jq -r '.dictation.helper')" == null ]]; }
        check "D7 the recognizer ends after the second press" wait_for 10 ended
        same "D8 nothing was written in the captions folder but the model" "$(ls -A "${HOME_DIR}/.local/share/amethystora/captions")" model
        # A password field is never typed into: refused before anything listens
        focus_field password
        ev "${MODULE}.get('captions')._dictation.toggle(); 1" >/dev/null
        sleep 2
        same "D8 a password field refuses dictation" "$(cstate | jq -r '.dictation.helper')" null
        # The focus leaving the window stops it
        focus_field plain
        ev "${MODULE}.get('captions')._dictation.toggle(); 1" >/dev/null
        wait_for 10 listening
        ev "Main.overview.show(); 1" >/dev/null
        ev "global.display.focus_window?.minimize?.(); 1" >/dev/null
        check "D8 the focus leaving the window ends it" wait_for 10 ended
        ev "Main.overview.hide(); 1" >/dev/null
    else
        not_ok "D6 the test window did not come up"
    fi
    in_session "ame desktop dictation off" >/dev/null 2>&1
    unbound() { [[ "$(cstate | jq -r .dictation.bound)" == false ]]; }
    check "D9 off again, the key is let go" wait_for 5 unbound
    if pgrep -u "${TEST_USER}" -f "amethystora-captions dictate" >/dev/null; then not_ok "D9 ...but a recognizer still runs"; else ok "D9 ...and no recognizer runs"; fi
    user_bus systemctl --user stop vmlab-69-fields.service >/dev/null 2>&1
fi

# === B. Bookmarks in search ============================================================================================

same "B12 off as the image ships, no provider" "$(gs get "${SEARCH_SCHEMA}" bookmarks) $(ask_provider bookmarks fedora)" "false none"
PROFILE="${HOME_DIR}/.mozilla/firefox/vmlab69.default-release"
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${HOME_DIR}/.mozilla" "${HOME_DIR}/.mozilla/firefox" "${PROFILE}"
printf '[Profile0]\nName=default-release\nIsRelative=1\nPath=vmlab69.default-release\nDefault=1\n' >"${HOME_DIR}/.mozilla/firefox/profiles.ini"
python3 - "${PROFILE}/places.sqlite" <<'EOF'
import sqlite3, sys
db = sqlite3.connect(sys.argv[1])
db.executescript('''
CREATE TABLE moz_places (id INTEGER PRIMARY KEY, url LONGVARCHAR, title LONGVARCHAR, visit_count INTEGER);
CREATE TABLE moz_bookmarks (id INTEGER PRIMARY KEY, type INTEGER, fk INTEGER, parent INTEGER, title LONGVARCHAR, lastModified INTEGER);
CREATE TABLE moz_historyvisits (id INTEGER PRIMARY KEY, place_id INTEGER);
INSERT INTO moz_places VALUES (1, 'https://docs.fedoraproject.org/', 'Fedora Docs', 3), (2, 'https://fedora-visited.example/', 'Fedora visited only', 9);
INSERT INTO moz_bookmarks VALUES (1, 2, NULL, 0, 'menu', 1), (2, 2, NULL, 1, 'Work', 2), (10, 1, 1, 2, 'Fedora Docs', 5);
INSERT INTO moz_historyvisits VALUES (1, 2);
''')
db.commit()
EOF
chown -R "${TEST_USER}:" "${HOME_DIR}/.mozilla"
PLACES_SUM="$(sha256sum <"${PROFILE}/places.sqlite")"
in_session "ame desktop search bookmarks on" >/dev/null 2>&1
sleep 1
found="$(ask_provider bookmarks fedora)"
same "B10 the bookmark answers, in its folder" "$(jq -r 'map(.name + " | " + .description) | join(";")' <<<"${found}")" 'Fedora Docs | bookmark · in “Work”'
if grep -q "visited" <<<"${found}"; then not_ok "B10 a page only visited answered"; else ok "B10 a page only visited never answers"; fi
same "B11 no copy is left in the account's runtime folder" "$(ls -d /run/user/"$(id -u "${TEST_USER}")"/amethystora-bookmarks-* 2>/dev/null | wc -l)" 0
same "B11 Firefox's file is as it was" "$(sha256sum <"${PROFILE}/places.sqlite")" "${PLACES_SUM}"
in_session "ame desktop search bookmarks off" >/dev/null 2>&1
same "B12 off again, no provider" "$(ask_provider bookmarks fedora)" none

# === N. Notes in search ================================================================================================

notes_pids() { pgrep -u "${TEST_USER}" -f '[/]usr/lib/amethystora-notes/amethystora-notes'; }
notes_gone() { [[ -z "$(notes_pids)" ]]; }
notes_stop() { pkill -u "${TEST_USER}" -f '[/]usr/lib/amethystora-notes/amethystora-notes' 2>/dev/null; wait_for 20 notes_gone; }
notes_start() { user_bus systemd-run --user --collect --unit="vmlab-69-notes-${RANDOM}" /usr/bin/amethystora-notes "$@" >/dev/null; }
search_object() { user_bus gdbus introspect --session --dest org.amethystora.Notes --object-path /org/amethystora/Notes/Search 2>/dev/null; }
has_search() { search_object | grep -q 'Find'; }
id32() { head -c16 /dev/urandom | od -An -tx1 | tr -d ' \n'; }
notes_stop
rm -rf "${NOTES_DIR}.vmlab69"
[[ -d "${NOTES_DIR}" ]] && mv "${NOTES_DIR}" "${NOTES_DIR}.vmlab69"
install -d -m 0700 -o "${TEST_USER}" -g "${TEST_USER}" "${NOTES_DIR}" "${NOTES_DIR}/resources"
echo '{"format":1,"encrypted":false}' >"${NOTES_DIR}/vault.json"
NB=$(id32) NOTE=$(id32) TASK=$(id32)
printf '{"version":1,"settings":{},"resources":{},"notebooks":{"%s":{"id":"%s","title":"Home","parent":null,"created":1,"updated":1}},"notes":{"%s":{"id":"%s","title":"Car: what the garage said","body":"brakes","notebook":"%s","tags":[],"created":1,"updated":2}},"tags":{},"revisions":{},"projects":{},"sections":{},"tasks":{"%s":{"id":"%s","title":"Call the garage","notes":"","project":null,"section":null,"parent":null,"due":"%s","time":null,"repeat":null,"reminder":null,"priority":4,"labels":[],"pinned":false,"done":null,"created":1,"order":1}},"labels":{}}\n' \
    "${NB}" "${NB}" "${NOTE}" "${NOTE}" "${NB}" "${TASK}" "${TASK}" "$(date +%F)" >"${NOTES_DIR}/data.json"
chown -R "${TEST_USER}:" "${NOTES_DIR}"
in_session "ame desktop search notes on" >/dev/null 2>&1
same "N15 Notes not running: nothing, and nothing starts it" "$(ask_provider notes garage | jq -r 'length') $(notes_pids | wc -l)" "0 0"
notes_start
check "N13 open, Notes has its search object" wait_for 30 has_search
found="$(ask_provider notes garage)"
same "N13 the note and the task answer, by title" "$(jq -r 'map(.name + " | " + .description) | join(";")' <<<"${found}")" \
    "Car: what the garage said | Notes · Home;Call the garage | Notes · task · $(date +%F)"
reply="$(user_bus gdbus call --session --dest org.amethystora.Notes --object-path /org/amethystora/Notes/Search \
    --method org.amethystora.Notes.Search1.Find garage 2>&1)"
if [[ "${reply}" == *garage* ]]; then not_ok "N14 a caller that is not the shell got titles" "${reply}"; else ok "N14 a caller that is not the shell is refused"; fi
ev "${MODULE}.get('search')._providers.get('notes').activateResult('notes:note:${NOTE}'); 1" >/dev/null
notes_front() { [[ "$(ev "global.display.focus_window?.get_wm_class()")" == org.amethystora.Notes ]]; }
check "N13 Enter shows the note in Notes" wait_for 10 notes_front
notes_stop
same "N15 Notes closed: the search says nothing" "$(ask_provider notes garage | jq -r 'length')" 0
in_session "ame desktop search notes off" >/dev/null 2>&1
rm -rf "${NOTES_DIR}"
[[ -d "${NOTES_DIR}.vmlab69" ]] && mv "${NOTES_DIR}.vmlab69" "${NOTES_DIR}"

# === M. Files by meaning ===============================================================================================

same "M19 off as the image ships" "$(gs get "${SEARCH_SCHEMA}" meaning) $(user_bus systemctl --user is-enabled amethystora-file-meaning.timer 2>/dev/null)" "false disabled"
DOCS="$(in_session 'xdg-user-dir DOCUMENTS' 2>/dev/null)"
[[ -n "${DOCS}" && "${DOCS}" != "${HOME_DIR}" ]] || DOCS="${HOME_DIR}/Documents"
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${DOCS}/Taxes"
echo "Dear customer, the refund of your income tax for 2025 has been paid into your bank account." >"${DOCS}/Taxes/refund-2025.txt"
echo "Plant the roses in spring, water the garden twice a week, and cut the hedge in June." >"${DOCS}/garden.txt"
chown -R "${TEST_USER}:" "${DOCS}"
in_session "localsearch index --add ${DOCS}" >/dev/null 2>&1 || true
indexed() { in_session "/usr/libexec/amethystora-file-meaning texts" 2>/dev/null | grep -q refund-2025; }
if ! wait_for 120 indexed; then
    skip "M16-M18 need localsearch to have read the planted files, and it did not in two minutes"
else
    in_session "ame desktop search meaning on" >/dev/null 2>&1
    built() { test -s "${HOME_DIR}/.local/state/amethystora/file-meaning/files.jsonl"; }
    check "M16 turning it on reads Documents' text once" wait_for 60 built
    found="$(ask_provider meaning letter about money back from taxes)"
    same "M16 a paraphrase with no word of the file finds it" "$(jq -r '.[0].name' <<<"${found}")" refund-2025.txt
    INDEX="${HOME_DIR}/.local/state/amethystora/file-meaning"
    same "M17 the index is 0700, its files 0600" "$(stat -c %a "${INDEX}" "${INDEX}/files.jsonl" "${INDEX}/vectors.f16" | paste -sd ' ')" "700 600 600"
    if grep -q "refund of your\|roses" "${INDEX}/files.jsonl" "${INDEX}/vectors.f16"; then not_ok "M17 the index holds words of the files"; else ok "M17 the index holds paths and vectors, no text"; fi
    same "M18 the reading waits for mains power" "$(user_bus systemctl --user show -p ConditionACPower --value amethystora-file-meaning.service 2>/dev/null; grep -c '^ConditionACPower=true$' /usr/lib/systemd/user/amethystora-file-meaning.service)" 1
    in_session "ame desktop search meaning off" >/dev/null 2>&1
    same "M18 off: the timer off, the index deleted" "$(user_bus systemctl --user is-enabled amethystora-file-meaning.timer 2>/dev/null) $(test -e "${INDEX}" && echo kept || echo gone)" "disabled gone"
fi
same "M19 off: no provider" "$(ask_provider meaning letter about money back from taxes)" none
rm -rf "${DOCS}/Taxes" "${DOCS}/garden.txt"

# === K. Click with the keyboard ========================================================================================

same "K20 off as the image ships" "$(gs get "${ACCESS_SCHEMA}" letters)" false
same "K20 ...and no key bound" "$(ev "JSON.stringify(${MODULE}.get('access').state().keys)" | jq -r 'index("letters-key") // "none"')" none
in_session "ame desktop access letters on" >/dev/null 2>&1
lbound() { [[ "$(ev "JSON.stringify(${MODULE}.get('access').state().keys)" | jq -r 'index("letters-key") != null')" == true ]]; }
check "K21 on, Super+Alt+K is bound" wait_for 5 lbound
fields_up
if wait_for 30 fields_here; then
    sleep 1
    keys super+alt+k
    letters_state() { ev "JSON.stringify(${MODULE}.get('access').letters?.state() ?? null)"; }
    shown() { [[ "$(letters_state | jq -r '.labels | length')" -ge 3 ]]; }
    if wait_for 10 shown; then
        ok "K21 Super+Alt+K puts letters on the window's controls ($(letters_state | jq -r '.labels | length'))"
        same "K21 ...holding the keyboard" "$(letters_state | jq -r .grabbed)" true
        # Test speakers is the lowest control of the window
        label="$(letters_state | jq -r '.labels | max_by(.y) | .name')"
        keys "text:${label}"
        clicked() { grep -q "^clicked	\"Test speakers\"" "${WORK}/own/fields.log" 2>/dev/null; }
        check "K21 typing its letters clicks Test speakers" wait_for 10 clicked
        gone() { [[ "$(letters_state | jq -r '.labels | length')" == 0 && "$(letters_state | jq -r .grabbed)" == false ]]; }
        check "K22 ...and the letters go" wait_for 5 gone
        keys super+alt+k
        wait_for 10 shown
        keys esc
        check "K22 Escape takes them away" wait_for 5 gone
        no_helper() { ! pgrep -u "${TEST_USER}" -f amethystora-letters >/dev/null; }
        check "K22 ...and ends the program that read the tree" wait_for 5 no_helper
    else
        not_ok "K21 no letters on the test window" "$(letters_state)" "$(journalctl -b --user -M "${TEST_USER}@" -o cat --no-pager 2>/dev/null | grep -i letters | tail -n 5)"
    fi
else
    not_ok "K21 the test window did not come up"
fi
user_bus systemctl --user stop vmlab-69-fields.service >/dev/null 2>&1
for i in $(seq 1 10); do
    user_bus gnome-extensions disable "${UUID}" >/dev/null 2>&1
    user_bus gnome-extensions enable "${UUID}" >/dev/null 2>&1
done
wait_for 30 ready
same "K23 ten disable/enable cycles leave no letters, pill or grab" \
    "$(ev "Main.uiGroup.get_children().filter(a => /amethystora-(letters|dictation)/.test(a.style_class ?? '')).length + ':' + Main.modalCount")" "0:0"
in_session "ame desktop access letters off" >/dev/null 2>&1

# === G. The guest login ================================================================================================

if /usr/libexec/amethystora-hardening is-on guest-login; then not_ok "G24 the guest login is on as the image ships"; else ok "G24 the guest login is off as the image ships"; fi
if getent passwd amethystora-guest >/dev/null; then not_ok "G24 ...but its account exists"; else ok "G24 ...and there is no guest account"; fi
same "G24 Security's settings list it, under logins, off" \
    "$(/usr/libexec/amethystora-hardening status --json | jq -r '.[] | select(.name == "guest-login") | "\(.group) \(.state) \(.loosens)"')" "logins off true"
if /usr/libexec/amethystora-hardening set guest-login on >"${WORK}/guest.out" 2>&1; then ok "G25 turned on"; else not_ok "G25 it would not turn on" "$(cat "${WORK}/guest.out")"; fi
GUEST_UID="$(id -u amethystora-guest 2>/dev/null)"
same "G25 the account, standard and without a password" \
    "$(id -nG amethystora-guest 2>/dev/null) $(passwd -S amethystora-guest 2>/dev/null | awk '{ print $2 }')" "amethystora-guest NP"
same "G25 the PAM feature is on" "$(grep -cx with-guest-login /etc/authselect/authselect.conf)" 1
if command -v pamtester >/dev/null; then
    # As root, as login and sshd ask; su from another account, since root itself may become anyone
    for service in login sshd sudo su; do
        as=(env)
        [[ "${service}" == su ]] && as=(runuser -u "${TEST_USER}" --)
        if "${as[@]}" pamtester "${service}" amethystora-guest authenticate </dev/null >/dev/null 2>&1; then
            not_ok "G25 ${service} let the guest in without a password"
        else
            ok "G25 ${service} refuses the guest"
        fi
    done
    if pamtester gdm-password amethystora-guest authenticate </dev/null >/dev/null 2>&1; then ok "G25 the login screen lets the guest in"; else not_ok "G25 the login screen refused the guest"; fi
else
    for service in login sshd sudo; do
        if [[ "$(grep -c 'user != amethystora-guest' /etc/authselect/system-auth /etc/authselect/password-auth | paste -sd ' ')" == \
            "/etc/authselect/system-auth:1 /etc/authselect/password-auth:1" ]]; then ok "G25 ${service}: the guest's refusal is in its PAM stack"; else not_ok "G25 ${service}: no refusal in PAM"; fi
    done
fi
grep -qx "DenyUsers amethystora-guest" /etc/ssh/sshd_config.d/60-amethystora-guest.conf && ok "G25 sshd denies the guest by name" || not_ok "G25 sshd does not deny the guest"
# A session of the guest's own (systemd's user manager, as a login starts it): the home is a tmpfs, and a file written
# there is gone after
systemctl start "user@${GUEST_UID}.service"
same "G26 the guest's home is a tmpfs while its user manager runs" "$(findmnt -n -o FSTYPE /var/home/amethystora-guest)" tmpfs
same "G26 ...answered as GNOME's first run" "$(cat /var/home/amethystora-guest/.config/gnome-initial-setup-done 2>/dev/null)" yes
runuser -u amethystora-guest -- sh -c 'echo secret >/var/home/amethystora-guest/left.txt; echo t >/tmp/vmlab69-guest-left'
systemctl stop "user@${GUEST_UID}.service"
unmounted() { ! mountpoint -q /var/home/amethystora-guest; }
check "G26 at logout the home is unmounted" wait_for 15 unmounted
same "G26 ...and what the guest left is gone" "$(ls -A /var/home/amethystora-guest 2>/dev/null | wc -l) $(test -e /tmp/vmlab69-guest-left && echo left || echo gone)" "0 gone"
/usr/libexec/amethystora-security-watch >/dev/null 2>&1
same "G27 the watcher finds no drift in the switch's files" \
    "$(jq -r '[.drift[]? | select(.path | test("guest|authselect"))] | length' /var/lib/amethystora/security/status.json 2>/dev/null)" 0
same "G27 the report says the guest login is on" \
    "$(/usr/libexec/amethystora-security-status --json 2>/dev/null | jq -r '.checks[] | select(.id == "guest-login") | "\(.group) \(.state)"')" "optional on"
/usr/libexec/amethystora-hardening set guest-login off >/dev/null 2>&1
same "G28 off: no account, no feature, no files" \
    "$(getent passwd amethystora-guest | wc -l) $(grep -cx with-guest-login /etc/authselect/authselect.conf) $(ls /etc/systemd/logind.conf.d/60-amethystora-guest.conf /etc/ssh/sshd_config.d/60-amethystora-guest.conf 2>/dev/null | wc -l)" \
    "0 0 0"

finish
