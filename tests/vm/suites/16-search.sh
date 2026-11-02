#!/usr/bin/bash
# vmlab-vm: search
# Search that answers (plans/beyond/spec-search.md, section 8, suite 16), in the test account's session, asked of GNOME
# Shell itself through Eval, which a lab-only extension of the test account opens (never in the image):
#   - the card (Super+Space): its place and width, Amethystora's providers in the overview's controller, in order;
#   - sums and conversions, what is not a sum, Enter copying the answer, and the session's own numbers;
#   - Control's settings (Enter changes one, Ctrl+N on a terminal one), open windows, the manual, clip:, the agent row;
#   - Glance on a file Files found, the keys turning each part off at once, the overview, ten disables and enables,
#     and nothing left running once the card is closed.
# The lab draws without a GPU, where GNOME turns animations off by itself: the motion steps are suite 33's.
# vmlab-paths: @ui/extension.js @ui/*search* @ui/*fuzzy* @sf/*themed/shell-search* @sf/*just/desktop.just @sf/usr/libexec/amethystora-file-find @sf/usr/share/glib-2.0/schemas/zz1*
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HOME_DIR="/var/home/${TEST_USER}"
SEARCH=org.gnome.shell.extensions.amethystora-ui.search
HERE="$(dirname "$(readlink -f "$0")")"
RUN="$RANDOM"
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        sed -E -e "s/^\(true, '(.*)'\)\$/\1/" -e 's/^"(.*)"$/\1/'
}
gs() { in_session "gsettings $*" 2>/dev/null; }
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
keys() { python3 "${HERE}/keys.py" "$@" >/dev/null 2>&1; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
S="${UI}.get('search')"
# The card's snapshot as JSON, and a jq filter of it
SNAP="/run/user/$(id -u "${TEST_USER}")/vmlab-16-snap.json"
snap() {
    ev "imports.gi.GLib.file_set_contents(${SNAP@Q}, ${S}.snapshot()); 'written'" >/dev/null
    cat "${SNAP}"
}
q() { snap | jq -r "$1" 2>/dev/null; }
# A JS string literal of the text
js() { jq -Rn --arg t "$1" '$t'; }
# soon SECONDS COMMAND...: wait_for, looking every tenth of a second, for what changes within a frame or two
soon() {
    local tries=$(($1 * 10))
    shift
    until "$@" >/dev/null 2>&1; do
        ((tries-- > 0)) || return 1
        sleep 0.1
    done
}
card_open() { [[ "$(q .open)" == true ]]; }
card_closed() { [[ "$(q .open)" == false ]]; }
# GNOME's view has every provider's answer for the text in the field (the manual's waits 250 ms, then asks)
settled() {
    [[ "$(ev "(() => { const v = Main.overview.searchController._searchResults; return !v.searchInProgress && v.terms.join(' ') === Main.overview.searchEntry.get_text().trim().split(/\s+/).join(' '); })()")" == true ]]
}
# Opens the card (when closed) and types TEXT into its field, then waits for the search to settle
search() {
    # A card that does not open leaves the text in the hidden overview's field, where GNOME still searches: said here,
    # since the step that then fails sees results without the card (its keys go nowhere)
    card_open || { keys super+space; soon 5 card_open || echo "# the card did not open for '${1:0:30}': $(shell_state)"; }
    ev "Main.overview.searchEntry.set_text($(js "$1")); 'typed'" >/dev/null
    soon 10 settled
}
close_card() { card_open && keys esc && keys esc; soon 3 card_closed; }
first_section() { q '.sections[0].id'; }
first_title() { q '.sections[0].rows[0].title'; }
section_rows() { q "[.sections[] | select(.id == \"$1\") | .rows[].title] | join(\"|\")"; }
clip_known() { [[ "$(ev 'String(globalThis.vmlabClip)')" != undefined ]]; }
# What the shell's clipboard holds now
clipboard() {
    ev "globalThis.vmlabClip = undefined; imports.gi.St.Clipboard.get_default().get_text(imports.gi.St.ClipboardType.CLIPBOARD, (c, t) => (globalThis.vmlabClip = t)); 'asked'" >/dev/null
    soon 3 clip_known
    ev "String(globalThis.vmlabClip)"
}

if ! has_session; then
    bash "$(dirname "$0")/10-account.sh" >/dev/null
fi
if ! wait_for 300 has_session; then
    not_ok "no session for ${TEST_USER}"
    finish
fi
for key in answers settings manual windows clipboard agent glance old-words remembered; do gs reset "${SEARCH}" "${key}"; done

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
# The new session's login keyring is locked again (GDM's automatic login), and lib.sh unlocks it once a boot only: the
# first Flatpak to ask for a secret (Text Editor, step 12) would bring up GNOME's modal unlock dialog, which takes every
# key after it. Unlocked once the new shell is up, and never waited on for long (a dialog already up holds it).
# Asked again until the login keyring says it is open: just after the login its service may not have loaded it yet
keyring_open() {
    local path
    path="$(user_bus busctl --user call org.freedesktop.secrets /org/freedesktop/secrets org.freedesktop.Secret.Service         ReadAlias s default 2>/dev/null | awk '{ gsub(/"/, "", $2); print $2 }')"
    [[ -n "${path}" && "${path}" != / ]] &&
        [[ "$(user_bus busctl --user get-property org.freedesktop.secrets "${path}" org.freedesktop.Secret.Collection Locked 2>/dev/null)" == "b false" ]]
}
fresh_keyring() {
    local pid tries
    for tries in 1 2 3 4 5 6; do
        unlock_login_keyring &
        pid=$!
        wait_for 20 sh -c "! kill -0 ${pid} 2>/dev/null" || kill "${pid}" 2>/dev/null
        keyring_open && return 0
        sleep 3
    done
    echo "# the login keyring did not open after the restart"
}
search_ready() { [[ "$(ev "typeof ${S}?.snapshot")" == function ]]; }
if ! wait_for 240 has_session || ! wait_for 120 search_ready; then
    not_ok "the search module did not start in the session"
    finish
