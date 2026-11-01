#!/usr/bin/bash
# vmlab-vm: sound
# The Soft sounds, the lock screen clock and the rounded screen corners (plans/beyond/spec-look.md, section 7, items 27
# to 34), in the test account's session, asked of GNOME Shell itself through Eval, which a lab-only extension of the
# test account opens (never in the image):
#   - the sound theme is Amethystora's, with the freedesktop theme under it, and its nine files are there;
#   - a name the theme has plays our file, through libcanberra and through GNOME Shell's own sound player; a name it has
#     not (the bell) plays the freedesktop theme's; what PipeWire says of each stream is recorded;
#   - ame desktop sounds off silences both, soft brings them back;
#   - the login chord is played once a session, not again at an unlock; plugging in the power (a stand-in UPower)
#     plays power-plug, and a battery filling up does not;
#   - the lock screen's stylesheet has the clock's block once, and the locked screen shows the clock in Quicksand at
#     116 px, weight 600, now playing as a card and one app's three notifications as one row with a count, no text;
#   - the corners: none on the lab's screen, which is not built in; four, 14 px, at the screen's corners and taking no
#     clicks once the screen is taken for a built-in one; none again once off;
#   - ten disables and enables of Amethystora UI leave four corners, play no second chord, and nothing throws.
# A phone's text message (item 32) needs a phone, which the lab has not: apps/connect/tests/notifications.rs checks its
# sound-name hint over D-Bus against a stand-in notification server.
# It leaves the account as it found it: sounds soft, corners off, no notification of its own.
# vmlab-paths: @ui/extension.js @ui/*sounds* @ui/*bezel* @sf/usr/share/sounds/* @sf/usr/libexec/amethystora-login-background @sf/*just/desktop.just
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@iarsslen.github.io
HOME_DIR="/var/home/${TEST_USER}"
HERE="$(dirname "$(readlink -f "$0")")"
WORK=/run/vmlab-23
SHOTS=/run/vmlab-23
SOUNDS=/usr/share/sounds/amethystora
BEZEL=org.gnome.shell.extensions.amethystora-ui.bezel
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        sed -E -e "s/^\(true, '(.*)'\)\$/\1/" -e 's/^"(.*)"$/\1/'
}
gs() { in_session "gsettings $*" 2>/dev/null; }
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
ready() { [[ "$(ev "${UI}.has('sounds') && ${UI}.has('bezel')")" == true ]]; }
restart_session() {
    local try
    for try in 1 2; do
        loginctl unlock-sessions
        sleep 2
        systemctl restart gdm
        wait_for 240 has_session && wait_for 120 ready && break
    done
    sleep 3
    ev "Main.overview.hide(); 'hidden'" >/dev/null
}
UID_N="$(id -u "${TEST_USER}")"
DUMP="/run/user/${UID_N}/vmlab-23-streams.json"
# What PipeWire showed while COMMAND ran in the session: every media.filename of a stream that came and went, with a
# doubled slash taken out (libcanberra joins XDG_DATA_DIRS' entries, which may end in one: /usr/share//sounds/...)
streams_during() {
    in_session "rm -f ${DUMP}; (timeout 5 pw-dump -m >${DUMP} 2>/dev/null &); sleep 0.8; $*; sleep 3" >/dev/null 2>&1
    grep -o '"media.filename": "[^"]*"' "${DUMP}" 2>/dev/null | sed -e 's/.*: "\(.*\)"/\1/' -e 's|//*|/|g' | sort -u
}
# What the shell's sound player was asked to play since record() began, by name
record() { ev "global._vmlab23 = []; const p = global.display.get_sound_player(); if (!p._vmlab23) { p._vmlab23 = p.play_from_theme; p.play_from_theme = function (n, d, c) { global._vmlab23.push(n); return p._vmlab23.call(p, n, d, c); }; } 'recording'" >/dev/null; }
recorded() { ev "(global._vmlab23 ?? []).join(',')"; }
locked() { [[ "$(ev "Main.screenShield?.locked ?? false")" == true ]]; }
unlocked() { [[ "$(ev "Main.sessionMode.currentMode")" == user ]]; }
corners() { ev "Main.uiGroup.get_children().filter(c => c.constructor.name === 'AmethystoraBezelCorner').length"; }

if ! has_session; then
    bash "$(dirname "$0")/10-account.sh" >/dev/null
fi
if ! wait_for 300 has_session; then
    not_ok "no session for ${TEST_USER}"
    finish
