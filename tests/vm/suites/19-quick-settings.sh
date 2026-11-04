#!/usr/bin/bash
# vmlab-vm: quick
# Quick settings as live tiles (plans/beyond/spec-quick.md, section 7, 19-quick-settings items 1 to 12 and 23), in the
# test account's session, asked of GNOME Shell itself through Eval, which a lab-only extension of the test account
# opens (never in the image):
#   - the module runs: tiles first, then the output slider, the theme block, Screenshot, Lock, Settings and Power, and
#     the hint; tiles 184 wide, the menu 408;
#   - a long press starts arrange mode and toggles nothing; a tile hidden from its corner button and another dragged
#     before power mode stay so across a new session, and a drag let go outside the tiles changes nothing; reset puts
#     them back;
#   - the theme block lists the image's themes, marks the current one, switches with `ame desktop theme`, refuses a
#     name it did not list, and hides and comes back with `ame desktop quick-settings theme off|on`;
#   - an app playing a tone has its own slider and mute (pactl reads them back), and the output picker sends it to
#     another output; an event sound makes no row;
#   - without a battery there is no battery tile; with a stand-in UPower (upower-standin.py) the tile says the charge
#     and when it runs out, and "Keep the battery at 80%" calls UPower's own method;
#   - ten disables and enables, the first in the middle of a drag, leave GNOME's quick settings as GNOME made them.
# The lab draws without a GPU, where GNOME turns animations off: arrange mode's shrinking and making room are run with
# animations forced on (global.force_animations), put back however the suite ends.
# The Wi-Fi strength and a Bluetooth device's battery need hardware the lab has not.
# vmlab-paths: @ui/extension.js @ui/*quick* @sf/*themed/shell-quick* @sf/*just/desktop.just @ui/*power* @ui/modules/tools/reach* @ui/modules/power/slept* @sf/usr/libexec/amethystora-sleep-report @sf/usr/share/polkit-1/actions/org.amethystora.wake.policy
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HOME_DIR="/var/home/${TEST_USER}"
QUICK=org.gnome.shell.extensions.amethystora-ui.quick
HERE="$(dirname "$(readlink -f "$0")")"
WORK=/run/vmlab-19
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        sed -E -e "s/^\(true, '(.*)'\)\$/\1/" -e 's/^"(.*)"$/\1/'
}
gs() { in_session "gsettings $*" 2>/dev/null; }
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
Q="${UI}.get('quick')"
QS="Main.panel.statusArea.quickSettings"
shell_click() { user_bus python3 "${HERE}/atspi.py" gnome-shell click "$@" >/dev/null 2>&1; }
keys() { python3 "${HERE}/keys.py" "$@" >/dev/null 2>&1; }
# The middle of an actor, as keys.py's fractions of the screen; with a third argument, that share of its width
where() {
    ev "(() => { const a = $1; const [x, y] = a.get_transformed_position(); const [w, h] = a.get_transformed_size();
        return ((x + w * ${2:-0.5}) / global.stage.width).toFixed(4) + ',' + ((y + h / 2) / global.stage.height).toFixed(4); })()"
}
tile() { echo "${Q}.tiles().find(t => ${Q}.idOf(t) === '$1')"; }
pa() { in_session "pactl $*" 2>/dev/null; }
open_menu() { ev "${QS}.menu.open(); 'open'" >/dev/null; sleep 1; }
ids() { ev "${Q}.tileIds().join(',')"; }
quick_ready() { [[ "$(ev "typeof ${Q}?.tileIds")" == function ]]; }
restart_session() {
    local try
    for try in 1 2; do
        loginctl unlock-sessions
        sleep 2
        systemctl restart gdm
        wait_for 240 has_session && wait_for 120 quick_ready && break
    done
    sleep 8
    ev "Main.overview.hide(); 'hidden'" >/dev/null
}
since="$(date '+%Y-%m-%d %H:%M:%S')"

if ! has_session; then
    bash "$(dirname "$0")/10-account.sh" >/dev/null
fi
if ! wait_for 300 has_session; then
    not_ok "no session for ${TEST_USER}"
    finish
