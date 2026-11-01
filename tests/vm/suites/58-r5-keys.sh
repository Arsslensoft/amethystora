#!/usr/bin/bash
# vmlab-vm: r5-keys
# Round 5/6's keys group (plans/ui-ideas/spec-keys.md, acceptance criteria 1 to 12), in the test account's session, asked
# of GNOME Shell itself through Eval, which a lab-only extension of the test account opens (never in the image), with a
# real keyboard of keys.py's and a small GTK window of the lab's whose text is written to a file:
#   - the keys module and its keys, accents off (1);
#   - every shortcut in one list, Super+Alt+V once; a changed one clashing with the tiling's, and Reset (2, 3);
#   - set refuses a bare letter, a schema or key not listed, a malformed combination (4);
#   - Control's Keyboard page: the card, its search, recording a combination by pressing it (5);
#   - the keyboard settings, each GNOME key, the xkb options kept but the one changed, and Control's states (6, 7);
#   - Super alone opening the search box, nothing, or the overview (8);
#   - Super+Alt+V typing the clipboard's text into the window in use (9);
#   - hold a key for accents: the strip, a pick, off repeating as before (10);
#   - ten disables and enables leave no wrapper of ours (11); the setup file (12).
# vmlab-paths: @ui/extension.js @ui/modules/keys.js @ui/modules/typing.js @ui/modules/typing/* @ui/modules/emoji/paste.js @ui/schemas/*keys* @ui/schemas/*typing* @ui/stylesheets/typing.css @sf/usr/libexec/amethystora-keybindings @sf/usr/libexec/amethystora-control-list @sf/usr/libexec/amethystora-setup* @sf/*just/desktop.just apps/control/* apps/control-core/* apps/agent-tools/src/never.rs
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@iarsslen.github.io
HOME_DIR="/var/home/${TEST_USER}"
HERE="$(dirname "$(readlink -f "$0")")"
WORK=/run/vmlab-58
KEYS=org.gnome.shell.extensions.amethystora-ui.keys
TYPING=org.gnome.shell.extensions.amethystora-ui.typing
INPUT=org.gnome.desktop.input-sources
KEYBOARD=org.gnome.desktop.peripherals.keyboard
MEDIA=org.gnome.settings-daemon.plugins.media-keys
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
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
KM="${UI}.get('keys')"
TM="${UI}.get('typing')"
# One field of the keys module's or the typing module's state, as JSON
keys_st() { ev "JSON.stringify(JSON.parse(${KM}.snapshot())$1)"; }
typing_st() { ev "JSON.stringify(JSON.parse(${TM}.snapshot())$1)"; }
press() { python3 "${HERE}/keys.py" "$@" >/dev/null 2>&1; }
list_json() { in_session "/usr/libexec/amethystora-keybindings list --json"; }
# The row of a shortcut in the helper's list, as a jq filter of it
row() { list_json | jq -c --arg s "$1" --arg k "$2" ".bindings[] | select(.schema == \$s and .key == \$k) | $3"; }
typed() { cat "${WORK}/text.txt" 2>/dev/null; }
# For wait_for, which runs a command again each round
typed_is() { [[ "$(typed)" == "$1" ]]; }
key_is() { [[ "$(gs get "$1" "$2")" == "$3" ]]; }
accents_wrapped() { [[ "$(typing_st '.accents.wrapped')" == true ]]; }

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
install -d -m 0755 -o "${TEST_USER}" -g "${TEST_USER}" "${WORK}"
# A text view whose text is written to a file at every change: what a person typed, and what was pasted or picked
cat >"${WORK}/text.js" <<'EOF'
imports.gi.versions.Gtk = '4.0';
const {GLib, Gtk} = imports.gi;
const out = ARGV[0];
const app = new Gtk.Application({application_id: 'org.amethystora.VmlabKeys'});
app.connect('activate', () => {
    const win = new Gtk.ApplicationWindow({application: app, title: 'Keys lab', default_width: 700, default_height: 320});
    const view = new Gtk.TextView({top_margin: 24, left_margin: 24});
    view.buffer.connect('changed', b => GLib.file_set_contents(out, b.get_text(b.get_start_iter(), b.get_end_iter(), false)));
    win.set_child(view);
    win.present();
    view.grab_focus();
});
app.run([]);
EOF
chown -R "${TEST_USER}:" "${HOME_DIR}/.local/share/gnome-shell" "${WORK}"
enabled="$(user_bus gsettings get org.gnome.shell enabled-extensions)"
[[ ${enabled} == *vmlab-eval@lab* ]] || user_bus gsettings set org.gnome.shell enabled-extensions "${enabled%]*}, 'vmlab-eval@lab']"
for key in plain-paste super-alone; do gs reset "${KEYS}" "${key}"; done
gs reset "${TYPING}" accents
loginctl unlock-sessions
systemctl restart gdm
keys_ready() { [[ "$(ev "typeof ${KM}?.snapshot")" == function ]]; }
if ! wait_for 240 has_session || ! wait_for 120 keys_ready; then
    not_ok "the keys module did not start in the session"
    finish