fi
fresh_keyring
# The overview a session starts in, gone for good: the startup over (it shows the overview at its end), and the shell
# in its normal mode, where the card's key works (no overview, no modal dialog)
overview_hidden() {
    ev "Main.overview.hide(); 'hidden'" >/dev/null
    [[ "$(ev "!Main.layoutManager._startingUp && !Main.overview.visible && Main.actionMode === Shell.ActionMode.NORMAL")" == true ]]
}
# What keeps the card's key from it, for a failure's note
shell_state() {
    ev "'action mode ' + Main.actionMode + ', modal ' + Main.modalCount + ', overview ' + Main.overview.visible + ', key focus ' + global.stage.key_focus"
}
wait_for 20 overview_hidden
# The lab's own theme and locale, read first, so that every path (the trap's too) puts back what the account had: its
# language in AccountsService (GDM's LANG at the next login, as suite 42 sets it), its region, and what the shell reads
USER_PATH="/org/freedesktop/Accounts/User$(id -u "${TEST_USER}")"
account_language() {
    busctl --system --json=short get-property org.freedesktop.Accounts "${USER_PATH}" org.freedesktop.Accounts.User Language |
        jq -r .data
}
set_language() {
    busctl --system call org.freedesktop.Accounts "${USER_PATH}" org.freedesktop.Accounts.User SetLanguage s "$1" >/dev/null
}
# What the shell reads, as the features read it: the numbers' locale from the answers provider itself (LC_ALL, LC_NUMERIC,
# LANG, never LANGUAGE), then the messages' language
shell_locale() {
    ev "(() => { const a = Main.overview.searchController._searchResults._providers.find(p => p.id === 'amethystora-answers'); return (a?._options().locale ?? 'no-answers') + ' ' + imports.gi.GLib.get_language_names_with_category('LC_MESSAGES')[0]; })()"
}
THEME_LAB="$(in_session "amethystora-theme current" 2>/dev/null)"
LANGUAGE_LAB="$(account_language)"
REGION_LAB="$(gs get org.gnome.system.locale region)"
LOCALE_LAB="$(shell_locale)"
THEME_CHANGED=""
LOCALE_CHANGED=""
# A new session: the old shell's mark gone (has_session is still "active" for the closing one), the module up again
new_shell() { [[ "$(ev "String(globalThis.vmlabOldShell)")" == undefined ]] && search_ready; }
relogin() {
    ev "globalThis.vmlabOldShell = true; 'marked'" >/dev/null
    loginctl unlock-sessions
    systemctl restart gdm
    wait_for 240 has_session && wait_for 120 new_shell || return 1
    fresh_keyring
    wait_for 20 overview_hidden
}
# The lab's locale back: the account's language and region, the manager's LC_NUMERIC, a new session; false unless the
# shell reads the lab's own again
lab_locale() {
    set_language "${LANGUAGE_LAB}"
    gs set org.gnome.system.locale region "\"${REGION_LAB}\""
    user_bus systemctl --user unset-environment LC_NUMERIC
    relogin && [[ "$(shell_locale)" == "${LOCALE_LAB}" ]]
}
lab_theme() { [[ -z ${THEME_LAB} ]] || in_session "amethystora-theme set ${THEME_LAB@Q}" >/dev/null 2>&1; }
cleanup() {
    user_bus systemctl --user stop 'vmlab-16-*' >/dev/null 2>&1
    as_user "amethystora-agent off" >/dev/null 2>&1
    in_session "ame desktop phone off" >/dev/null 2>&1
    rm -f "${HOME_DIR}"/Pictures/vmlab-glance-*
    gs reset org.gnome.desktop.interface scaling-factor
    gs set "${SEARCH}" glance true
    gs reset org.gnome.desktop.search-providers disabled
    [[ -z ${THEME_CHANGED} ]] || lab_theme
    if [[ -n ${LOCALE_CHANGED} ]] && ! lab_locale; then
        not_ok "the lab's locale is back after the suite" "expected: ${LOCALE_LAB}" "found: $(shell_locale)"
        exit 1
    fi
}
trap cleanup EXIT
# Firefox's search provider starts a hidden Firefox for an account that never ran it, which does not answer (seen with
# Firefox 159): GNOME's search then never settles, and settled() waits on it. Off for the suite, put back after
gs set org.gnome.desktop.search-providers disabled "['org.mozilla.firefox.desktop']"