fi
gs reset "${QUICK}" tile-order
gs reset "${QUICK}" tile-hidden

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
restart_session
if ! quick_ready; then
    not_ok "the quick module did not start in the session"
    finish
fi
install -d -m 0755 "${WORK}"
cleanup() {
    pkill -f upower-standin.py 2>/dev/null
    if [[ -e "${WORK}/upower-masked" ]]; then
        systemctl unmask --runtime upower.service >/dev/null 2>&1
        systemctl start upower.service >/dev/null 2>&1
    fi
    in_session "pkill -x paplay" >/dev/null 2>&1
    [[ -n "${LAB2:-}" ]] && pa unload-module "${LAB2}" >/dev/null
    [[ -n "${LAB3:-}" ]] && pa unload-module "${LAB3}" >/dev/null
    [[ -n "${SINK:-}" ]] && pa set-default-sink "${SINK}" >/dev/null
    gs reset "${QUICK}" tile-order
    gs reset "${QUICK}" tile-hidden
    ev "global.force_animations = ${FORCED:-false}; 'restored'" >/dev/null 2>&1
}
trap cleanup EXIT
FORCED="$(ev "global.force_animations")"
[[ "${FORCED}" == true ]] || FORCED=false

# --- 1. The module runs ---------------------------------------------------------------------------------------------
same "1 the quick module runs" "$(ev "${UI}.has('quick')")" true

# --- 2, 3. Layout and sizes ----------------------------------------------------------------------------------------------
open_menu
tiles="$(ids)"
for id in wired dark-style night-light dnd power-mode; do
    # GNOME shows Night Light only where Mutter can change the gamma, which the lab's virtual screen cannot
    if [[ ${id} == night-light && "$(ev "String(global.backend.get_monitor_manager().night_light_supported)")" == false ]]; then
        skip "2 the night-light tile - the lab's screen has no gamma, so GNOME hides it"
        continue
    fi
    [[ ",${tiles}," == *",${id},"* ]] && ok "2 the ${id} tile is there" || not_ok "2 the ${id} tile is there" "tiles: ${tiles}"
done
[[ ",${tiles}," != *",battery,"* ]] && ok "2 no battery tile without a battery" || not_ok "2 no battery tile" "tiles: ${tiles}"
same "2 the grid starts with a tile" "$(ev "(() => { const g = ${QS}.menu._grid; const first = g.get_children().find(c => c.visible); return ${Q}.tiles().includes(first); })()")" true
LAST="(() => { const g = ${QS}.menu._grid; const shown = g.get_children().filter(c => c.visible && c !== g.layout_manager._overlay
    && c.constructor.name !== 'BackgroundAppsToggle');
    return shown.slice(-4).map(c => c.style_class?.includes('amethystora-quick-theme') ? 'theme'
        : c.style_class?.includes('amethystora-quick-hint') ? 'hint' : c.constructor.name).join(','); })()"
same "2 under the tiles: the volume, the theme block, the row of buttons and the hint" "$(ev "${LAST}")" \
    "OutputStreamSlider,theme,SystemItem,hint"
ACTIONS="(() => { const item = ${QS}.menu._grid.get_children().find(c => c.constructor.name === 'SystemItem');
    return item.child.get_children().filter(c => c.visible).map(b => b.get_child().get_children().at(-1).text).join(','); })()"
same "2 the row of buttons" "$(ev "${ACTIONS}")" "Screenshot,Lock,Settings,Power"
same "3 every tile is 184 wide and at least 56 high" \
    "$(ev "${Q}.tiles().every(t => Math.round(t.width) === 184 && Math.round(t.height) >= 56)")" true
width="$(ev "Math.round(${QS}.menu.box.width)")"
((width >= 406 && width <= 410)) && ok "3 the menu is 408 wide" || not_ok "3 the menu is 408 wide" "found: ${width}"

# --- 4. Subtitles ------------------------------------------------------------------------------------------------------
power="$(ev "${Q}.tiles().find(t => ${Q}.idOf(t) === 'power-mode')?.subtitle ?? ''")"
[[ -n "${power}" ]] && ok "4 the power mode tile says the mode (${power})" || not_ok "4 the power mode tile says the mode"
skip "4 the Wi-Fi strength and a Bluetooth device's battery - no Wi-Fi or Bluetooth in the lab"