fi
loginctl unlock-sessions
sleep 3
# Out of the overview a session starts in
press click:0.5,0.5 esc

# --- 1: the module and its keys --------------------------------------------------------------------------------------
same "1 Super+Alt+V pastes as plain text" "$(gs get "${KEYS}" plain-paste)" "['<Super><Alt>v']"
same "1 Super alone opens the overview" "$(gs get "${KEYS}" super-alone)" "'overview'"
same "1 holding a key for accents is off" "$(gs get "${TYPING}" accents)" "false"
same "1 the key is bound, nothing wrapped" "$(keys_st '| [.bound, .wrapped] | join(" ")')" "true false"
same "1 accents off wraps nothing" "$(typing_st '.accents | [.on, .wrapped] | join(" ")')" "false false"

# --- 2, 3, 4: every shortcut in one list ------------------------------------------------------------------------------
LIST="$(list_json)"
[[ "$(jq '[.bindings[] | select(.owner == "tiling")] | length' <<<"${LIST}")" == 100 &&
    "$(jq '[.bindings[] | select(.owner == "gnome")] | length' <<<"${LIST}")" -gt 100 &&
    "$(jq '[.bindings[] | select(.owner == "desktop")] | length' <<<"${LIST}")" -gt 20 ]] &&
    ok "2 the list holds GNOME's, the desktop's, the tiling's 100 and the custom shortcuts" ||
    not_ok "2 the list is short" "$(jq -c '[.bindings[] | .owner] | group_by(.) | map({(.[0]): length}) | add' <<<"${LIST}")"
same "2 Super+Alt+V is bound once" "$(jq '[.bindings[] | .value[] | ascii_downcase
    | select(. == "<super><alt>v" or . == "<alt><super>v")] | length' <<<"${LIST}")" "1"
same "2 nothing clashes as the image ships it" "$(jq '[.bindings[] | select(.clash != [])] | length' <<<"${LIST}")" "0"
same "2 the agent's launcher is listed" "$(jq -r '.bindings[] | select(.schema == "custom" and .key == "custom27") | .owner' <<<"${LIST}")" \
    "desktop"
gs set org.gnome.shell.extensions.amethystora-ui.tiling enabled true
in_session "ame desktop shortcut set ${MEDIA} previous '<Super>Left'" >/dev/null 2>&1
same "3 a shortcut changed is the account's" "$(row "${MEDIA}" previous '.changed')" "true"
same "3 ...and clashes with the tiling's" "$(row "${MEDIA}" previous '[.clash[] | .owner] | unique | join(" ")')" "tiling"
same "3 ...written into GNOME's own key" "$(gs get "${MEDIA}" previous)" "['<Super>Left']"
in_session "ame desktop shortcut reset ${MEDIA} previous" >/dev/null 2>&1
same "3 Reset puts it back, with no clash" "$(row "${MEDIA}" previous '[.changed, (.clash | length)] | map(tostring) | join(" ")')" \
    "false 0"
refused() { ! in_session "/usr/libexec/amethystora-keybindings $*" >/dev/null 2>&1; }
check "4 a bare letter is no shortcut" refused set "${MEDIA}" previous a
check "4 Shift and a letter is none either" refused set "${MEDIA}" previous "'<Shift>a'"
check "4 a schema it does not list is refused" refused set org.gnome.desktop.interface font-name "'<Super>a'"
check "4 a key of a list that is no shortcut is refused" refused set "${MEDIA}" custom-keybindings "'<Super>a'"
check "4 a malformed combination is refused" refused set "${MEDIA}" previous "'<Super'"
check "4 a key GTK does not know is refused" refused set "${MEDIA}" previous "'<Super>Nokey'"
check "4 a function key alone is one" in_session "/usr/libexec/amethystora-keybindings set ${MEDIA} previous F9"
in_session "ame desktop shortcut reset ${MEDIA} previous" >/dev/null 2>&1