fi
install -d -m 0755 "${WORK}"
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab"
cat >"${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab/metadata.json" <<'EOF'
{"uuid": "vmlab-eval@lab", "name": "VM lab", "description": "The lab's Eval", "shell-version": ["49", "50", "51"]}
EOF
cat >"${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab/extension.js" <<'EOF'
export default class { enable() { global.context.unsafe_mode = true; } disable() {} }
EOF
chown -R "${TEST_USER}:" "${HOME_DIR}/.local/share/gnome-shell"
LINGERED="$(loginctl show-user "${TEST_USER}" -p Linger --value 2>/dev/null)"
enabled="$(user_bus gsettings get org.gnome.shell enabled-extensions)"
[[ ${enabled} == *vmlab-eval@lab* ]] || user_bus gsettings set org.gnome.shell enabled-extensions "${enabled%]*}, 'vmlab-eval@lab']"
cleanup() {
    pkill -f upower-standin.py 2>/dev/null
    if [[ -e "${WORK}/upower-masked" ]]; then
        systemctl unmask --runtime upower.service >/dev/null 2>&1
        systemctl start upower.service >/dev/null 2>&1
    fi
    pkill -f "${WORK}/mpris.py" 2>/dev/null
    in_session "ame desktop sounds soft" >/dev/null 2>&1
    gs set "${BEZEL}" round-corners false
    ev "if (global._vmlab23Builtin) imports.gi.Meta.Monitor.prototype.is_builtin = global._vmlab23Builtin; 'restored'" >/dev/null 2>&1
    ev "Main.messageTray.getSources().filter(s => s.title === 'Lab chat').forEach(s => s.destroy()); 'gone'" >/dev/null 2>&1
    loginctl unlock-sessions >/dev/null 2>&1
    [[ "${LINGERED}" == yes ]] || loginctl disable-linger "${TEST_USER}" >/dev/null 2>&1
    rm -f "${DUMP}"
}
trap cleanup EXIT
since="$(date '+%Y-%m-%d %H:%M:%S')"

# --- 28. The theme is installed and chosen ---------------------------------------------------------------------------
gs reset org.gnome.desktop.sound theme-name
gs reset org.gnome.desktop.sound event-sounds
same "28 the sound theme is amethystora" "$(gs get org.gnome.desktop.sound theme-name)" "'amethystora'"
check "28 it falls through to the freedesktop theme" grep -qx "Inherits=freedesktop" "${SOUNDS}/index.theme"
n="$(find "${SOUNDS}/stereo" -name '*.wav' | wc -l)"
same "28 the nine sounds are there" "${n}" 9

# The login chord, looked for before anything else plays: the session is started again with a recorder of PipeWire's
# streams running in the account's manager, kept through the restart by lingering. PipeWire itself restarts with the
# session, which ends the recorder: it starts again at once, adding to the file, and so meets the new PipeWire
loginctl enable-linger "${TEST_USER}"
in_session "rm -f ${DUMP}; systemd-run --user --collect --unit=vmlab-23-dump -p Restart=always -p RestartSec=200ms \
    bash -c 'timeout 120 pw-dump -m >>${DUMP} 2>/dev/null'" >/dev/null 2>&1
restart_session
if ! ready; then
    not_ok "the sounds and bezel modules did not start in the session"
    finish