# --- 5. Arrange mode from a long press, and hiding a tile, across a new session -----------------------------------------
ev "global.force_animations = true; 'forced'" >/dev/null
was="$(ev "$(tile dark-style).checked")"
at="$(where "$(tile dark-style)")"
keys "move:${at}" btn:down sleep:900 btn:up
arranging() { [[ "$(ev "${Q}._arrange.active")" == true ]]; }
check "5 holding a tile starts arrange mode" wait_for 3 arranging
same "5 the press that started it toggled nothing" "$(ev "$(tile dark-style).checked")" "${was}"
same "5 the tile held has the keyboard" "$(ev "$(tile dark-style) === [...${Q}._arrange._handles.values()].find(e => e.handle.has_key_focus())?.actor")" true
shrunk() { [[ "$(ev "${Q}.tiles().every(t => Math.abs(t.scale_x - 0.96) < 0.001)")" == true ]]; }
check "5 every tile shrinks a little" wait_for 2 shrunk
if shell_click "Hide Do Not Disturb"; then
    ok "5 the corner button of Do Not Disturb, through AT-SPI"
else
    skip "5 AT-SPI did not reach the corner button: hidden through the module's own path"
    ev "${Q}.hideForTest('dnd')" >/dev/null
fi
hidden_dnd() { [[ "$(gs get "${QUICK}" tile-hidden)" == "['dnd']" ]]; }
check "5 tile-hidden is ['dnd']" wait_for 5 hidden_dnd
[[ ",$(ids)," != *",dnd,"* ]] && ok "5 Do Not Disturb left the tiles" || not_ok "5 Do Not Disturb left the tiles" "$(ids)"
same "5 the Hidden row holds it while arranging" "$(ev "${Q}.arrangeable().some(a => ${Q}.hiddenActor(a) && ${Q}.idOf(a) === 'dnd')")" true
# Tab goes from handle to handle, and so to the tile put away. The menu keeps the keyboard wherever it goes
hidden_focused() { [[ "$(ev "[...${Q}._arrange._handles.values()].some(e => e.hidden && e.handle.has_key_focus())")" == true ]]; }
for ((i = 0; i < 40; i++)); do
    hidden_focused && break
    keys tab
done
check "5 Tab reaches the tile put away" hidden_focused

# --- 6. Moving a tile, against GNOME's order (power mode comes before dark style), by dragging it -------------------
[[ ",$(ids)," == *",power-mode,"*"dark-style,"* ]] && ok "6 GNOME has power mode before dark style" ||
    not_ok "6 GNOME has power mode before dark style" "$(ids)"
settled() { [[ "$(ev "${Q}.tiles().every(t => t.translation_x === 0 && t.translation_y === 0 && t.opacity === 255)")" == true ]]; }
# Let go outside every tile: the tiles slide back and nothing is written
from="$(where "$(tile night-light)")"
keys "move:${from}" btn:down "glide:${from},0.3,0.5,600" sleep:300 btn:up
check "6 let go outside the tiles: no tile is left moved, hidden or see-through" wait_for 3 settled
same "6 and the order is not written" "$(gs get "${QUICK}" tile-order)" "@as []"
# Dark style onto the start of power mode
from="$(where "$(tile dark-style)")"
to="$(where "$(tile power-mode)" 0.2)"
keys "move:${from}" btn:down "glide:${from},${to},700" sleep:400 btn:up
moved() { [[ "$(gs get "${QUICK}" tile-order)" == *"'dark-style', 'power-mode'"* ]]; }
if wait_for 3 moved; then
    ok "6 dragged: tile-order has dark-style before power-mode"
else
    not_ok "6 dragged: tile-order has dark-style before power-mode" "found: $(gs get "${QUICK}" tile-order)"
    ev "${Q}.moveForTest('dark-style', 'power-mode')" >/dev/null