# --- 1: the providers, in order -------------------------------------------------------------------------------------
same "1 the search is not degraded" "$(q .degraded)" false
# Times anywhere and Remind me (the time group) are on as the image ships; bookmarks, notes and meaning are off
same "1 the overview's controller holds Amethystora's ten, in their places" \
    "$(ev "Main.overview.searchController._searchResults._providers.map(p => p.id).filter(id => id.startsWith('amethystora-')).join(' ')")" \
    "amethystora-clipboard amethystora-answers amethystora-times amethystora-remind amethystora-words amethystora-settings amethystora-windows amethystora-remembered amethystora-manual amethystora-agent"

# --- 2: place and width, at scale 1 and 2 ---------------------------------------------------------------------------
place_ok() {
    local scale want_top top
    scale="$(ev "imports.gi.St.ThemeContext.get_for_stage(global.stage).scale_factor")"
    same "2 its width at scale ${scale}" "$(q .width)" "$((640 * scale))"
    want_top="$(ev "(() => { const a = Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex); return a.y + Math.round(0.11 * a.height); })()")"
    top="$(q .top)"
    if ((top >= want_top - 1 && top <= want_top + 1)); then ok "2 its top edge at 11% of the work area (scale ${scale})"; else not_ok "2 its top" "expected ${want_top}" "found ${top}"; fi
}
search ""
if card_open; then ok "2 Super+Space opens the card"; else not_ok "2 Super+Space opens the card" "$(shell_state)"; fi
place_ok
close_card
gs set org.gnome.desktop.interface scaling-factor 2
scaled() { [[ "$(ev "imports.gi.St.ThemeContext.get_for_stage(global.stage).scale_factor")" == 2 ]]; }
if soon 10 scaled; then
    search ""
    place_ok
    close_card
else
    # org.gnome.desktop.interface scaling-factor is X11's: a Wayland session's scale is the monitor's own
    skip "2 the screen did not go to scale 2 (scaling-factor is X11's; the lab's Wayland monitor stays at 1)"
fi
gs reset org.gnome.desktop.interface scaling-factor
unscaled() { [[ "$(ev "imports.gi.St.ThemeContext.get_for_stage(global.stage).scale_factor")" == 1 ]]; }
soon 10 unscaled

# --- 3-5: sums and conversions, and what is not one -----------------------------------------------------------------
answer_is() { search "$1"; [[ "$(first_section)" == amethystora-answers && "$(first_title)" == "$2" ]]; }
nbsp=$' '
while IFS='|' read -r text want; do
    if answer_is "${text}" "${want//_/${nbsp}}"; then ok "3-4 ${text} = ${want}"; else not_ok "3-4 ${text}" "expected ${want}" "found $(first_section) $(first_title)"; fi
done <<EOF
2+3*4|14
1284 * 0.18|231.12
200 + 10%|220
-2^2|-4
2^-1|0.5
10/3|3.333333333
2^3^2|512
(1+2)*3|9
5 km in mi|3.10686_mi
100 F in C|37.7778_°C
32 F to C|0_°C
0 K in C|-273.15_°C
5 kg in lb|11.0231_lb
12 in in cm|30.48_cm
1.5 GB in MiB|1,430.51_MiB
90 min in h|1.5_h
3.5 km in miles|2.1748_mi
EOF
long="1$(printf ' %.0s' $(seq 198))+1"
for text in "1/0" "(1+2" "42" "2024-10-28" "07-12" "1.2.3" "5 km in kg" "hello" "100 EUR in USD" "${long}"; do
    search "${text}"
    if [[ "$(q '[.sections[].id] | index("amethystora-answers")')" == null ]]; then ok "5 no answer for ${text:0:20}"; else not_ok "5 an answer for ${text:0:20}"; fi
done

# --- 6: Enter copies the answer -------------------------------------------------------------------------------------
search "2+3*4"
keys return
check "6 Enter closes the card" soon 3 card_closed
same "6 ...and the answer is on the clipboard" "$(clipboard)" "14"