# --- 5: Control's Keyboard page -------------------------------------------------------------------------------------
in_session "setsid amethystora-control keyboard >/dev/null 2>&1 &" >/dev/null 2>&1
if user_bus python3 "${HERE}/atspi.py" amethystora-control wait 40 "Every shortcut" >/dev/null 2>&1; then
    ok "5 Control opens on the Keyboard page"
    user_bus python3 "${HERE}/atspi.py" amethystora-control type "Previous track" "Find a shortcut" >/dev/null 2>&1
    check "5 its search finds Previous track" user_bus python3 "${HERE}/atspi.py" amethystora-control has "Change the shortcut Previous track"
    user_bus python3 "${HERE}/atspi.py" amethystora-control click "Change the shortcut Previous track" >/dev/null 2>&1
    sleep 1
    # GNOME asks once whether an app may hold the shortcuts back while it listens
    user_bus python3 "${HERE}/atspi.py" gnome-shell click Allow >/dev/null 2>&1 && sleep 1
    if user_bus python3 "${HERE}/atspi.py" amethystora-control has "Press the new keys for" >/dev/null 2>&1; then
        ok "5 Change waits for the new keys"
        press super+alt+comma
        wait_for 15 key_is "${MEDIA}" previous "['<Super><Alt>comma']"
        same "5 the keys pressed are the shortcut's" "$(gs get "${MEDIA}" previous)" "['<Super><Alt>comma']"
        if user_bus python3 "${HERE}/atspi.py" amethystora-control wait 15 "Reset the shortcut Previous track" >/dev/null 2>&1; then
            user_bus python3 "${HERE}/atspi.py" amethystora-control click "Reset the shortcut Previous track" >/dev/null 2>&1
            sleep 3
            same "5 Reset on the page puts it back" "$(row "${MEDIA}" previous '.changed')" "false"
        else
            not_ok "5 no Reset on a changed shortcut"
        fi
    else
        not_ok "5 Change did not wait for keys" "$(user_bus python3 "${HERE}/atspi.py" amethystora-control text 2>&1 | head -n 30)"
    fi
    in_session "pkill -f amethystora-control" >/dev/null 2>&1
else
    not_ok "5 Control did not show the Keyboard page" "$(user_bus python3 "${HERE}/atspi.py" amethystora-control text 2>&1 | head -n 20)"
fi
in_session "ame desktop shortcut reset ${MEDIA} previous" >/dev/null 2>&1

# --- 6, 7: the keyboard settings --------------------------------------------------------------------------------------
gs set "${INPUT}" xkb-options "\"['grp:alt_shift_toggle']\""
in_session "ame desktop keyboard caps escape" >/dev/null 2>&1
same "6 Caps Lock as Escape keeps the other option" "$(gs get "${INPUT}" xkb-options)" "['grp:alt_shift_toggle', 'caps:escape']"
in_session "ame desktop keyboard caps ctrl" >/dev/null 2>&1
same "6 ...and replaces itself" "$(gs get "${INPUT}" xkb-options)" "['grp:alt_shift_toggle', 'ctrl:nocaps']"
in_session "ame desktop keyboard compose ralt" >/dev/null 2>&1
same "7 the Compose key beside it" "$(gs get "${INPUT}" xkb-options)" "['grp:alt_shift_toggle', 'ctrl:nocaps', 'compose:ralt']"
STATES="$(user_bus /usr/libexec/amethystora-control-list --json 2>/dev/null | jq -r '[.items[] | select(.page == "keyboard" and .kind != "keybindings") | "\(.id)=\(.state)"] | join(" ")')"
[[ "${STATES}" == *"keyboard-caps=ctrl"* && "${STATES}" == *"keyboard-compose=ralt"* ]] &&
    ok "7 Control says Caps Lock and Compose as set" || not_ok "7 Control's states" "${STATES}"