fi
[[ ",$(ids)," == *",dark-style,power-mode,"* ]] && ok "6 and the tiles do" || not_ok "6 and the tiles do" "$(ids)"
check "6 after the drop no tile is left moved, hidden or see-through" wait_for 3 settled
same "6 no clone of the tile is left" "$(ev "${Q}._arrange._layer.get_children().filter(c => c instanceof imports.gi.Clutter.Clone).length")" 0
ev "${Q}.arrange(false)" >/dev/null
unshrunk() { [[ "$(ev "${Q}.tiles().every(t => t.scale_x === 1)")" == true ]]; }
check "6 Done: every tile at its own size again" wait_for 2 unshrunk
ev "global.force_animations = ${FORCED}; 'restored'" >/dev/null

restart_session
open_menu
[[ ",$(ids)," != *",dnd,"* ]] && ok "5 Do Not Disturb stays hidden in a new session" || not_ok "5 stays hidden" "$(ids)"
[[ ",$(ids)," == *",dark-style,power-mode,"* ]] && ok "6 the order stays in a new session" || not_ok "6 the order stays" "$(ids)"

# --- 7. Reset -------------------------------------------------------------------------------------------------------------
check "7 ame desktop quick-settings reset" in_session "ame desktop quick-settings reset"
same "7 tile-order empty" "$(gs get "${QUICK}" tile-order)" "@as []"
same "7 tile-hidden empty" "$(gs get "${QUICK}" tile-hidden)" "@as []"
[[ ",$(ids)," == *",dnd,"* ]] && ok "7 Do Not Disturb is back" || not_ok "7 Do Not Disturb is back" "$(ids)"
[[ ",$(ids)," == *",power-mode,"*"dark-style,dnd,"* ]] && ok "7 in GNOME's order" || not_ok "7 in GNOME's order" "$(ids)"

# --- 8. The theme block -----------------------------------------------------------------------------------------------
open_menu
swatches() { ev "${Q}.themeSwatches().map(s => s.slug).join(' ')"; }
has_swatches() { [[ -n "$(swatches)" ]]; }
wait_for 5 has_swatches
user_themes="$(ls "${HOME_DIR}/.config/amethystora/themes" 2>/dev/null)"
if [[ -z "${user_themes}" ]]; then
    same "8 a swatch for every theme of the image" "$(swatches)" "$(ls /usr/share/amethystora/themes | sort | paste -sd ' ')"
else
    skip "8 the account has themes of its own: the list is not the image's alone"
fi
current="$(cat "${HOME_DIR}/.config/amethystora/current/theme.name")"
same "8 the current theme is marked" "$(ev "${Q}.themeSwatches().filter(s => s.marked).map(s => s.slug).join()")" "${current}"
same "8 a name it did not list is refused" "$(ev "${Q}.pickTheme('../x')")" false
same "8 a tap on Nord" "$(ev "${Q}.pickTheme('nord')")" true
is_nord() { [[ "$(cat "${HOME_DIR}/.config/amethystora/current/theme.name")" == nord ]]; }
check "8 the theme is Nord within 30 s" wait_for 30 is_nord
same "8 Nord is dark" "$(gs get org.gnome.desktop.interface color-scheme)" "'prefer-dark'"
nord_marked() { [[ "$(ev "${Q}.themeSwatches().filter(s => s.marked).map(s => s.slug).join()")" == nord ]]; }
check "8 Nord is marked" wait_for 10 nord_marked
ev "${Q}.pickTheme('${current}')" >/dev/null
back() { [[ "$(cat "${HOME_DIR}/.config/amethystora/current/theme.name")" == "${current}" ]]; }
check "8 and back to ${current}" wait_for 30 back
if journalctl -b --since "${since}" --no-pager -o cat 2>/dev/null | grep -q "desktop theme \.\./x"; then
    not_ok "8 nothing ran for the refused name"
else
    ok "8 nothing ran for the refused name"
fi
theme_in_grid() { [[ "$(ev "${Q}._theme.block.get_parent() === ${QS}.menu._grid")" == "$1" ]]; }
check "8 ame desktop quick-settings theme off" in_session "ame desktop quick-settings theme off"
check "8 the theme block is gone" wait_for 3 theme_in_grid false
check "8 ame desktop quick-settings theme on" in_session "ame desktop quick-settings theme on"
check "8 the theme block is back" wait_for 3 theme_in_grid true