# --- 7: the session's own numbers (the parser in the image's gjs; the provider in a French session at the end) ------
CALC="file:///usr/share/gnome-shell/extensions/${UUID}/modules/search/calc.js"
calc_known() { [[ "$(ev "String(globalThis.vmlabCalc)")" != undefined ]]; }
calc() {
    ev "globalThis.vmlabCalc = undefined; import('${CALC}').then(m => (globalThis.vmlabCalc = m.evaluate($(js "$1"), {locale: '$2'})?.${3:-display} ?? 'none')); 'asked'" >/dev/null
    soon 3 calc_known
    ev "String(globalThis.vmlabCalc)"
}
same "7 fr: 1,5 + 1" "$(calc '1,5 + 1' fr-FR)" "2,5"
same "7 fr: grouped with U+202F, copied without" "$(calc $'1 234,5 * 2' fr-FR copy)" "2469"
same "7 de: 1.000,5 * 2" "$(calc '1.000,5 * 2' de-DE)" "2.001"
# GNOME splits the field at every space, the no-break ones too: the provider reads the field itself
search $'1 234 * 2'
same "7 a number grouped with U+202F reaches the answer through the provider" "$(first_section) $(first_title)" \
    "amethystora-answers $(ev "new Intl.NumberFormat('en-US').format(2468)")"
close_card

# --- 8: Control's settings ------------------------------------------------------------------------------------------
in_session "ame desktop phone off" >/dev/null 2>&1
# Control's list is read the first time a search starts, in the background, and the section comes when it is in
search "phone pairing"
settings_in() { [[ "$(section_rows amethystora-settings)" == *"Phone pairing"* ]]; }
check "8 Control's list arrives within 60 s" soon 60 settings_in
same "8 the first setting row" "$(q '[.sections[] | select(.id == "amethystora-settings")][0].rows[0] | .title + " " + .meta')" "Phone pairing On"
check "8 ...and Control's own row" test "$(section_rows amethystora-settings)" == "Phone pairing|Control › Phone pairing"
if [[ "$(first_section)" == amethystora-settings ]]; then keys return; else keys ctrl+1; fi
enabled_now() { [[ "$(as_user "XDG_RUNTIME_DIR=/run/user/$(id -u "${TEST_USER}") systemctl --user is-enabled amethystora-connect.service" 2>/dev/null)" == enabled ]]; }
check "8 Enter turns phone pairing on within 5 s" soon 5 enabled_now
offers_off() { close_card; search "phone pairing"; [[ "$(q '[.sections[] | select(.id == "amethystora-settings")][0].rows[0].meta')" == Off ]]; }
check "8 the row then offers Off (the list read again)" wait_for 30 offers_off
close_card
in_session "ame desktop phone off" >/dev/null 2>&1
search "start over"
start_over_in() { [[ "$(section_rows amethystora-settings)" == *"Start over"* ]]; }
soon 30 start_over_in
place="$(q '[.sections[].rows[]] | map(.title) | index("Start over")')"
if [[ "${place}" =~ ^[0-9]+$ ]] && ((place < 9)); then
    keys "ctrl+$((place + 1))"
    powerwash() { pgrep -af "amethystora-in-terminal -- ame system powerwash" >/dev/null; }
    check "8 Ctrl+$((place + 1)) opens Start over in a terminal" soon 10 powerwash
    pkill -f "ame system powerwash" 2>/dev/null
else
    not_ok "8 no Start over row" "$(snap)"
fi
# A setting whose command Control fills in (a slider's {value}) is never a row Enter runs, only Control › …
search "transition length"
same "8 a slider is only offered as Control › …" \
    "$(q '[.sections[] | select(.id == "amethystora-settings")][0].rows // [] | map(select(.title | startswith("Control ›") | not)) | length')" 0
close_card
# An app's name that only a setting's sentence shares: Enter is the app's, and no setting comes before it
search "firefox"
same "8 a setting's sentence does not come before an app" \
    "$(q '[.sections[] | select(.id == "amethystora-settings")][0].rows // [] | length')" 0
close_card

# --- 9-10: open windows, and Ctrl+N -------------------------------------------------------------------------------------
user_bus systemd-run --user --collect --unit="vmlab-16-a" kitty --title "vmlab-16-a" >/dev/null
user_bus systemd-run --user --collect --unit="vmlab-16-b" kitty --title "vmlab-16-b" >/dev/null
user_bus systemd-run --user --collect --unit="vmlab-16-w" kitty --title "vmlab-16-window-${RUN}" >/dev/null
windows_up() { [[ "$(ev "global.display.get_tab_list(0, null).filter(w => w.get_title()?.startsWith('vmlab-16')).length")" == 3 ]]; }
check "9 three kitty windows" wait_for 20 windows_up
ev "(() => { const w = global.display.get_tab_list(0, null).find(w => w.get_title() === 'vmlab-16-window-${RUN}'); w.change_workspace_by_index(1, false); global.workspace_manager.get_workspace_by_index(0).activate(global.get_current_time()); return 'moved'; })()" >/dev/null
on_first() { [[ "$(ev "global.workspace_manager.get_active_workspace_index()")" == 0 ]]; }
soon 3 on_first
search "vmlab-16-window"
same "9 its row" "$(q '[.sections[] | select(.id == "amethystora-windows")][0].rows[0] | .title + " / " + .meta + " / " + .hint')" \
    "vmlab-16-window-${RUN} / $(ev "Meta.prefs_get_workspace_name(1)") / Enter switches"