in_session "ame desktop keyboard caps normal" >/dev/null 2>&1
in_session "ame desktop keyboard compose off" >/dev/null 2>&1
same "6 back to normal leaves the other option alone" "$(gs get "${INPUT}" xkb-options)" "['grp:alt_shift_toggle']"
in_session "ame desktop keyboard middle-paste off" >/dev/null 2>&1
same "7 middle-click paste off" "$(gs get org.gnome.desktop.interface gtk-enable-primary-paste)" "false"
in_session "ame desktop keyboard numlock on" >/dev/null 2>&1
same "7 Num Lock on at every start" "$(gs get "${KEYBOARD}" remember-numlock-state) $(gs get "${KEYBOARD}" numlock-state)" "false true"
in_session "ame desktop keyboard repeat-delay 400" >/dev/null 2>&1
same "7 the repeat delay" "$(gs get "${KEYBOARD}" delay)" "uint32 400"
if in_session "ame desktop keyboard repeat-delay 5000" >/dev/null 2>&1; then
    not_ok "7 a delay out of its range was taken"
else
    ok "7 a delay out of its range is refused"
fi
in_session "ame desktop keyboard per-window on" >/dev/null 2>&1
same "7 each window keeps its layout" "$(gs get "${INPUT}" per-window)" "true"
STATES="$(user_bus /usr/libexec/amethystora-control-list --json 2>/dev/null | jq -r '[.items[] | select(.page == "keyboard" and .kind != "keybindings") | "\(.id)=\(.state)"] | join(" ")')"
same "7 Control says each as set" "${STATES}" "keyboard-caps=normal keyboard-compose=off keyboard-super=overview keyboard-middle-paste=off keyboard-numlock=on keyboard-repeat-delay=400 keyboard-per-window=on keyboard-accents=off"
for key in remember-numlock-state numlock-state delay; do gs reset "${KEYBOARD}" "${key}"; done
gs reset "${INPUT}" per-window
gs reset "${INPUT}" xkb-options

# --- 8: Super alone ---------------------------------------------------------------------------------------------------
search_open() { [[ "$(ev "String(!!${UI}.get('search')._open)")" == true ]]; }
overview_open() { [[ "$(ev "String(Main.overview.visible)")" == true ]]; }
in_session "ame desktop keyboard super search" >/dev/null 2>&1
same "8 search wraps the overview's toggle" "$(keys_st '| [.mode, .toggleIsOurs] | map(tostring) | join(" ")')" "search true"
press super
if wait_for 6 search_open && ! overview_open; then ok "8 Super alone opens the search box"; else
    not_ok "8 Super alone did not open the search box" "$(keys_st '')"; fi
press esc
sleep 1
in_session "ame desktop keyboard super nothing" >/dev/null 2>&1
press super
sleep 2
if ! search_open && ! overview_open; then ok "8 Super alone opens nothing"; else not_ok "8 Super alone opened something"; fi
ev "Main.overview.toggle(); 1" >/dev/null
if wait_for 6 overview_open; then ok "8 the overview still opens another way"; else not_ok "8 the overview's toggle is lost"; fi
ev "Main.overview.hide(); 1" >/dev/null
sleep 1
in_session "ame desktop keyboard super overview" >/dev/null 2>&1
same "8 overview wraps nothing" "$(keys_st '.wrapped')" "false"
press super
if wait_for 6 overview_open; then ok "8 Super alone opens the overview, as GNOME's"; else not_ok "8 Super alone no longer opens the overview"; fi
ev "Main.overview.hide(); 1" >/dev/null
sleep 1