# --- 9, 10. Each app's volume and the output -----------------------------------------------------------------------------
if ! in_session "command -v paplay && command -v pactl" >/dev/null; then
    skip "9 each app's volume - no paplay or pactl"
else
    # A minute of a 440 Hz tone, as a 16-bit WAV the account can play
    cat >"${WORK}/tone.py" <<'EOF'
import math, struct, wave
w = wave.open("/tmp/vmlab-tone.wav", "w")
w.setnchannels(2)
w.setsampwidth(2)
w.setframerate(44100)
w.writeframes(b"".join(struct.pack("<hh", s, s) for s in (int(6000 * math.sin(2 * math.pi * 440 * i / 44100)) for i in range(44100 * 60))))
w.close()
EOF
    in_session "python3 ${WORK}/tone.py"
    tone() {
        user_bus systemd-run --user --collect --unit="vmlab-tone-$1" paplay --property=application.name='Lab tone' \
            --property=application.id=lab.tone /tmp/vmlab-tone.wav >/dev/null
    }
    ev "${QS}.menu.close(); 'closed'" >/dev/null
    tone 1
    sleep 1
    open_menu
    row() { [[ ",$(ev "${Q}.appRows().join(',')")," == *",Lab tone,"* ]]; }
    check "9 the tone has a row of its own" wait_for 5 row
    same "9 its slider to half" "$(ev "${Q}.setAppRowVolume('Lab tone', 0.5)")" true
    sleep 1
    vols="$(pa -f json list sink-inputs | jq -r '.[] | select(.properties["application.name"] == "Lab tone") | .volume[].value_percent' | tr -d '%' | paste -sd ' ')"
    half=true
    for v in ${vols}; do ((v >= 48 && v <= 52)) || half=false; done
    [[ -n "${vols}" && ${half} == true ]] && ok "9 pactl says 50% on every channel" || not_ok "9 pactl says 50%" "found: ${vols}"
    if shell_click "Mute Lab tone"; then ok "9 its mute button, through AT-SPI"; else ev "${Q}.muteForTest('Lab tone')" >/dev/null; skip "9 AT-SPI did not reach the mute button: muted through the module's own path"; fi
    muted() { [[ "$(pa -f json list sink-inputs | jq -r '.[] | select(.properties["application.name"] == "Lab tone") | .mute')" == true ]]; }
    check "9 pactl says muted" wait_for 3 muted
    user_bus systemctl --user stop vmlab-tone-1 >/dev/null 2>&1
    norow() { [[ "$(ev "${Q}.appRows().length + ' ' + ${Q}._volume.block.visible")" == "0 false" ]]; }
    check "9 the row and the block go with the stream" wait_for 2 norow
    if in_session "command -v canberra-gtk-play" >/dev/null; then
        user_bus systemd-run --user --collect --unit=vmlab-bell canberra-gtk-play -i bell -l 20 >/dev/null
        event() { [[ "$(pa -f json list sink-inputs | jq '[.[] | select(.properties["media.role"] == "event")] | length')" -gt 0 ]]; }
        if wait_for 3 event; then
            same "9 an event sound playing makes no row" "$(ev "${Q}.appRows().length")" 0
        else
            skip "9 an event sound - none played"
        fi
        user_bus systemctl --user stop vmlab-bell >/dev/null 2>&1
    else
        skip "9 an event sound - no canberra-gtk-play"
    fi

    SINK="$(pa get-default-sink)"
    LAB2="$(pa load-module module-null-sink sink_name=lab2 sink_properties=device.description=Lab2)"
    # The lab's own sink is PipeWire's dummy output, which GNOME does not list: a second stand-in, so that there are two to pick from
    LAB3="$(pa load-module module-null-sink sink_name=lab3 sink_properties=device.description=Lab3)"
    ev "${QS}.menu.close(); 'closed'" >/dev/null
    tone 2
    sleep 1
    open_menu
    outputs="$(ev "${Q}.outputs().join(',')")"
    lab2name="$(tr ',' '\n' <<<"${outputs}" | grep -m1 '^Lab2')"
    [[ -n "${lab2name}" && "$(tr ',' '\n' <<<"${outputs}" | wc -l)" -ge 2 ]] &&
        ok "10 the picker lists the outputs (${outputs})" || not_ok "10 the picker lists the outputs" "found: ${outputs}"
    same "10 Lab2 picked" "$(ev "${Q}.pickOutput('${lab2name}')")" true
    lab2() { [[ "$(pa get-default-sink)" == lab2 ]]; }
    check "10 pactl says the output is lab2" wait_for 3 lab2
    footer() { [[ "$(ev "${Q}._volume.footer()")" == "Playing through ${lab2name}." ]]; }
    check "10 the footer says Lab2" wait_for 3 footer
    user_bus systemctl --user stop vmlab-tone-2 >/dev/null 2>&1
    pa set-default-sink "${SINK}" >/dev/null
    pa unload-module "${LAB2}" >/dev/null
    pa unload-module "${LAB3}" >/dev/null
    LAB2=""
    LAB3=""