keys return
switched() { [[ "$(ev "global.workspace_manager.get_active_workspace_index() + ' ' + global.display.focus_window?.get_title()")" == "1 vmlab-16-window-${RUN}" ]]; }
check "9 Enter switches to it, on its workspace" soon 5 switched
ev "global.workspace_manager.get_workspace_by_index(0).activate(global.get_current_time()); 'back'" >/dev/null
soon 3 on_first
search "vmlab-16"
second="$(q '[.sections[].rows[].title][1]')"
keys ctrl+2
focused_second() { [[ "$(ev "global.display.focus_window?.get_title()")" == "${second}" ]]; }
check "10 Ctrl+2 opens the second result (${second})" soon 5 focused_second
user_bus systemctl --user stop 'vmlab-16-*' >/dev/null 2>&1

# --- 11: the manual, with the agentic features off ------------------------------------------------------------------
as_user "amethystora-agent off" >/dev/null 2>&1
search "rolling back"
check "11 the manual's section answers rolling back" test -n "$(section_rows amethystora-manual)"
same "11 its first row" "$(q '[.sections[] | select(.id == "amethystora-manual")][0].rows[0].title')" "Updates › Rolling back"
search "why does my screen turn yellow at night"
same "11 the spec's question finds Night Light" "$(q '[.sections[] | select(.id == "amethystora-manual")][0].rows[0].title')" \
    "Getting around › Night Light and the screen's colour"
search "rolling back"
place="$(q '[.sections[].rows[]] | map(.title) | index("Updates › Rolling back")')"
manual_open() { [[ "$(ev "global.display.get_tab_list(0, null).some(w => w.get_wm_class() === 'org.amethystora.Manual')")" == true ]]; }
if [[ "${place}" =~ ^[0-9]+$ ]] && ((place < 9)); then
    open_before="$(q .open)"
    keys "ctrl+$((place + 1))"
    if wait_for 30 manual_open; then
        ok "11 Ctrl+N opens the Manual"
    else
        not_ok "11 Ctrl+N opens the Manual" "card open before the key: ${open_before}" "$(shell_state)" \
            "$(pgrep -af amethystora-manual || echo 'no amethystora-manual process')"
    fi
else
    not_ok "11 Ctrl+N opens the Manual" "Updates › Rolling back is result '${place}', not one of the nine" "$(shell_state)"
fi
ev "global.display.get_tab_list(0, null).filter(w => w.get_wm_class() === 'org.amethystora.Manual').forEach(w => w.delete(0)); 'closed'" >/dev/null

# --- 12: clip: ------------------------------------------------------------------------------------------------------
# wl-copy takes the clipboard only once a surface of its own gets the keyboard, which nothing of the shell may hold then
close_card
user_bus systemd-run --user --quiet --collect --unit=vmlab-16-clip -- wl-copy --foreground "vmlab-clip-${RUN}"
recorded() { [[ "$(ev "Main.extensionManager.lookup('amethystora-clipboard@arsslensoft.github.io')?.stateObj?.history?.().some(i => i.text === 'vmlab-clip-${RUN}')")" == true ]]; }
if soon 10 recorded; then
    ok "12 the clipboard extension recorded the copy"
else
    not_ok "12 the clipboard extension recorded the copy" "the shell's clipboard: $(clipboard)" "$(shell_state)"
fi
search "vmlab-clip"
same "12 no clipboard section without clip:" "$(q '[.sections[].id] | index("amethystora-clipboard")')" null
search "clip:"
same "12 clip: shows the history" "$(q '[.sections[] | select(.id == "amethystora-clipboard")][0].rows[0].title')" "vmlab-clip-${RUN}"
search "clip: vmlab-clip"
same "12 clip: WORDS keeps what holds them" "$(section_rows amethystora-clipboard)" "vmlab-clip-${RUN}"
close_card
user_bus systemd-run --user --collect --unit=vmlab-16-editor flatpak run org.gnome.TextEditor --new-window >/dev/null
editor_up() { user_bus python3 "${HERE}/atspi.py" gnome-text-editor has "New Document" >/dev/null 2>&1; }
editor_focused() { [[ "$(ev "global.display.focus_window?.get_wm_class()")" == *TextEditor* ]]; }
if wait_for 60 editor_up && soon 10 editor_focused; then
    search "clip: vmlab-clip"
    keys return
    pasted() { user_bus python3 "${HERE}/atspi.py" gnome-text-editor text 2>/dev/null | grep -q "vmlab-clip-${RUN}"; }
    check "12 Enter pastes it into the document" wait_for 5 pasted
else
    not_ok "12 Text Editor did not open" "focus: $(ev "global.display.focus_window?.get_wm_class() ?? 'none'")" "$(shell_state)"