fi
sleep 6
in_session "systemctl --user stop vmlab-23-dump" >/dev/null 2>&1
# One stream, whoever played it: libcanberra's own login sound is hidden (17-cleanup.sh), so the module's is the only one
login="$(jq -r '.[]? | select(.type == "PipeWire:Interface:Node")
    | select((.info.props["media.filename"]? // "" | gsub("/+"; "/")) == "/usr/share/sounds/amethystora/stereo/desktop-login.wav") | .id' \
    "${DUMP}" 2>/dev/null | sort -u | wc -l)"
if [[ "${login}" == 1 ]]; then
    ok "31 the login chord played once as the session started"
else
    not_ok "31 the login chord played once as the session started" "streams with it: ${login}" \
        "players: $(jq -r '.[]? | select(.info.props["media.filename"]? != null) | "\(.id) \(.info.props["application.name"] // "?") \(.info.props["media.filename"])"' "${DUMP}" 2>/dev/null | sort -u | tr '\n' ';')"
fi

# --- 29. A name resolves to our file ---------------------------------------------------------------------------------
for name in message-new-instant screen-capture trash-empty dialog-error; do
    seen="$(streams_during "canberra-gtk-play -i ${name} -d test")"
    same "29 canberra plays ${name} from the Soft set" "${seen}" "${SOUNDS}/stereo/${name}.wav"
done
same "29 the bell, not in the set, plays the freedesktop theme's" "$(streams_during "canberra-gtk-play -i bell -d test")" \
    /usr/share/sounds/freedesktop/stereo/bell.oga
# GNOME plays the shutter only with the flash, the Screenshot method's second argument
seen="$(streams_during "gdbus call --session --dest org.gnome.Shell.Screenshot --object-path /org/gnome/Shell/Screenshot --method org.gnome.Shell.Screenshot.Screenshot false true /tmp/vmlab-23-shot.png")"
# GNOME 51 plays its own shutter by its file, /usr/share/gnome-shell/sounds/screen-capture.oga, whatever the sound theme
if [[ "$(gnome-shell --version | grep -oE '[0-9]+' | head -n 1)" -ge 51 ]]; then
    same "29 a screenshot plays GNOME 51's own shutter" "${seen}" /usr/share/gnome-shell/sounds/screen-capture.oga
else
    same "29 a screenshot plays screen-capture from the Soft set" "${seen}" "${SOUNDS}/stereo/screen-capture.wav"
fi
in_session "rm -f /tmp/vmlab-23-shot.png" >/dev/null 2>&1
seen="$(streams_during "gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval \"global.display.get_sound_player().play_from_theme('audio-volume-change', 'Volume changed', null)\"")"
same "29 the shell's player plays the volume tick from the Soft set" "${seen}" "${SOUNDS}/stereo/audio-volume-change.wav"
in_session "rm -f ${DUMP}; (timeout 4 pw-dump -m >${DUMP} 2>/dev/null &); sleep 0.8; canberra-gtk-play -i message-new-instant -d test; sleep 2" >/dev/null 2>&1
props="$(grep -oE '"(event.id|media.filename|application.name|canberra.xdg-theme.name)": "[^"]*"' "${DUMP}" | sort -u | paste -sd ' ')"
ok "29 what PipeWire says of a theme sound: ${props:-nothing}"

# --- 30. Off --------------------------------------------------------------------------------------------------------------
check "30 ame desktop sounds off" in_session "ame desktop sounds off"
same "30 event sounds are off" "$(gs get org.gnome.desktop.sound event-sounds)" false
same "30 canberra plays nothing" "$(streams_during "canberra-gtk-play -i message-new-instant -d test")" ""
same "30 the shell plays nothing" \
    "$(streams_during "gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval \"global.display.get_sound_player().play_from_theme('dialog-error', 'Error', null)\"")" ""
check "30 ame desktop sounds soft" in_session "ame desktop sounds soft"
same "30 event sounds are on again" "$(gs get org.gnome.desktop.sound event-sounds)" true
same "30 the theme is amethystora" "$(gs get org.gnome.desktop.sound theme-name)" "'amethystora'"
same "30 canberra plays again" "$(streams_during "canberra-gtk-play -i message-new-instant -d test")" \
    "${SOUNDS}/stereo/message-new-instant.wav"

# --- 31. Once a session, and the power -------------------------------------------------------------------------------------
record
loginctl lock-session "$(loginctl show-user "${TEST_USER}" -p Display --value)"
if wait_for 30 locked; then
    sleep 2
    loginctl unlock-sessions
    check "31 the screen unlocks" wait_for 30 unlocked
    wait_for 30 ready
    sleep 3
    same "31 no second chord after an unlock" "$(recorded)" ""
else
    not_ok "31 the screen did not lock"
fi

cp "${HERE}/upower-standin.py" "${WORK}/"
systemctl stop upower.service
systemctl mask --runtime upower.service >/dev/null && touch "${WORK}/upower-masked"
: >/run/upower-standin.log
systemd-run --collect --unit=vmlab-23-upower python3 "${WORK}/upower-standin.py" >/dev/null
sleep 2
restart_session
record
standin_pid="$(systemctl show -p MainPID --value vmlab-23-upower)"
kill -USR1 "${standin_pid}"
plugged() { [[ "$(recorded)" == power-plug ]]; }
check "31 plugging the power in plays power-plug" wait_for 6 plugged
kill -USR2 "${standin_pid}"
sleep 2
same "31 taking it out plays nothing" "$(recorded)" power-plug
kill -USR1 "${standin_pid}"
again() { [[ "$(recorded)" == power-plug,power-plug ]]; }
check "31 plugging in again plays it again" wait_for 6 again
same "31 the chord did not play with this session's power" "$(recorded | tr ',' '\n' | grep -c desktop-login)" 0

# --- 32. A phone's text message --------------------------------------------------------------------------------------------
skip "32 a phone's text message asks for message-new-instant-phone - the lab has no phone; apps/connect/tests/notifications.rs checks the hint"

# --- 33. The lock screen's stylesheet --------------------------------------------------------------------------------------
clock_rule="$(python3 -I - <<'EOF'
import gi
gi.require_version("Gio", "2.0")
from gi.repository import Gio
bundle = Gio.Resource.load("/usr/share/gnome-shell/gnome-shell-theme.gresource")
marker = "Amethystora: the lock screen clock and its cards"
out = []
for variant in ("dark", "light"):
    css = bundle.lookup_data(f"/org/gnome/shell/theme/gnome-shell-{variant}.css", 0).get_data().decode()
    clock = css[css.find(marker):]
    clock = clock[clock.find(".unlock-dialog-clock .unlock-dialog-clock-time {"):]
    clock = clock[:clock.find("}")]
    out.append("%s:%d:%s" % (variant, css.count(marker), all(line in clock for line in (
        'font-family: "Quicksand";', "font-size: 116px;", "font-weight: 600;"))))
print(" ".join(out))
EOF
)"
same "33 both stylesheets have the clock's block once, Quicksand at 116 px and 600" "${clock_rule}" "dark:1:True light:1:True"