fi

# --- 11, 12. The battery ----------------------------------------------------------------------------------------------------
same "11 no 80% row without a battery" "$(ev "${Q}.batteryRow()")" null
[[ ",$(ids)," != *",battery,"* ]] && ok "11 no battery tile" || not_ok "11 no battery tile" "$(ids)"

cp "${HERE}/upower-standin.py" "${WORK}/"
standin() {
    pkill -f upower-standin.py 2>/dev/null
    : >/run/upower-standin.log
    systemd-run --collect --unit="vmlab-upower-$1" python3 "${WORK}/upower-standin.py" "${@:2}" >/dev/null
    sleep 2
}
systemctl stop upower.service
systemctl mask --runtime upower.service >/dev/null && touch "${WORK}/upower-masked"
standin a
restart_session
open_menu
same "12 the battery tile comes first" "$(ids | cut -d, -f1)" battery
subtitle="$(ev "${Q}.batterySubtitle()")"
[[ "${subtitle}" =~ ^86\ ?%\ ·\ until\ [0-9] ]] && ok "12 it says the charge and until when (${subtitle})" ||
    not_ok "12 it says the charge and until when" "found: ${subtitle}"
row="$(ev "JSON.stringify(${Q}.batteryRow())" | sed -E 's/[\\]+"/"/g')"
[[ "$(jq -r .title <<<"${row}")" =~ ^Keep\ the\ battery\ at\ 80\ ?%$ && "$(jq -r .active <<<"${row}")" == false ]] &&
    ok "12 Keep the battery at 80%, off" || not_ok "12 Keep the battery at 80%, off" "found: ${row}"
ev "${Q}._battery.tile.menu.open(); 'open'" >/dev/null
sleep 1
if shell_click "Keep the battery at"; then
    ok "12 the row, clicked through AT-SPI"
else
    skip "12 AT-SPI did not reach the row: switched through the module's own path"
    ev "${Q}.toggleBatteryLimit()" >/dev/null
fi
called() { grep -qx "EnableChargeThreshold $1" /run/upower-standin.log; }
check "12 UPower's EnableChargeThreshold true" wait_for 3 called true
same "12 the battery's menu stays open, the switch in sight" "$(ev "${Q}._battery.tile.menu.isOpen")" true
active() { [[ "$(ev "${Q}.batteryRow()?.active")" == "$1" && "$(ev "${Q}.batteryRow()?.upower")" == "$1" ]]; }
check "12 the switch is on, as UPower says" wait_for 3 active true
stops() { [[ "$(ev "${Q}.batterySubtitle()")" =~ stops\ at\ 80\ ?%\ when\ plugged\ in ]]; }
check "12 the tile says where charging stops" wait_for 3 stops
ev "${Q}.toggleBatteryLimit()" >/dev/null
check "12 and false again" wait_for 3 called false
standin r --refuse
restart_session
open_menu
ev "${Q}._battery.tile.menu.open(); 'open'" >/dev/null
sleep 1
shell_click "Keep the battery at" || ev "${Q}.toggleBatteryLimit()" >/dev/null
check "12 refused: UPower was asked" wait_for 3 called true
refused() { [[ "$(ev "JSON.stringify([${Q}.batteryRow()?.active, ${Q}.batteryRow()?.error, ${Q}._battery.tile.menu.isOpen])")" == '[false,true,true]' ]]; }
check "12 refused: the switch goes back and the menu says so" wait_for 5 refused
standin b --unsupported
restart_session
open_menu
same "12 a battery without a limit: no 80% row" "$(ev "${Q}.batteryRow()")" null
[[ ",$(ids)," == *",battery,"* ]] && ok "12 the battery tile stays" || not_ok "12 the battery tile stays" "$(ids)"
pkill -f upower-standin.py
systemctl unmask --runtime upower.service >/dev/null
rm -f "${WORK}/upower-masked"
systemctl start upower.service
restart_session

