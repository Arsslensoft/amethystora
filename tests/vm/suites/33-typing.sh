#!/usr/bin/bash
# vmlab-vm: typing
# Emoji and symbols (plans/beyond/spec-search.md, section 8, suite 33), in the test account's session, asked of GNOME
# Shell itself through Eval, which a lab-only extension of the test account opens (never in the image), and typed as a
# person types (keys.py):
#   - Super+. is the picker's, Smile keeps Ctrl+Alt+Space, the old key of the image is gone and the hook resets it;
#   - it opens under the cursor of a Text Editor document on Smileys, finds by name, and Enter types the pick;
#   - Recent, the arrow keys, a terminal, the skin tone, Escape, a broken state file, the data let go when closed;
#   - the motion (forced on: the lab draws without a GPU), the card's too, and ten disables and enables.
# Then text shortcuts (plans/beyond/spec-phase7.md, part B, section 8, suite 33, steps T1-T20 below): off by default and
# nothing wrapped, expanding through GNOME's own input method in Text Editor, kitty and a GTK 4 window, never in password,
# PIN or number fields or the shell's own, beside Mozc, not in X11 apps (as documented), the file and its helper, Control's
# Typing group, nothing logged, the cost per key, ten disables and enables, and the lock screen.
# vmlab-paths: @ui/extension.js @ui/*typing* @ui/*emoji* @sf/usr/libexec/amethystora-text-shortcuts @sf/*themed/shell-emoji* @sf/usr/libexec/amethystora-control-list @sf/*just/desktop.just
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HOME_DIR="/var/home/${TEST_USER}"
EMOJI=org.gnome.shell.extensions.amethystora-ui.emoji
A11Y=org.gnome.desktop.a11y.interface
HERE="$(dirname "$(readlink -f "$0")")"
RUN="$RANDOM"
STATE="${HOME_DIR}/.local/state/amethystora/emoji.json"
CUSTOM6=/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom6/
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        sed -E -e "s/^\(true, '(.*)'\)\$/\1/" -e 's/^"(.*)"$/\1/'
}
gs() { in_session "gsettings $*" 2>/dev/null; }
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
keys() { python3 "${HERE}/keys.py" "$@" >/dev/null 2>&1; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
E="${UI}.get('emoji')"
SNAP="/run/user/$(id -u "${TEST_USER}")/vmlab-33-snap.json"
snap() {
    ev "imports.gi.GLib.file_set_contents(${SNAP@Q}, ${E}.snapshot()); 'written'" >/dev/null
    cat "${SNAP}"
}
q() { snap | jq -r "$1" 2>/dev/null; }
is_open() { [[ "$(q .open)" == true ]]; }
is_closed() { [[ "$(q .open)" == false ]]; }
editor_text() { user_bus python3 "${HERE}/atspi.py" gnome-text-editor text 2>/dev/null; }
# soon SECONDS COMMAND...: wait_for, looking every tenth of a second
soon() {
    local tries=$(($1 * 10))
    shift
    until "$@" >/dev/null 2>&1; do
        ((tries-- > 0)) || return 1
        sleep 0.1
    done
}
open_picker() { keys super+period; soon 5 is_open; loaded_now() { [[ "$(q .loaded)" == true ]]; }; soon 5 loaded_now; }

if ! has_session; then
    bash "$(dirname "$0")/10-account.sh" >/dev/null
fi
if ! wait_for 300 has_session; then
    not_ok "no session for ${TEST_USER}"
    finish
fi
gs reset "${EMOJI}" skin-tone
rm -f "${STATE}" "${STATE}.bak"

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

# --- 15 first: an account that kept the image's old Super+. for Smile has it reset by the hook, once ----------------
user_bus dconf write "${CUSTOM6}binding" "'<Super>period'"
user_bus dconf write "${CUSTOM6}command" "'flatpak run it.mijorus.smile'"
# The hook runs once per account (its version in setup_versioning.json): forgotten, it runs as at the update
VERSIONS="${HOME_DIR}/.local/share/amethystora/setup_versioning.json"
if [[ -s "${VERSIONS}" ]]; then
    jq 'del(.version.user["emoji-key"])' "${VERSIONS}" >"${VERSIONS}.vmlab" && mv "${VERSIONS}.vmlab" "${VERSIONS}"
    chown "${TEST_USER}:" "${VERSIONS}"
fi
user_bus env HOME="${HOME_DIR}" bash /usr/share/amethystora/user-setup.hooks.d/23-emoji-key.sh >/dev/null 2>&1
same "15 the hook resets the old key" "$(user_bus dconf read "${CUSTOM6}binding")" ""

loginctl unlock-sessions
systemctl restart gdm
emoji_ready() { [[ "$(ev "typeof ${E}?.snapshot")" == function ]]; }
if ! wait_for 240 has_session || ! wait_for 120 emoji_ready; then
    not_ok "the emoji module did not start in the session"
    finish
fi
overview_hidden() { ev "Main.overview.hide(); 'hidden'" >/dev/null; [[ "$(ev "Main.overview.visible")" == false ]]; }
wait_for 20 overview_hidden
FORCED="$(ev "global.force_animations")"
# The lab's own language, read first, so that every path (the trap's too) puts it back: the account's language in
# AccountsService (GDM's LANG at the next login, as suite 42 sets it) and what the shell reads
USER_PATH="/org/freedesktop/Accounts/User$(id -u "${TEST_USER}")"
account_language() {
    busctl --system --json=short get-property org.freedesktop.Accounts "${USER_PATH}" org.freedesktop.Accounts.User Language |
        jq -r .data
}
set_language() {
    busctl --system call org.freedesktop.Accounts "${USER_PATH}" org.freedesktop.Accounts.User SetLanguage s "$1" >/dev/null
}
shell_locale() { ev "imports.gi.GLib.get_language_names_with_category('LC_MESSAGES')[0]"; }
LANGUAGE_LAB="$(account_language)"
LOCALE_LAB="$(shell_locale)"
LOCALE_CHANGED=""
# A new session: the old shell's mark gone (has_session is still "active" for the closing one), the module up again
new_shell() { [[ "$(ev "String(globalThis.vmlabOldShell)")" == undefined ]] && emoji_ready; }
relogin() {
    ev "globalThis.vmlabOldShell = true; 'marked'" >/dev/null
    loginctl unlock-sessions
    systemctl restart gdm
    wait_for 240 has_session && wait_for 120 new_shell || return 1
    wait_for 20 overview_hidden
}
# The lab's language back and a new session; false unless the shell reads the lab's own again
lab_locale() {
    set_language "${LANGUAGE_LAB}"
    relogin && [[ "$(shell_locale)" == "${LOCALE_LAB}" ]]
}
cleanup() {
    # The text shortcuts' part puts back what it changed (its switch and the account's shortcuts file)
    declare -F typing_cleanup >/dev/null && typing_cleanup
    ev "global.force_animations = ${FORCED:-false}; 'restored'" >/dev/null 2>&1
    gs reset "${A11Y}" reduced-motion
    gs reset "${EMOJI}" skin-tone
    user_bus systemctl --user stop 'vmlab-33-*' >/dev/null 2>&1
    if [[ -n ${LOCALE_CHANGED} ]] && ! lab_locale; then
        not_ok "the lab's language is back after the suite" "expected: ${LOCALE_LAB}" "found: $(shell_locale)"
        exit 1
    fi
}
trap cleanup EXIT

# --- 1: the keys ----------------------------------------------------------------------------------------------------
same "1 the picker's key" "$(gs get "${EMOJI}" emoji-picker)" "['<Super>period']"
same "1 the image's old Smile key is gone" "$(user_bus dconf read "${CUSTOM6}binding")" ""
same "1 the skin tone" "$(gs get "${EMOJI}" skin-tone)" "'none'"
check "1 Smile keeps Ctrl+Alt+Space" grep -q "binding='<Control><Alt>space'" /etc/dconf/db/distro.d/02-amethystora-keybindings

# --- 2-4: a document, the picker under its cursor, a pick typed in --------------------------------------------------
# GNOME's input method keeps the text cursor only while IBus is connected to it, which comes a few seconds after the
# session starts: the editor opens after that, so that the cursor its text input reports is kept
ibus_up() { [[ "$(ev "String(!!Main.inputMethod._context)")" == true ]]; }
wait_for 60 ibus_up || not_ok "2 IBus did not connect to the shell within 60 s"
user_bus systemd-run --user --collect --unit=vmlab-33-editor flatpak run org.gnome.TextEditor --new-window >/dev/null
editor_up() { user_bus python3 "${HERE}/atspi.py" gnome-text-editor has "New Document" >/dev/null 2>&1; }
editor_focused() { [[ "$(ev "global.display.focus_window?.get_wm_class()")" == *TextEditor* ]]; }
if ! wait_for 60 editor_up || ! soon 10 editor_focused; then
    not_ok "2 Text Editor did not open"
    finish
fi
open_picker
check "2 Super+. opens the picker" is_open
# 15: and Smile does not start on it any more
smile() { pgrep -f it.mijorus.smile >/dev/null; }
if soon 3 smile; then not_ok "15 Super+. started Smile"; else ok "15 Super+. starts no Smile"; fi
scale="$(ev "imports.gi.St.ThemeContext.get_for_stage(global.stage).scale_factor")"
same "2 its width" "$(q .width)" "$((400 * scale))"
# What the picker was placed by, as the module says: read after it opened, the input method's cursor is the picker's
# own field's
placed="$(q .placed)"
if [[ ${placed} == caret ]]; then
    ok "2 placed under the text cursor"
else
    not_ok "2 placed under the text cursor" "found: placed by the ${placed:-nothing} (y $(q .y))" "IBus connected: $(ev "String(!!Main.inputMethod._context)")"
fi
same "3 it opens on Smileys" "$(q .tab)" smileys
same "3 ...with 😀 first" "$(q '.cells[0]')" "😀"
# A known clipboard first: a session restored from an earlier run may hold 😀 already
ev "imports.gi.St.Clipboard.get_default().set_text(imports.gi.St.ClipboardType.CLIPBOARD, 'vmlab-33'); 'seeded'" >/dev/null
keys text:grinning
query_is() { [[ "$(q .query)" == "$1" ]]; }
soon 3 query_is grinning
same "4 grinning" "$(q '.query + " " + .cells[0]')" "grinning 😀"
keys return
check "4 Return closes it" soon 2 is_closed
typed() { editor_text | grep -q "$1"; }
check "4 the document holds 😀 within 2 s" wait_for 2 typed "😀"
same "4 Recent keeps it" "$(jq -c .recent "${STATE}")" '["😀"]'
clip_known() { [[ "$(ev "String(globalThis.vmlabClip)")" != undefined ]]; }
ev "globalThis.vmlabClip = undefined; imports.gi.St.Clipboard.get_default().get_text(imports.gi.St.ClipboardType.CLIPBOARD, (c, t) => (globalThis.vmlabClip = t)); 'asked'" >/dev/null
soon 3 clip_known
clip="$(ev "String(globalThis.vmlabClip)")"
if [[ "${clip}" == vmlab-33 ]]; then ok "4 typed through the input method: the clipboard is untouched"; else
    not_ok "4 the pick went through the clipboard" "the window took no text from the input method (clipboard: ${clip})"; fi
same "4 the window has the input method back once the picker closed" "$(ev "String(!!Main.inputMethod.currentFocus)")" true

# --- 5-6: Recent, a search, the arrow keys --------------------------------------------------------------------------
open_picker
same "5 it opens on Recent" "$(q '.tab + " " + .cells[0]')" "recent 😀"
keys text:euro
soon 3 query_is euro
same "5 euro" "$(q '.cells[0]')" "€"
keys return
check "5 the document ends with 😀€" wait_for 2 typed "😀€"
open_picker
keys ctrl+pagedown
tab_is() { [[ "$(q .tab)" == "$1" ]]; }
soon 3 tab_is smileys
third="$(q '.cells[2]')"
keys down right right return
check "6 Down, Right, Right, Return types the third character (${third})" wait_for 2 typed "${third}"

# --- 7: a terminal --------------------------------------------------------------------------------------------------
user_bus systemd-run --user --collect --unit=vmlab-33-kitty kitty --title vmlab-33 sh -c "cat > /tmp/vmlab-33-${RUN}" >/dev/null
kitty_up() { [[ "$(ev "global.display.focus_window?.get_title()")" == vmlab-33 ]]; }
if wait_for 20 kitty_up; then
    open_picker
    keys text:euro
    soon 3 query_is euro
    keys return
    check "7 Return closes it" soon 2 is_closed
    in_file() { [[ -s "/tmp/vmlab-33-${RUN}" ]] || { keys return ctrl+d; [[ -s "/tmp/vmlab-33-${RUN}" ]]; }; }
    wait_for 10 in_file
    same "7 the terminal took €" "$(cat "/tmp/vmlab-33-${RUN}" 2>/dev/null)" "€"
else
    not_ok "7 kitty did not open"
fi
user_bus systemctl --user stop vmlab-33-kitty >/dev/null 2>&1
rm -f "/tmp/vmlab-33-${RUN}"
ev "global.display.get_tab_list(0, null).find(w => /TextEditor/.test(w.get_wm_class() ?? ''))?.activate(global.get_current_time()); 'back'" >/dev/null
soon 5 editor_focused

# --- 8: the skin tone -----------------------------------------------------------------------------------------------
in_session "ame desktop emoji tone medium" >/dev/null
same "8 ame desktop emoji tone medium" "$(gs get "${EMOJI}" skin-tone)" "'medium'"
open_picker
keys "text:waving hand"
soon 3 query_is "waving hand"
same "8 waving hand in the medium tone" "$(q '.cells[0]')" "👋🏽"
keys esc
soon 2 is_closed
in_session "ame desktop emoji tone none" >/dev/null
open_picker
keys "text:waving hand"
soon 3 query_is "waving hand"
same "8 ...and back to none" "$(q '.cells[0]')" "👋"

# --- 9: closing without typing --------------------------------------------------------------------------------------
before="$(editor_text | md5sum)"
keys esc
check "9 Escape closes it" soon 2 is_closed
open_picker
keys super+period
check "9 the same key closes it" soon 2 is_closed
open_picker
keys "click:0.95,0.5"
check "9 a click elsewhere closes it" soon 3 is_closed
same "9 nothing was typed" "$(editor_text | md5sum)" "${before}"

# --- 10: a broken state file ----------------------------------------------------------------------------------------
printf '{"version":1,"recent":' >"${STATE}"
chown "${TEST_USER}:" "${STATE}"
rm -f "${STATE}.bak"
ev "global.display.get_tab_list(0, null).find(w => /TextEditor/.test(w.get_wm_class() ?? ''))?.activate(global.get_current_time()); 'back'" >/dev/null
soon 5 editor_focused
open_picker
same "10 a broken file: it opens on Smileys" "$(q .tab)" smileys
keys return
rewritten() { jq -e '.recent | length == 1' "${STATE}" && test -s "${STATE}.bak"; }
check "10 after a pick the file is JSON again, with its copy beside it" soon 3 rewritten

# --- 13: the data is let go -----------------------------------------------------------------------------------------
same "13 closed, nothing of the data is held" "$(q .loaded)" false

# --- 14: the motion, forced on --------------------------------------------------------------------------------------
ev "global.force_animations = true; 'forced'" >/dev/null
gs set "${A11Y}" reduced-motion no-preference
motion_on() { [[ "$(ev "${UI}.get('comfort').reduceMotion")" == false ]]; }
soon 3 motion_on
scale_x="$(ev "(() => { ${E}.show(); return ${E}._picker.actor.scale_x.toFixed(3); })()")"
if [[ ${scale_x} =~ ^0[.][0-9]+$ ]]; then ok "14 the picker grows from ${scale_x}"; else not_ok "14 no scale while it opens" "${scale_x}"; fi
grown() { [[ "$(ev "${E}._picker.actor.scale_x")" == 1 ]]; }
check "14 ...to 1" soon 2 grown
ev "${E}.close(); 'closed'" >/dev/null
gs set "${A11Y}" reduced-motion reduce
motion_off() { [[ "$(ev "${UI}.get('comfort').reduceMotion")" == true ]]; }
soon 3 motion_off
same "14 reduce motion: no scale, the opacity still fades" \
    "$(ev "(() => { ${E}.show(); const a = ${E}._picker.actor; return a.scale_x + ' ' + (a.opacity < 255); })()")" "1 true"
ev "${E}.close(); 'closed'" >/dev/null
S="${UI}.get('search')"
same "14 the card under reduce motion: no scale" "$(ev "(() => { ${S}._show(); return ${S}._box.scale_x; })()")" 1
ev "${S}._close('now'); 'closed'" >/dev/null
gs set "${A11Y}" reduced-motion no-preference
soon 3 motion_on
card_scale="$(ev "(() => { ${S}._show(); return ${S}._box.scale_x.toFixed(3); })()")"
if [[ ${card_scale} =~ ^0[.][0-9]+$ ]]; then ok "14 the card grows from ${card_scale}"; else not_ok "14 the card does not grow" "${card_scale}"; fi
ev "${S}._close('now'); 'closed'" >/dev/null
ev "global.force_animations = ${FORCED:-false}; 'restored'" >/dev/null
gs reset "${A11Y}" reduced-motion

# --- 12: ten disables and enables -----------------------------------------------------------------------------------
since="$(date '+%Y-%m-%d %H:%M:%S')"
ev "globalThis.vmlabCycles = 'running'; (async () => { for (let i = 0; i < 10; i++) { await Main.extensionManager.disableExtension('${UUID}'); await Main.extensionManager.enableExtension('${UUID}'); } globalThis.vmlabCycles = 'done'; })(); 'started'" >/dev/null
cycles() { [[ "$(ev "String(globalThis.vmlabCycles)")" == "$1" ]]; }
check "12 ten cycles end" wait_for 120 cycles done
same "12 the key is bound once, after the cycles" "$(ev "(Main.wm._allowedKeybindings['emoji-picker'] ?? 0) !== 0")" true
ev "(async () => { await Main.extensionManager.disableExtension('${UUID}'); globalThis.vmlabCycles = 'disabled'; })(); 'off'" >/dev/null
soon 20 cycles disabled
same "12 ...and not while it is off" "$(ev "(Main.wm._allowedKeybindings['emoji-picker'] ?? 0) === 0")" true
ev "(async () => { await Main.extensionManager.enableExtension('${UUID}'); })(); 'on'" >/dev/null
check "12 enabled again" wait_for 20 emoji_ready
gs set "${EMOJI}" emoji-picker "[]"
unbound() { [[ "$(ev "(Main.wm._allowedKeybindings['emoji-picker'] ?? 0) === 0")" == true ]]; }
check "12 an empty key turns the picker off" soon 3 unbound
gs reset "${EMOJI}" emoji-picker
bound() { ! unbound; }
check "12 ...and its key back on again" soon 3 bound
if journalctl --since "${since}" -o cat _UID="$(id -u "${TEST_USER}")" | grep -q "JS ERROR\|Gjs-CRITICAL"; then
    not_ok "12 JS ERROR in the cycles" "$(journalctl --since "${since}" -o cat _UID="$(id -u "${TEST_USER}")" | grep -A3 "JS ERROR\|Gjs-CRITICAL" | head -12)"
else
    ok "12 no JS ERROR in ten cycles"
fi

# --- 11: an Arabic session: يورو finds €, and the tabs run right to left ------------------------------------------
LOCALE_CHANGED=1
set_language ar_EG.UTF-8
if relogin && [[ "$(shell_locale)" == ar_EG* ]]; then
    ok "11 the shell reads Arabic"
    ev "${E}.show(); 'shown'" >/dev/null
    soon 5 is_open
    loaded() { [[ "$(q .loaded)" == true ]]; }
    soon 5 loaded
    ev "${E}._picker.entry.set_text('يورو'); 'typed'" >/dev/null
    soon 3 query_is "يورو"
    same "11 يورو finds €" "$(q '.cells[0]')" "€"
    rtl="$(ev "(() => { const b = ${E}._picker._tabsBox.get_children().filter(c => c.visible); const [x0] = b[0].get_transformed_position(); const [x1] = b[1].get_transformed_position(); return x0 > x1; })()")"
    same "11 the tabs run right to left" "${rtl}" true
    ev "${E}.close(); 'closed'" >/dev/null
else
    not_ok "11 the shell reads Arabic" "expected: ar_EG.UTF-8 (LC_MESSAGES)" "found: $(shell_locale)"
fi
if lab_locale; then
    ok "the lab's language is back: ${LOCALE_LAB}"
else
    not_ok "the lab's language is back" "expected: ${LOCALE_LAB}" "found: $(shell_locale)"
fi
LOCALE_CHANGED=""
user_bus systemctl --user stop vmlab-33-editor >/dev/null 2>&1

# ==== Text shortcuts ===============================================================================================
TYPING=org.gnome.shell.extensions.amethystora-ui.typing
TY="${UI}.get('typing')"
SHORTCUTS="${HOME_DIR}/.config/amethystora/text-shortcuts.json"
OWN="/run/user/$(id -u "${TEST_USER}")/vmlab-33"
install -d -o "${TEST_USER}" -g "${TEST_USER}" -m 0700 "${OWN}"
# The account's own shortcuts are kept aside and put back at the end, whatever happens
[[ -e "${SHORTCUTS}" ]] && cp -p "${SHORTCUTS}" "${OWN}/kept.json"
typing_cleanup() {
    gs reset "${TYPING}" expand
    user_bus systemctl --user stop 'vmlab-33-sc-*' >/dev/null 2>&1
    rm -f "${SHORTCUTS}" "${SHORTCUTS}.bak" "${SHORTCUTS}.unreadable"
    [[ -e "${OWN}/kept.json" ]] && cp -p "${OWN}/kept.json" "${SHORTCUTS}"
    [[ -n ${SOURCES:-} ]] && in_session "gsettings set org.gnome.desktop.input-sources sources \"${SOURCES}\"" >/dev/null 2>&1
    return 0
}
ame_as() { user_bus env HOME="${HOME_DIR}" LANGUAGE=C LC_ALL=C.UTF-8 ame "$@"; }
TSNAP="${OWN}/snap.json"
tq() {
    ev "imports.gi.GLib.file_set_contents(${TSNAP@Q}, ${TY}.snapshot()); 'written'" >/dev/null
    jq -r "$1" "${TSNAP}" 2>/dev/null
}
own_wrapper() { ev "String(Main.inputMethod.hasOwnProperty('notify_key_event'))"; }
journal_mark="$(date '+%Y-%m-%d %H:%M:%S')"

# add_sc ABBR TEXT: the text on stdin, as Control hands it, never on a command line (printf is bash's own)
add_sc() { printf '%s' "$2" | ame_as desktop text-shortcut add "$1" >/dev/null; }
rm -f "${SHORTCUTS}" "${SHORTCUTS}.bak" "${SHORTCUTS}.unreadable"
add_sc ';sig' $'Best wishes,\nSam'
add_sc ';addr' '4 Amethyst Lane, Riverton'
add_sc ';d' 'Date: {date}'
add_sc ';sig2' 'Second'
same "T0 the four shortcuts" "$(ame_as desktop text-shortcut list | paste -sd ' ')" ";addr ;d ;sig ;sig2"
same "T0 a text from stdin is kept as it came" "$(ame_as desktop text-shortcut show ';sig' | od -c | head -2 | md5sum)" \
    "$(printf 'Best wishes,\nSam' | od -c | head -2 | md5sum)"

# A document of its own, read through AT-SPI as one line (its lines joined by |)
user_bus systemd-run --user --collect --unit=vmlab-33-sc-editor flatpak run org.gnome.TextEditor --new-window >/dev/null
editor_here() {
    ev "global.display.get_tab_list(0, null).find(w => /TextEditor/.test(w.get_wm_class() ?? ''))?.activate(global.get_current_time()); 'back'" >/dev/null
    editor_focused
}
doc() { editor_text | paste -sd '|'; }
has_doc() { doc | grep -qF -- "$1"; }
if ! wait_for 60 editor_up || ! soon 10 editor_here; then
    not_ok "T Text Editor did not open"
fi

# --- T1: off by default, nothing wrapped, nothing expands ----------------------------------------------------------
same "T1 off by default" "$(gs get "${TYPING}" expand)" false
same "T1 nothing wrapped" "$(tq .wrapped)" false
same "T1 GNOME's input method untouched" "$(own_wrapper)" false
keys text:t1 space semicolon text:addr space
check "T1 ;addr stays as typed" wait_for 4 has_doc "t1 ;addr "

# --- T2: on, at once ------------------------------------------------------------------------------------------------
ame_as desktop text-shortcuts on >/dev/null
wrapped_now() { [[ "$(tq .wrapped)" == true && "$(tq .monitor)" == true ]]; }
check "T2 on within 2 s, no log-out" soon 2 wrapped_now
same "T2 the wrapper is the object's own" "$(own_wrapper)" true

# --- T3, T5, T7: expanding, a correction, and the clipboard untouched ------------------------------------------------
ev "globalThis.vmlabClip = undefined; imports.gi.St.Clipboard.get_default().get_text(imports.gi.St.ClipboardType.CLIPBOARD, (c, t) => (globalThis.vmlabClip = String(t))); 'asked'" >/dev/null
soon 3 clip_known
clip_before="$(ev "String(globalThis.vmlabClip)")"
keys return text:t3 space semicolon text:addr space
check "T3 ;addr became the address" wait_for 4 has_doc "t3 4 Amethyst Lane, Riverton "
if has_doc "t3 ;addr"; then not_ok "T3 the abbreviation is left in the document"; else ok "T3 no ;addr left"; fi
keys return semicolon text:sig space
check "T3 ;sig became two lines" wait_for 4 has_doc "Best wishes,|Sam "
keys return semicolon text:sig2 space
check "T3 the longest wins: ;sig2" wait_for 4 has_doc "|Second "
keys return semicolon text:d space
today="$(LC_ALL=C date +'%B %-d, %Y')"
check "T3 {date} is today, ${today}" wait_for 4 has_doc "Date: ${today} "
keys return text:t5 space semicolon text:adx backspace text:dr space
check "T5 a BackSpace in the abbreviation still expands" wait_for 4 has_doc "t5 4 Amethyst Lane, Riverton "
ev "globalThis.vmlabClip = undefined; imports.gi.St.Clipboard.get_default().get_text(imports.gi.St.ClipboardType.CLIPBOARD, (c, t) => (globalThis.vmlabClip = String(t))); 'asked'" >/dev/null
soon 3 clip_known
same "T7 the clipboard is untouched" "$(ev "String(globalThis.vmlabClip)")" "${clip_before}"

# --- T4, T6: where a word starts, case, and a moved cursor ---------------------------------------------------------
keys return text:t4x semicolon text:addr space
check "T4 x;addr stays (no word starts there)" wait_for 4 has_doc "t4x;addr "
keys return text:t4c space semicolon shift+a text:ddr space
check "T4 ;Addr stays (case counts)" wait_for 4 has_doc "t4c ;Addr "
keys return text:t6 space semicolon text:addr left right space
check "T6 the arrows forget what was typed" wait_for 4 has_doc "t6 ;addr "

# --- T8: password, PIN and number fields ---------------------------------------------------------------------------
cat >"${OWN}/fields.js" <<'EOF'
// The suite's window: a password entry, a PIN entry, a digits entry and a plain one; each says its text on every
// change into fields.log beside it, and the one named in focus.txt there takes the focus when that file changes
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
const win = new Gtk.Window({title: 'vmlab-33-fields', default_width: 600, default_height: 400});
const box = new Gtk.Box({orientation: Gtk.Orientation.VERTICAL, spacing: 8});
win.set_child(box);
const fields = {
    password: new Gtk.PasswordEntry(),
    pin: new Gtk.Entry({input_purpose: Gtk.InputPurpose.PIN}),
    digits: new Gtk.Entry({input_purpose: Gtk.InputPurpose.DIGITS}),
    plain: new Gtk.Entry(),
    plain2: new Gtk.Entry(),
};
for (const [name, field] of Object.entries(fields)) {
    box.append(field);
    field.connect('notify::text', () => log(name, field.text));
}
const focus = Gio.File.new_for_path(`${here}/focus.txt`);
const monitor = focus.monitor_file(Gio.FileMonitorFlags.NONE, null);
monitor.connect('changed', () => {
    try {
        // Without selecting: GtkEntry.grab_focus() selects its text, which the next key would replace
        const field = fields[new TextDecoder().decode(focus.load_contents(null)[1]).trim()];
        if (field?.grab_focus_without_selecting)
            field.grab_focus_without_selecting();
        else
            field?.grab_focus();
    } catch {}
});
win.connect('close-request', () => loop.quit());
win.present();
loop.run();
EOF
chown "${TEST_USER}:" "${OWN}/fields.js"
rm -f "${OWN}/fields.log"
user_bus systemd-run --user --collect --unit=vmlab-33-sc-fields gjs -m "${OWN}/fields.js" >/dev/null
fields_here() {
    ev "global.display.get_tab_list(0, null).find(w => w.get_title() === 'vmlab-33-fields')?.activate(global.get_current_time()); 'back'" >/dev/null
    [[ "$(ev "global.display.focus_window?.get_title()")" == vmlab-33-fields ]]
}
field_text() { grep "^$1	" "${OWN}/fields.log" 2>/dev/null | tail -n 1 | cut -f2- | jq -r .; }
field_is() { [[ "$(field_text "$1")" == "$2" ]]; }
if wait_for 30 fields_here; then
    password_before="$(tq .skipped.password)"
    for field in password pin digits plain; do
        printf '%s\n' "${field}" >"${OWN}/focus.txt"
        sleep 1
        keys semicolon text:addr space
        if [[ ${field} == plain ]]; then
            check "T8 the plain field expands" wait_for 4 field_is plain "4 Amethyst Lane, Riverton "
        else
            check "T8 the ${field} field keeps ;addr" wait_for 4 field_is "${field}" ";addr "
        fi
        [[ ${field} == password ]] && same "T8 nothing kept after the password field" "$(tq .bufferLength)" 0
    done
    skipped_password=$(($(tq .skipped.password) - password_before))
    if ((skipped_password >= 2)); then ok "T8 password and PIN keys skipped (${skipped_password})"; else not_ok "T8 password and PIN keys skipped" "${skipped_password}"; fi
    # T8b: half a shortcut in one field, the rest in another field of the same window, which the module cannot see
    # happen (Mutter's one input focus): the app's own text is what decides, and nothing expands
    printf 'plain2\n' >"${OWN}/focus.txt"
    sleep 1
    keys semicolon text:ad
    printf 'plain\n' >"${OWN}/focus.txt"
    sleep 1
    keys text:dr space
    check "T8b a shortcut split over two fields does not expand" wait_for 4 field_is plain "4 Amethyst Lane, Riverton dr "
    same "T8b ...and the first field keeps its half" "$(field_text plain2)" ";ad"
    # T8c: a key pressed while an expansion waits gives it up (a key event as the observer sees one)
    X="${TY}._expander"
    same "T8c a key gives a waiting expansion up" "$(ev "(() => { const C = imports.gi.Clutter; ${X}._expand(';addr'); const waiting = ${X}._expanding !== null; ${X}._observe({type: () => C.EventType.KEY_PRESS, get_key_symbol: () => C.KEY_x, get_state: () => 0, get_key_unicode: () => 'x'}, false); return waiting + ' ' + (${X}._expanding === null); })()")" "true true"
else
    not_ok "T8 the fields window did not open"
fi
user_bus systemctl --user stop vmlab-33-sc-fields >/dev/null 2>&1

# --- T9: the shell's own fields ------------------------------------------------------------------------------------
soon 5 editor_here
shell_before="$(tq .skipped.shell)"
keys super+space
search_open() { [[ "$(ev "String(global.stage.key_focus instanceof imports.gi.Clutter.Text)")" == true ]]; }
if soon 5 search_open; then
    keys semicolon text:addr space
    sleep 1
    same "T9 the search box keeps ;addr" "$(ev "global.stage.key_focus.get_text()")" ";addr "
    keys esc
    ((($(tq .skipped.shell)) > shell_before)) && ok "T9 the shell's keys skipped" || not_ok "T9 the shell's keys skipped"
else
    not_ok "T9 Super+Space opened no search box"
fi

# --- T10: kitty, one line only -------------------------------------------------------------------------------------
rm -f "/tmp/vmlab-33-sc-${RUN}"
user_bus systemd-run --user --collect --unit=vmlab-33-sc-kitty kitty --title vmlab-33-sc sh -c "cat > /tmp/vmlab-33-sc-${RUN}" >/dev/null
kitty_sc() { [[ "$(ev "global.display.focus_window?.get_title()")" == vmlab-33-sc ]]; }
if wait_for 20 kitty_sc; then
    keys semicolon text:addr space return semicolon text:sig space return
    # T10b: another window takes the keyboard while an expansion in the terminal waits for its BackSpace keys: the
    # text goes nowhere, not into the window that has the keyboard now
    in_doc_before="$(doc | grep -o '4 Amethyst Lane' | wc -l)"
    X="${TY}._expander"
    same "T10b an expansion waits in the terminal, and the editor takes the keyboard" "$(ev "(() => { ${X}._expand(';addr'); const waiting = ${X}._expanding !== null; global.display.get_tab_list(0, null).find(w => /TextEditor/.test(w.get_wm_class() ?? ''))?.activate(global.get_current_time()); return waiting + ' ' + (global.display.focus_window?.get_title() !== 'vmlab-33-sc'); })()")" "true true"
    sleep 1
    same "T10b ...and nothing reached the window that took the keyboard" "$(doc | grep -o '4 Amethyst Lane' | wc -l)" "${in_doc_before}"
    ev "global.display.get_tab_list(0, null).find(w => w.get_title() === 'vmlab-33-sc')?.activate(global.get_current_time()); 'back'" >/dev/null
    soon 5 kitty_sc
    keys return ctrl+d
    sc_written() { [[ "$(wc -l <"/tmp/vmlab-33-sc-${RUN}" 2>/dev/null)" -ge 2 ]]; }
    wait_for 10 sc_written
    same "T10 kitty: the address" "$(sed -n 1p "/tmp/vmlab-33-sc-${RUN}")" "4 Amethyst Lane, Riverton "
    same "T10 kitty: ;sig stays, its text has two lines" "$(sed -n 2p "/tmp/vmlab-33-sc-${RUN}")" ";sig "
    if grep -q '^Sam' "/tmp/vmlab-33-sc-${RUN}"; then not_ok "T10 a line break reached the terminal"; else ok "T10 no line break typed"; fi
    same "T10b ...nor the terminal it was given up in" "$(grep -c 'Amethyst' "/tmp/vmlab-33-sc-${RUN}")" 1
else
    not_ok "T10 kitty did not open"
fi
user_bus systemctl --user stop vmlab-33-sc-kitty >/dev/null 2>&1
rm -f "/tmp/vmlab-33-sc-${RUN}"

# --- T11: beside Mozc, which keeps working, and the input sources the module never wrote ----------------------------
SOURCES="$(in_session "gsettings get org.gnome.desktop.input-sources sources")"
in_session "gsettings set org.gnome.desktop.input-sources sources \"[('xkb', 'us'), ('ibus', 'mozc-jp')]\"" >/dev/null
soon 5 editor_here
# IBus's one engine, as the source switcher sets it
in_session "ibus engine mozc-jp" >/dev/null 2>&1
sleep 2
before_preedit="$(( $(tq .skipped.preedit) + $(tq .skipped.filtered) ))"
keys text:sa
preedit="$(ev "String(Main.inputMethod.hasPreedit())")"
if [[ ${preedit} == true ]]; then
    keys semicolon text:addr space
    sleep 1
    after_preedit="$(( $(tq .skipped.preedit) + $(tq .skipped.filtered) ))"
    ((after_preedit > before_preedit)) && ok "T11 Mozc's keys are left to it" || not_ok "T11 Mozc's keys are left to it"
    keys esc esc
else
    skip "T11 Mozc showed no preedit here (its mode may be direct input)"
fi
in_session "ibus engine xkb:us::eng" >/dev/null 2>&1
sleep 2
same "T11 the input sources are what the test set" "$(in_session "gsettings get org.gnome.desktop.input-sources sources")" "[('xkb', 'us'), ('ibus', 'mozc-jp')]"
keys return text:t11 space semicolon text:addr space
check "T11 back on the US layout it expands again" wait_for 4 has_doc "t11 4 Amethyst Lane, Riverton "
in_session "gsettings set org.gnome.desktop.input-sources sources \"${SOURCES}\"" >/dev/null

# --- T12: an X11 app is not covered (as documented) ----------------------------------------------------------------
cat >"${OWN}/x11.js" <<'EOF'
// A GTK 3 entry under Xwayland, which says its text into x11.log beside it
imports.gi.versions.Gtk = '3.0';
const {Gtk, GLib, Gio} = imports.gi;
const here = GLib.path_get_dirname(imports.system.programPath);
Gtk.init(null);
const win = new Gtk.Window({title: 'vmlab-33-x11'});
const entry = new Gtk.Entry();
entry.connect('changed', () => {
    const stream = Gio.File.new_for_path(`${here}/x11.log`).replace(null, false, Gio.FileCreateFlags.NONE, null);
    stream.write_all(new TextEncoder().encode(entry.text), null);
    stream.close(null);
});
win.add(entry);
win.connect('destroy', () => Gtk.main_quit());
win.show_all();
Gtk.main();
EOF
chown "${TEST_USER}:" "${OWN}/x11.js"
rm -f "${OWN}/x11.log"
user_bus systemd-run --user --collect --unit=vmlab-33-sc-x11 env GDK_BACKEND=x11 gjs "${OWN}/x11.js" >/dev/null
x11_here() {
    ev "global.display.get_tab_list(0, null).find(w => w.get_title() === 'vmlab-33-x11')?.activate(global.get_current_time()); 'back'" >/dev/null
    [[ "$(ev "global.display.focus_window?.get_title()")" == vmlab-33-x11 ]]
}
if wait_for 30 x11_here; then
    keys semicolon text:addr space
    sleep 2
    if [[ "$(cat "${OWN}/x11.log" 2>/dev/null)" == ";addr " ]]; then
        ok "X11 apps are not covered (as documented)"
    else
        not_ok "T12 the X11 entry says" "$(cat "${OWN}/x11.log" 2>/dev/null)"
    fi
else
    skip "T12 the X11 window did not open"
fi
user_bus systemctl --user stop vmlab-33-sc-x11 >/dev/null 2>&1

# --- T13: the file and its helper ----------------------------------------------------------------------------------
soon 5 editor_here
ame_as desktop text-shortcut remove ';addr' >/dev/null
count_is() { [[ "$(tq .shortcuts)" == "$1" ]]; }
check "T13 the shell reads the file again within 2 s" soon 2 count_is 3
keys return text:t13 space semicolon text:addr space
check "T13 a removed shortcut stays as typed" wait_for 4 has_doc "t13 ;addr "
add_sc ';addr' '4 Amethyst Lane, Riverton'
soon 2 count_is 4
keys return text:t13b space semicolon text:addr space
check "T13 added back, it expands" wait_for 4 has_doc "t13b 4 Amethyst Lane, Riverton "
same "T13 the file is 0600" "$(stat -c %a "${SHORTCUTS}")" 600
check "T13 its copy is beside it" test -s "${SHORTCUTS}.bak"
# A broken file: the shell and the helper's list both read its copy, and the list changes nothing; the next change
# puts the file back
printf 'garbage' >"${SHORTCUTS}"
chown "${TEST_USER}:" "${SHORTCUTS}"
sleep 1
check "T13 the shell reads the copy of a broken file" soon 2 count_is 4
same "T13 the helper's list reads the copy" "$(ame_as desktop text-shortcut list | paste -sd ' ')" ";addr ;d ;sig ;sig2"
same "T13 ...and leaves the file as it is" "$(cat "${SHORTCUTS}")" garbage
add_sc ';addr' '4 Amethyst Lane, Riverton'
check "T13 the next change puts the file back" jq -e '.shortcuts | length == 4' "${SHORTCUTS}"

# --- T14: what the helper refuses ----------------------------------------------------------------------------------
refused() {
    local what="$1" message="$2" said code
    shift 2
    said="$(ame_as desktop text-shortcut "$@" 2>&1 >/dev/null)"
    code=$?
    if [[ ${code} -ne 0 && ${said} == *"${message}"* ]]; then ok "T14 ${what}"; else not_ok "T14 ${what}" "exit ${code}: ${said}"; fi
}
refused "a shortcut with a space" "A shortcut is 2 to 32 characters, with no spaces." add 'a b' x
refused "an escape sequence" "The text has a character that cannot be typed." add ';x' $'\e[31m'
said="$(printf '\e[31m' | ame_as desktop text-shortcut add ';x' 2>&1 >/dev/null)"
[[ ${said} == *"cannot be typed"* ]] && ok "T14 an escape sequence on stdin" || not_ok "T14 an escape sequence on stdin" "${said}"
FULL="${OWN}/full"
install -d -o "${TEST_USER}" -m 0700 "${FULL}/amethystora"
jq -n '{version: 1, shortcuts: [range(200) | {abbr: ";n\(.)", text: "x"}]}' >"${FULL}/amethystora/text-shortcuts.json"
chown -R "${TEST_USER}:" "${FULL}"
said="$(user_bus env HOME="${HOME_DIR}" XDG_CONFIG_HOME="${FULL}" LANGUAGE=C /usr/libexec/amethystora-text-shortcuts add ';one-more' x 2>&1)"
code=$?
if [[ ${code} == 2 && ${said} == "There are 200 shortcuts already." ]]; then ok "T14 the 201st"; else not_ok "T14 the 201st" "exit ${code}: ${said}"; fi
rm -rf "${FULL}"

# --- T16: Control's Typing group -----------------------------------------------------------------------------------
in_session "pkill -x amethystora-control" >/dev/null 2>&1
user_bus systemd-run --user --collect --unit=vmlab-33-sc-control amethystora-control desktop >/dev/null
control() { user_bus python3 "${HERE}/atspi.py" amethystora-control "$@"; }
if control wait 30 "Your shortcuts" >/dev/null 2>&1; then
    words="$(control text 2>/dev/null)"
    for abbr in ';addr' ';d' ';sig' ';sig2'; do
        grep -qx -- "${abbr}" <<<"${words}" && ok "T16 Control lists ${abbr}" || not_ok "T16 Control lists ${abbr}"
    done
    grep -qF 'Amethyst Lane' <<<"${words}" && not_ok "T16 Control's window shows a text" || ok "T16 no text shown, only abbreviations"
    ev "global.display.get_tab_list(0, null).find(w => /Control/.test(w.get_wm_class() ?? ''))?.activate(global.get_current_time()); 'back'" >/dev/null
    sleep 1
    keys ctrl+f text:shortcut
    # The keyboard to the field to try them in, one Tab at a time, as a person would
    try_focused() {
        user_bus python3 - <<'PYEOF'
import sys
import gi
gi.require_version("Atspi", "2.0")
from gi.repository import Atspi
def walk(node, depth=0):
    if node is None or depth > 60:
        return
    yield node
    for i in range(node.get_child_count()):
        yield from walk(node.get_child_at_index(i), depth + 1)
desktop = Atspi.get_desktop(0)
for i in range(desktop.get_child_count()):
    app = desktop.get_child_at_index(i)
    if app is not None and app.get_name() == "amethystora-control":
        sys.exit(0 if any((n.get_name() or "") == "Try it here" and n.get_state_set().contains(Atspi.StateType.FOCUSED)
                          for n in walk(app)) else 1)
sys.exit(1)
PYEOF
    }
    reached=""
    for _ in $(seq 40); do
        keys tab
        if try_focused; then reached=1; break; fi
    done
    if [[ -n ${reached} ]]; then
        keys semicolon text:sig2 space
        check "T16 the try field says what it expanded" control wait 4 "Expanded: ;sig2 became “Second”."
    else
        skip "T16 the try field could not be reached with Tab here"
    fi
    # Another shortcut's text is replaced only on a second press, after the window has said so
    control type ';d' Shortcut >/dev/null 2>&1
    control type 'Replaced' Text >/dev/null 2>&1
    control click "Add the shortcut" >/dev/null 2>&1
    check "T16 Add over an abbreviation there is asks first" control wait 4 "You have a shortcut ;d already"
    same "T16 ...and has replaced nothing yet" "$(ame_as desktop text-shortcut show ';d')" 'Date: {date}'
    control click "Add the shortcut" >/dev/null 2>&1
    replaced() { [[ "$(ame_as desktop text-shortcut show ';d')" == Replaced ]]; }
    check "T16 ...a second press replaces it" wait_for 10 replaced
    add_sc ';d' 'Date: {date}'
    control click "Remove the shortcut ;sig2" >/dev/null 2>&1
    gone() { ! ame_as desktop text-shortcut list | grep -qx ';sig2'; }
    check "T16 Remove in the window takes ;sig2 away" wait_for 10 gone
else
    not_ok "T16 Control did not show the Typing group"
fi
user_bus systemctl --user stop vmlab-33-sc-control >/dev/null 2>&1
same "T16 the switch reads on" "$(user_bus env HOME="${HOME_DIR}" /usr/libexec/amethystora-control-list --json | jq -r '.items[] | select(.id == "text-shortcuts") | .state')" on
same "T16 ame control carries no text" "$(ame_as control 2>/dev/null | grep -c 'Amethyst Lane'; user_bus env HOME="${HOME_DIR}" /usr/libexec/amethystora-control-list --json | grep -c 'Amethyst Lane')" $'0\n0'

# --- T18: the cost per key ------------------------------------------------------------------------------------------
soon 5 editor_here
timed0="$(tq .timed)"
total0="$(tq .totalUs)"
keys return text:"$(printf 'the quick brown fox jumps over %.0s' 1 2 3 4 5 6)"
sleep 2
# The sentence's own keys: what the session typed before (the fields, kitty, Control) is left out
timed1="$(tq .timed)"
mean_us=$(((timed1 > timed0) ? ($(tq .totalUs) - total0) / (timed1 - timed0) : 0))
max_us="$(tq .maxUs)"
if ((mean_us < 50)); then ok "T18 a key costs ${mean_us} µs on average (the longest ${max_us} µs)"; else not_ok "T18 a key costs ${mean_us} µs on average" "the longest ${max_us} µs"; fi
# The longest, generously: the lab draws in software and a garbage collection can fall in a key, but no key's own work
# comes near 5 ms
if ((max_us < 5000)); then ok "T18 no key took 5 ms (the longest ${max_us} µs)"; else not_ok "T18 a key took ${max_us} µs"; fi

# --- T17: off again -------------------------------------------------------------------------------------------------
ame_as desktop text-shortcuts off >/dev/null
unwrapped() { [[ "$(own_wrapper)" == false && "$(tq .monitor)" == false ]]; }
check "T17 off: GNOME's input method as it was, no monitor" soon 2 unwrapped
observed_off="$(tq .observed)"
keys return text:t17 space semicolon text:addr space
check "T17 ;addr stays" wait_for 4 has_doc "t17 ;addr "
same "T18 off, nothing of the module runs per key" "$(tq .observed)" "${observed_off}"

# --- T19: ten disables and enables with it on ----------------------------------------------------------------------
ame_as desktop text-shortcuts on >/dev/null
soon 2 wrapped_now
cycle_since="$(date '+%Y-%m-%d %H:%M:%S')"
ev "globalThis.vmlabCycles = 'running'; (async () => { for (let i = 0; i < 10; i++) { await Main.extensionManager.disableExtension('${UUID}'); await Main.extensionManager.enableExtension('${UUID}'); } globalThis.vmlabCycles = 'done'; })(); 'started'" >/dev/null
check "T19 ten cycles end" wait_for 120 cycles done
soon 5 wrapped_now
same "T19 the object's own wrapper, over GNOME's" "$(ev "String(Main.inputMethod.notify_key_event !== Object.getPrototypeOf(Main.inputMethod).notify_key_event)")" true
soon 5 editor_here
before_key="$(tq .observed)"
# keys.py presses Shift first, which the observer sees too (press and release) unless the new device lost it: three
# x seen once each are 6 or 8, seen twice (a second observer) 12 or 16
keys x x x
sleep 1
seen=$(($(tq .observed) - before_key))
if ((seen == 6 || seen == 8)); then ok "T19 one key is seen once (press and release)"; else not_ok "T19 one key is seen once (press and release)" "three x and Shift: expected 6 or 8" "found: ${seen}"; fi
if journalctl --since "${cycle_since}" -o cat _UID="$(id -u "${TEST_USER}")" | grep -q "JS ERROR\|Gjs-CRITICAL"; then
    not_ok "T19 JS ERROR in the cycles" "$(journalctl --since "${cycle_since}" -o cat _UID="$(id -u "${TEST_USER}")" | grep -A3 "JS ERROR\|Gjs-CRITICAL" | head -12)"
else
    ok "T19 no JS ERROR in ten cycles"
fi

# --- T20: the lock screen -------------------------------------------------------------------------------------------
loginctl lock-sessions
sleep 4
same "T20 locked: the module is off" "$(ev "typeof Main.extensionManager.lookup('${UUID}')?.stateObj?.modules?.get('typing')?.snapshot")" undefined
same "T20 locked: GNOME's input method as it was" "$(own_wrapper)" false
loginctl unlock-sessions
sleep 4
check "T20 unlocked: wrapped again" soon 5 wrapped_now

# --- T15: nothing typed was logged ---------------------------------------------------------------------------------
if journalctl --since "${journal_mark}" -o cat _UID="$(id -u "${TEST_USER}")" | grep -E 'Amethyst Lane|;addr|Best wishes' |
    grep -v 'vmlab' | grep -q .; then
    not_ok "T15 a shortcut or what was typed is in the user's journal"
else
    ok "T15 nothing typed in the user's journal"
fi
if journalctl --since "${journal_mark}" -o cat | grep -E 'Amethyst Lane|Best wishes' | grep -v 'vmlab\|systemd-run\|runuser' | grep -q .; then
    not_ok "T15 a shortcut's text is in the journal" "$(journalctl --since "${journal_mark}" -o cat | grep -E 'Amethyst Lane|Best wishes' | head -3)"
else
    ok "T15 no shortcut's text in the journal"
fi

typing_cleanup
user_bus systemctl --user stop vmlab-33-sc-editor >/dev/null 2>&1
finish