fi
# The app runs in a scope of its own, which stopping flatpak run's unit leaves: a window that comes late would take the
# keyboard from the card in the next step
user_bus systemctl --user stop vmlab-16-editor >/dev/null 2>&1
user_bus flatpak kill org.gnome.TextEditor >/dev/null 2>&1
# Private mode is the clipboard menu's switch, kept in no setting
private_mode() {
    ev "(() => { const c = Main.extensionManager.lookup('amethystora-clipboard@arsslensoft.github.io').stateObj.clipboardIndicator; if (c.privateModeMenuItem.state !== $1) c.togglePrivateMode(); return String(c.privateModeMenuItem.state); })()"
}
same "12 private mode on" "$(private_mode true)" true
search "clip:"
same "12 private mode shows nothing" "$(q '[.sections[].id] | index("amethystora-clipboard")')" null
private_mode false >/dev/null
close_card

# --- 13: the agent row ----------------------------------------------------------------------------------------------
QUESTION="why is my screen yellow"
search "${QUESTION}"
same "13 no agent row while the features are off" "$(q '[.sections[].id] | index("amethystora-agent")')" null
as_user "amethystora-agent on" >/dev/null 2>&1
agent_last() { [[ "$(q '.sections[-1].id')" == amethystora-agent ]]; }
check "13 within 3 s the row is last" soon 3 agent_last
same "13 its header" "$(q '.sections[-1].title')" "THE AGENT"
same "13 its row" "$(q '.sections[-1].rows[0].title')" "Ask the agent: ${QUESTION}"
# The spec's question also finds apps (the forgiving app search takes "screen", and "is" inside "Disks"), which come
# before the manual and the agent (spec 1.1), so its row may be past the nine: the key is tried on words nothing else
# answers, which leave the agent row among the first few
ASK="zqxj vbkw"
search "${ASK}"
soon 3 agent_last
place="$(q '[.sections[].rows[]] | length')"
asked() { pgrep -af "amethystora-agent ask -- ${ASK}" >/dev/null; }
if [[ "$(q '.sections[-1].id')" == amethystora-agent && "${place}" =~ ^[0-9]+$ ]] && ((place >= 1 && place <= 9)); then
    keys "ctrl+${place}"
    check "13 Ctrl+${place} starts amethystora-agent ask -- ${ASK}" soon 5 asked
else
    not_ok "13 the agent row has a Ctrl key" "it is result '${place}'" "$(snap)" "$(shell_state)"
fi
pkill -f "amethystora-agent ask --" 2>/dev/null
pkill -f "xdg-terminal-exec .*amethystora-agent" 2>/dev/null
search "-rf"
same "13 no row for a question that starts with -" "$(q '[.sections[].id] | index("amethystora-agent")')" null
search "${QUESTION}"
as_user "amethystora-agent off" >/dev/null 2>&1
gone() { [[ "$(q '[.sections[].id] | index("amethystora-agent")')" == null ]]; }
check "13 off again: the row goes within 3 s" soon 3 gone
close_card

# --- 14: Glance, and 15 for it: off, Space shows nothing ------------------------------------------------------------
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${HOME_DIR}/Pictures"
picture="$(find /usr/share/backgrounds -name '*.png' | head -1)"
for n in 1 2; do install -o "${TEST_USER}" -g "${TEST_USER}" -m 0644 "${picture}" "${HOME_DIR}/Pictures/vmlab-glance-${n}-${RUN}.png"; done
# Fedora's indexer starts on demand (no login starts it: its unit has no [Install] and its autostart is skipped under
# systemd), as Nautilus's provider asks it; started here as that does, so that its first crawl is not the search's wait
user_bus gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
    --method org.freedesktop.DBus.StartServiceByName org.freedesktop.LocalSearch3 0 >/dev/null 2>&1