# --- 23. Ten disables and enables, the first in the middle of a drag ----------------------------------------------------
open_menu
ev "${Q}.arrange(true)" >/dev/null
sleep 1
from="$(where "$(tile dark-style)")"
to="$(where "$(tile power-mode)")"
# keys.py lets go of every button when it ends, so it holds this one down in the background until the cycle began
keys "move:${from}" btn:down "glide:${from},${to},400" sleep:20000 btn:up &
HOLD=$!
dragging() { [[ "$(ev "Boolean(${Q}?._arrange?._drag)")" == true ]]; }
check "23 a drag is under way" wait_for 10 dragging
CYCLE="(async () => { const uuid = '${UUID}'; const wait = ms => new Promise(r => setTimeout(r, ms));
    const g = ${QS}.menu._grid;
    Main.extensionManager.disableExtension(uuid); await wait(500); ${QS}.menu.close();
    const sys = g.get_children().find(c => c.constructor.name === 'SystemItem');
    const gnome = g.get_children()[0] === sys && sys.child.get_children()[0].constructor.name === 'PowerToggle'
        && !g.get_children().some(c => (c.style_class ?? '').includes('amethystora-quick'))
        && !Object.hasOwn(${QS}.menu, 'close');
    const count = () => Main.uiGroup.get_n_children();
    const before = count();
    for (let i = 0; i < 10; i++) { Main.extensionManager.enableExtension(uuid); await wait(400); Main.extensionManager.disableExtension(uuid); await wait(400); }
    const after = count();
    const walk = (a, clones) => { let n = (a.style_class ?? '').includes('amethystora-quick') || (a.name ?? '').includes('amethystora-quick')
        || (clones && a instanceof imports.gi.Clutter.Clone) ? 1 : 0;
        for (const c of a.get_children()) n += walk(c, clones); return n; };
    const left = walk(Main.uiGroup, false) + walk(${QS}.menu.actor, true);
    Main.extensionManager.enableExtension(uuid);
    globalThis.vmlabCycle = gnome + ' ' + (after - before) + ' ' + left; })(); 'started'"
ev "${CYCLE}" >/dev/null
cycle_done() { [[ -n "$(ev "globalThis.vmlabCycle ?? ''")" ]]; }
wait_for 120 cycle_done
wait "${HOLD}"
same "23 disabled: GNOME's own order and battery button; ten cycles leave nothing" "$(ev "globalThis.vmlabCycle")" "true 0 0"
wait_for 30 quick_ready
open_menu
same "23 enabled again: the same layout" "$(ev "${LAST}")" "OutputStreamSlider,theme,SystemItem,hint"
same "23 and every tile at its own size, in sight" "$(ev "${Q}.tiles().every(t => t.scale_x === 1 && t.opacity === 255 && t.translation_x === 0)")" true
ev "${QS}.menu.close(); 'closed'" >/dev/null

errors="$(journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --since "${since}" --no-pager -o cat 2>/dev/null |
    grep -E "JS ERROR|Amethystora UI: the quick module" | head -n 20)"
if [[ -z ${errors} ]]; then ok "1 nothing of quick settings threw"; else not_ok "1 quick settings threw" "${errors}"; fi
finish