# --- 9: paste as plain text -------------------------------------------------------------------------------------------
lab_window() { [[ "$(ev "String(global.display.focus_window?.title === 'Keys lab')")" == true ]]; }
start_lab() {
    rm -f "${WORK}/text.txt"
    in_session "setsid gjs ${WORK}/text.js ${WORK}/text.txt >/dev/null 2>&1 &" >/dev/null 2>&1
    wait_for 20 lab_window
}
if start_lab; then
    ev "imports.gi.St.Clipboard.get_default().set_text(imports.gi.St.ClipboardType.CLIPBOARD, 'Results for the third quarter'); 1" >/dev/null
    sleep 1
    press super+alt+v
    wait_for 8 typed_is "Results for the third quarter"
    same "9 Super+Alt+V types the clipboard's text in" "$(typed)" "Results for the third quarter"
    same "9 ...counted as typed, never as anything else" "$(keys_st '.pastes | [.asked, .typed] | map(tostring) | join(" ")')" "1 1"
    if journalctl -b --since "-5 min" --no-pager -o cat 2>/dev/null | grep -q "third quarter"; then
        not_ok "9 the pasted text reached the journal"
    else
        ok "9 the pasted text is in no log"
    fi

    # --- 10: hold a key for accents -------------------------------------------------------------------------------------
    in_session "ame desktop accents on" >/dev/null 2>&1
    wait_for 5 accents_wrapped
    same "10 on, the input method is wrapped" "$(typing_st '.accents.wrapped')" "true"
    press space c a f
    # One keyboard for the whole hold: keys.py's goes away, and its keys up, when it ends
    press down:e sleep:1300 up:e
    same "10 holding e shows the strip, which stays once the e is let go" "$(typing_st '.accents.open')" "true"
    same "10 ...and only one e reached the app" "$(typed)" "Results for the third quarter cafe"
    press 2
    wait_for 5 typed_is "Results for the third quarter cafè"
    same "10 2 types è in place of the e" "$(typed)" "Results for the third quarter cafè"
    press space down:e sleep:1300 up:e esc
    sleep 1
    same "10 Escape leaves the letter as it was" "$(typed)" "Results for the third quarter cafè e"
    in_session "ame desktop accents off" >/dev/null 2>&1
    sleep 1
    press space down:e sleep:1500 up:e
    sleep 1
    after="$(typed)"
    [[ "${after}" == "Results for the third quarter cafè e eee"* ]] && ok "10 off, a held key repeats as before" ||
        not_ok "10 off, a held key did not repeat" "${after}"
    same "10 off, nothing is wrapped" "$(typing_st '.accents.wrapped')" "false"
    ev "global.get_window_actors().map(a => a.meta_window).filter(w => w.title === 'Keys lab').forEach(w => w.delete(global.get_current_time())); 1" >/dev/null
else
    not_ok "9 the lab's text window did not open"
fi

# --- 11: ten disables and enables -------------------------------------------------------------------------------------
in_session "ame desktop accents on" >/dev/null 2>&1
in_session "ame desktop keyboard super search" >/dev/null 2>&1
sleep 1
for _ in 1 2 3 4 5 6 7 8 9 10; do
    ev "Main.extensionManager.disableExtension('${UUID}'); 1" >/dev/null
    sleep 0.5
    ev "Main.extensionManager.enableExtension('${UUID}'); 1" >/dev/null
    sleep 0.5
done
ev "Main.extensionManager.disableExtension('${UUID}'); 1" >/dev/null
sleep 1
same "11 disabled, nothing of ours is left on the overview or the input method" \
    "$(ev "JSON.stringify([Object.prototype.hasOwnProperty.call(Main.overview, 'toggle'), Object.prototype.hasOwnProperty.call(Main.inputMethod, 'notify_key_event')])")" \
    "[false,false]"
ev "Main.extensionManager.enableExtension('${UUID}'); 1" >/dev/null
wait_for 30 keys_ready
same "11 enabled again, each is back once" "$(keys_st '| [.wrapped, .toggleIsOurs] | map(tostring) | join(" ")') $(typing_st '.accents.wrapped')" \
    "true true true"
in_session "ame desktop accents off" >/dev/null 2>&1

# --- 12: the setup file -----------------------------------------------------------------------------------------------
gs set "${INPUT}" xkb-options "\"['caps:escape']\""
in_session "ame desktop keyboard repeat-delay 350" >/dev/null 2>&1
saved="$(in_session "ame setup save -" 2>/dev/null)"
same "12 the setup file keeps the keyboard" "$(jq -c '[.settings.input, .settings.keyboard, .settings.keys]' <<<"${saved}")" \
    '[{"xkb-options":["caps:escape"]},{"delay":350},{"super-alone":"search"}]'
same "12 ...and never accents" "$(jq -c '.settings | [paths | map(tostring) | join(".") | select(test("accents$") and (test("look") | not))]' <<<"${saved}")" "[]"
printf '%s\n' '{"version": 1, "settings": {"keys": {"super-alone": "dock"}}}' >"${WORK}/bad.json"
chown "${TEST_USER}:" "${WORK}/bad.json"
if in_session "ame setup diff ${WORK}/bad.json" >/dev/null 2>&1; then
    not_ok "12 the setup file took a value Super alone cannot have"
else
    ok "12 the setup file refuses a value Super alone cannot have"
fi
gs reset "${INPUT}" xkb-options
gs reset "${KEYBOARD}" delay
in_session "ame desktop keyboard super overview" >/dev/null 2>&1

check "no JS ERROR from the keys module or the accents" test -z "$(journalctl -b --since "-40 min" --no-pager -o cat 2>/dev/null |
    grep -E 'JS ERROR|Amethystora UI: (accents|the key to paste)' | grep -i 'keys\|accents\|paste')"
finish