# Files (50.4) answers nothing the first time it is asked a text with a hyphen in it, as the suite sets one whole (a
# person typing reaches it through the shorter texts before), and GNOME keeps an answer for as long as the text stays the
# same: each try asks with a word the two pictures' names both hold, and another one the next time
FILES_TRIES=0
files_found() {
    local text=glance
    ((++FILES_TRIES % 2)) || text=glanc
    search "${text}"
    [[ "$(q '[.sections[].rows[].title | select(startswith("vmlab-glance"))] | length')" == 2 ]]
}
previewer() { [[ "$(ev "global.display.get_tab_list(0, null).some(w => w.get_sandboxed_app_id() === 'org.gnome.NautilusPreviewer')")" == true ]]; }
no_previewer() { ! previewer; }
on_file() { [[ "$(q '[.sections[].rows[] | select(.selected and (.title | startswith("vmlab-glance-1")))] | length')" == 1 ]]; }
to_first_file() { keys down; for _ in $(seq 12); do on_file && return 0; keys down; done; on_file; }
if wait_for 120 files_found; then
    ok "14 Files found both pictures"
    to_first_file
    same "14 the file's hint" "$(q '[.sections[].rows[] | select(.hint == "Space previews")][0].title' | cut -c1-14)" "vmlab-glance-1"
    # The hint goes with its row: a new search destroys the row, and the next file row must get a hint of its own (adding
    # the destroyed one took the shell down, on the real machine)
    search "vmlab-nothing-is-called-this"
    no_rows() { [[ "$(q '[.sections[].rows[].title | select(startswith("vmlab-glance"))] | length')" == 0 ]]; }
    check "14 another search takes the file rows away" soon 10 no_rows
    wait_for 60 files_found
    to_first_file
    same "14 ...and the file's hint comes back, on a new row" "$(q '[.sections[].rows[] | select(.hint == "Space previews")][0].title' | cut -c1-14)" "vmlab-glance-1"
    keys space
    check "14 Space opens the previewer" soon 10 previewer
    same "14 the tiling has not taken the previewer" "$(ev "global.display.get_tab_list(0, null).some(w => w.get_sandboxed_app_id() === 'org.gnome.NautilusPreviewer' && ${UI}.get('tiling').engine?.isTiled(w))")" false
    same "14 the card steps aside" "$(q '.glancing') $(q '.opacity')" "true 0"
    keys down
    on_second() { [[ "$(q '[.sections[].rows[] | select(.selected)][0].title' | cut -c1-14)" == "vmlab-glance-2" ]]; }
    check "14 Down shows the second file" soon 3 on_second
    keys space
    check "14 Space closes it" soon 5 no_previewer
    back() { [[ "$(q '.glancing') $(q '.opacity') $(q '.open')" == "false 255 true" ]]; }
    check "14 the card is back, the search kept" soon 3 back
    close_card
    gs set "${SEARCH}" glance false
    files_found
    to_first_file
    same "15 glance off: no Space previews hint" "$(q '[.sections[].rows[] | select(.hint == "Space previews")] | length')" 0
    keys space
    if soon 3 previewer; then not_ok "15 glance off: Space opened the previewer"; else ok "15 glance off: Space shows no preview"; fi
    goes_on() { [[ "$(ev "Main.overview.searchEntry.get_text()")" == *' ' ]] && card_open; }
    check "15 glance off: Space on a row goes on typing, the card stays" soon 3 goes_on
    gs set "${SEARCH}" glance true
    pkill -f loupe 2>/dev/null
else
    not_ok "14 Files did not find the pictures in 120 s" \
        "localsearch-3: $(user_bus systemctl --user is-active localsearch-3.service 2>&1)" \
        "$(in_session "localsearch status 2>&1; localsearch info ~/Pictures/vmlab-glance-1-${RUN}.png 2>&1 | head -3" 2>&1 | head -8)" \
        "sections: $(q '[.sections[].id] | join(" ")')" "$(shell_state)"
fi
close_card

# --- 15: each part off, at once -------------------------------------------------------------------------------------
gs set "${SEARCH}" answers false
search "2+3*4"
same "15 answers off: no answer" "$(q '[.sections[].id] | index("amethystora-answers")')" null
gs set "${SEARCH}" answers true
close_card
# GNOME searches only when the text changes, and the field keeps it while the card never opened: emptied first
ev "Main.overview.searchEntry.set_text(''); 'cleared'" >/dev/null
search "2+3*4"
same "15 answers on again" "$(first_section)" amethystora-answers
gs set "${SEARCH}" windows false
windows_gone() { [[ "$(ev "Main.overview.searchController._searchResults._providers.some(p => p.id === 'amethystora-windows')")" == false ]]; }
check "15 windows off: its provider is gone" soon 3 windows_gone
gs set "${SEARCH}" windows true
close_card

# --- 16: Arabic digits (the image's gjs; the session's turn comes at the end) ---------------------------------------
same "16 ar: ٢+٣" "$(calc '٢+٣' ar-EG)" "$(ev "new Intl.NumberFormat('ar-EG').format(5)")"

# --- 17: the overview -----------------------------------------------------------------------------------------------
ev "Main.overview.show(); 'shown'" >/dev/null
overview_up() { [[ "$(ev "Main.overview.visible && !Main.overview.animationInProgress")" == true ]]; }
soon 5 overview_up
ev "Main.overview.searchEntry.set_text(''); Main.overview.searchEntry.set_text('2+3*4'); 'typed'" >/dev/null
soon 10 settled
same "17 the overview's results begin with the answer" \
    "$(ev "(() => { const c = Main.overview.searchController._searchResults._content; const s = c.get_children().find(s => s.visible); return s.provider.id + ' ' + (s.getFirstResult()?.metaInfo?.name ?? s.getFirstResult()?.app?.get_name?.()); })()")" \
    "amethystora-answers 14"
ev "Main.overview.searchEntry.set_text(''); Main.overview.hide(); 'hidden'" >/dev/null
overview_gone() { [[ "$(ev "Main.overview.visible")" == false ]]; }
soon 5 overview_gone

# --- the light theme: the same card in the theme's own colours ------------------------------------------------------
search "2+3*4"
dark="$(ev "${S}._box.get_theme_node().get_foreground_color().to_string()")"
close_card
THEME_CHANGED=1
in_session "amethystora-theme set amethystora-light" >/dev/null 2>&1
recoloured() { search "2+3*4"; [[ "$(ev "${S}._box.get_theme_node().get_foreground_color().to_string()")" != "${dark}" ]]; }
check "theme: the card takes the light theme's text colour" wait_for 30 recoloured
same "theme: the answer is still there" "$(first_section) $(first_title)" "amethystora-answers 14"
close_card
lab_theme
THEME_CHANGED=""