# --- 34. The lock screen shows it -------------------------------------------------------------------------------------------
cp "${HERE}/mpris.py" "${WORK}/"
chmod 0755 "${WORK}"
in_session "systemd-run --user --collect --unit=vmlab-23-mpris python3 ${WORK}/mpris.py" >/dev/null 2>&1
# The three from one client, a second apart, as one chat app sends them: GNOME 50 looks a sender up before it makes its
# source, so three notify-send, or three calls at once, can each get a row of their own
in_session "python3 -c 'import time; from gi.repository import Gio, GLib
bus = Gio.bus_get_sync(Gio.BusType.SESSION)
for i in (1, 2, 3):
    bus.call_sync(\"org.freedesktop.Notifications\", \"/org/freedesktop/Notifications\", \"org.freedesktop.Notifications\", \"Notify\",
        GLib.Variant(\"(susssasa{sv}i)\", (\"Lab chat\", 0, \"\", \"Sam\", \"Secret %d\" % i, [], {}, -1)), None, Gio.DBusCallFlags.NONE, 5000, None); time.sleep(1)'" >/dev/null 2>&1
sleep 2
loginctl lock-session "$(loginctl show-user "${TEST_USER}" -p Display --value)"
if wait_for 30 locked; then
    sleep 3
    T="Main.screenShield._dialog._clock._time"
    same "34 the clock is Quicksand, 116 px, weight 600" \
        "$(ev "(() => { const f = ${T}.get_theme_node().get_font(); return [f.get_family(), f.get_size() / 1024, f.get_size_is_absolute(), f.get_weight()].join(' '); })()")" \
        "Quicksand 116 true 600"
    same "34 the date is Quicksand, 20 px" \
        "$(ev "(() => { const f = Main.screenShield._dialog._clock._date.get_theme_node().get_font(); return f.get_family() + ' ' + f.get_size() / 1024; })()")" \
        "Quicksand 20"
    BOX="Main.screenShield._dialog._notificationsBox._notificationBox"
    rows="$(ev "${BOX}.get_children().filter(c => c.visible).map(c => c.style_class + ':' + c.get_theme_node().get_border_radius(0)).join(' ')")"
    [[ " ${rows} " == *" unlock-dialog-notification-source:999 "* ]] && ok "34 the app's notifications are one pill (${rows})" ||
        not_ok "34 the app's notifications are one pill" "rows: ${rows}"
    [[ " ${rows} " == *" message"*":22 "* ]] && ok "34 what is playing is a card of radius 22" ||
        not_ok "34 what is playing is a card of radius 22" "rows: ${rows}"
    texts="$(ev "(() => { const all = []; const walk = a => { if (a.text !== undefined && a.visible) all.push(a.text); a.get_children().forEach(walk); }; walk(${BOX}); return all.join('|'); })()")"
    # GNOME counts the ones not seen yet: the first came up as a banner before the lock, so 2 or 3
    [[ "${texts}" == *"Lab chat"* && "|${texts}|" =~ \|[23]\| && "${texts}" != *Secret* ]] &&
        ok "34 the row names the app and the count, never the text (${texts})" ||
        not_ok "34 the row names the app and the count, never the text" "shown: ${texts}"
    user_bus gdbus call --session --dest org.gnome.Shell.Screenshot --object-path /org/gnome/Shell/Screenshot \
        --method org.gnome.Shell.Screenshot.Screenshot false false "/tmp/vmlab-23-lock.png" >/dev/null 2>&1 &&
        mv "/tmp/vmlab-23-lock.png" "${SHOTS}/lock.png" 2>/dev/null
    [[ -s "${SHOTS}/lock.png" ]] && ok "34 the locked screen is in ${SHOTS}/lock.png, to look at" ||
        skip "34 no screenshot of the locked screen"
    loginctl unlock-sessions
    check "34 the screen unlocks" wait_for 30 unlocked
else
    not_ok "34 the screen did not lock"
fi
in_session "systemctl --user stop vmlab-23-mpris" >/dev/null 2>&1
wait_for 30 ready

# --- 27. Rounded screen corners ------------------------------------------------------------------------------------------
gs set "${BEZEL}" round-corners false
out="$(in_session "ame desktop screen-corners on" 2>&1)"
[[ "${out}" == *"no screen of its own"* ]] && ok "27 on a screen that is not built in, ame says so" ||
    not_ok "27 on a screen that is not built in, ame says so" "${out}"
same "27 and no corner is drawn" "$(corners)" 0
ev "global._vmlab23Builtin = imports.gi.Meta.Monitor.prototype.is_builtin; imports.gi.Meta.Monitor.prototype.is_builtin = function () { return true; }; 'stubbed'" >/dev/null
gs set "${BEZEL}" round-corners false
gs set "${BEZEL}" round-corners true
sleep 1
same "27 with the screen taken for a built-in one, four corners" "$(corners)" 4
geo="$(ev "(() => { const s = imports.gi.St.ThemeContext.get_for_stage(global.stage).scale_factor; const m = Main.layoutManager.monitors[0]; const r = 14 * s;
    const want = [[m.x, m.y], [m.x + m.width - r, m.y], [m.x, m.y + m.height - r], [m.x + m.width - r, m.y + m.height - r]];
    const cs = Main.uiGroup.get_children().filter(c => c.constructor.name === 'AmethystoraBezelCorner');
    return cs.every((c, i) => c.x === want[i][0] && c.y === want[i][1] && c.width === r && c.height === r && !c.reactive); })()")"
same "27 each 14 px times the scale, at a corner of the screen, taking no clicks" "${geo}" true
same "27 above everything the shell draws" \
    "$(ev "Main.uiGroup.get_children().slice(-4).every(c => c.constructor.name === 'AmethystoraBezelCorner')")" true
user_bus gdbus call --session --dest org.gnome.Shell.Screenshot --object-path /org/gnome/Shell/Screenshot \
    --method org.gnome.Shell.Screenshot.Screenshot false false "/tmp/vmlab-23-corners.png" >/dev/null 2>&1 &&
    mv "/tmp/vmlab-23-corners.png" "${SHOTS}/corners.png" 2>/dev/null
pixels="$(python3 -I - "${SHOTS}/corners.png" <<'EOF' 2>&1
import sys, gi
gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf
p = GdkPixbuf.Pixbuf.new_from_file(sys.argv[1])
rs, n, px = p.get_rowstride(), p.get_n_channels(), p.get_pixels()
at = lambda x, y: "#%02x%02x%02x" % tuple(px[y * rs + x * n:y * rs + x * n + 3])
print(at(0, 0), at(20, 20) != "#000000")
EOF
)"
same "27 the corner pixel is black and one inside the curve is not" "${pixels}" "#000000 True"
record
for i in $(seq 10); do
    ev "Main.extensionManager.disableExtension('${UUID}'); Main.extensionManager.enableExtension('${UUID}'); 'cycled'" >/dev/null
done
# Sent back to back on purpose; GNOME may leave it off after the last one, so it is turned on again before the count
ev "Main.extensionManager.enableExtension('${UUID}'); 1" >/dev/null
wait_for 30 ready
same "27 ten disables and enables leave four corners" "$(corners)" 4
sleep 2
same "31 and play no second chord" "$(recorded)" ""
check "27 ame desktop screen-corners off" in_session "ame desktop screen-corners off"
same "27 off, no corner is left" "$(corners)" 0

# --- Nothing of Amethystora UI threw meanwhile -----------------------------------------------------------------------------
errors="$(journalctl --since "${since}" _UID="${UID_N}" 2>/dev/null | grep -E "JS ERROR|Amethystora UI: the (sounds|bezel) module" | grep -iE "sounds|bezel" | head -5)"
[[ -z "${errors}" ]] && ok "no error from the sounds or bezel module" || not_ok "an error from the sounds or bezel module" "${errors}"

finish