# --- 18: ten disables and enables -----------------------------------------------------------------------------------
since="$(date '+%Y-%m-%d %H:%M:%S')"
ev "globalThis.vmlabCycles = 'running'; (async () => { for (let i = 0; i < 10; i++) { await Main.extensionManager.disableExtension('${UUID}'); await Main.extensionManager.enableExtension('${UUID}'); } globalThis.vmlabCycles = 'done'; })(); 'started'" >/dev/null
cycles_done() { [[ "$(ev "String(globalThis.vmlabCycles)")" == done ]]; }
check "18 ten cycles end" wait_for 120 cycles_done
ev "globalThis.vmlabCycles = 'off'; (async () => { await Main.extensionManager.disableExtension('${UUID}'); globalThis.vmlabCycles = 'disabled'; })(); 'off'" >/dev/null
disabled() { [[ "$(ev "String(globalThis.vmlabCycles)")" == disabled ]]; }
soon 20 disabled
same "18 disabled: GNOME's own providers only" \
    "$(ev "Main.overview.searchController._searchResults._providers.some(p => p.id.startsWith('amethystora-'))")" false
same "18 ...and its sections back in GNOME's look" \
    "$(ev "Main.overview.searchController._searchResults._content.get_children().filter(d => d.providerInfo).every(d => d.providerInfo.get_parent().orientation === imports.gi.Clutter.Orientation.HORIZONTAL && d.providerInfo.get_child().get_first_child().visible)")" true
ev "(async () => { await Main.extensionManager.enableExtension('${UUID}'); })(); 'on'" >/dev/null
check "18 enabled again" wait_for 20 search_ready
same "18 one section for each provider" \
    "$(ev "(() => { const v = Main.overview.searchController._searchResults; return v._providers.length === v._content.get_n_children(); })()")" true
if journalctl --since "${since}" -o cat _UID="$(id -u "${TEST_USER}")" | grep -q "JS ERROR\|Gjs-CRITICAL"; then
    not_ok "18 JS ERROR in the cycles" "$(journalctl --since "${since}" -o cat _UID="$(id -u "${TEST_USER}")" | grep -A3 "JS ERROR\|Gjs-CRITICAL" | head -12)"
else
    ok "18 no JS ERROR in ten cycles"
fi

# --- 19: idle -------------------------------------------------------------------------------------------------------
search "2+3*4"
close_card
sleep 60
same "19 nothing of the card is left running" "$(q .timeouts)" 0

# --- 7 and 16 in a session of their own: French numbers, then Arabic, right to left --------------------------------
# As a person sets them: French numbers as the account's region and in its user manager's environment (whichever of the
# two the next login keeps), Arabic as the account's language; each block first asks the shell what it reads
LOCALE_CHANGED=1
gs set org.gnome.system.locale region "\"'fr_FR.UTF-8'\""
user_bus systemctl --user set-environment LC_NUMERIC=fr_FR.UTF-8
if relogin && [[ "$(shell_locale)" == "fr-FR "* ]]; then
    ok "7 the shell reads French numbers"
    search "1,5 + 1"
    same "7 fr session: 1,5 + 1" "$(first_title)" "2,5"
    search $'1 234,5 * 2'
    same "7 fr session: grouped with U+202F, through the provider" "$(first_title)" "$(ev "new Intl.NumberFormat('fr-FR').format(2469)")"
    keys return
    same "7 fr session: copied without its group" "$(clipboard)" "2469"
    close_card
else
    not_ok "7 the shell reads French numbers" "expected: fr-FR (the answers' locale)" "found: $(shell_locale)"
fi
gs set org.gnome.system.locale region "\"${REGION_LAB}\""
user_bus systemctl --user unset-environment LC_NUMERIC
set_language ar_EG.UTF-8
if relogin && [[ "$(shell_locale)" == *" ar_EG"* ]]; then
    ok "16 the shell reads Arabic"
    search "٢+٣"
    same "16 ar session: ٢+٣" "$(first_title)" "$(ev "new Intl.NumberFormat('ar-EG').format(5)")"
    search "rolling back"
    same "16 ar session: a row's picture at its right" \
        "$(ev "(() => { const r = ${S}.results().find(r => r.metaInfo?.id?.startsWith('manual/')); const c = r.get_child(); const [px] = c.get_first_child().get_transformed_position(); const [tx] = c.get_child_at_index(1).get_transformed_position(); return px > tx; })()")" true
    close_card
else
    not_ok "16 the shell reads Arabic" "expected: ar_EG.UTF-8 (LC_MESSAGES)" "found: $(shell_locale)"
fi
if lab_locale; then
    ok "the lab's locale is back: ${LOCALE_LAB}"
else
    not_ok "the lab's locale is back" "expected: ${LOCALE_LAB}" "found: $(shell_locale)"
fi
LOCALE_CHANGED=""

finish
