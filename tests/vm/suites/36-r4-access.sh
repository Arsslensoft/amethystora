#!/usr/bin/bash
# vmlab-vm: r4-access
# Seeing and reading, touch and pen (plans/beyond/spec-r4-access.md, section 11), in the test account's session, asked
# of GNOME Shell itself through Eval, which a lab-only extension of the test account opens (never in the image), with
# keys and clicks from keys.py and touches and clicks from a pointer or touch screen of the lab's own:
#   - both schemas as shipped, and nothing made or bound while everything is off;
#   - what the screen really shows with a colour filter on, read from QEMU's screendump between phases (screen_next):
#     red and green filtered once, also where a square was drawn and moved (partial redraws), through Zoom's
#     magnified picture, and a screen marked HDR left unfiltered;
#   - a colour filter: the screen filtered as the published matrices say; the screenshot UI (Print) and GNOME's
#     screenshot service saving the apps' own colours with a filter on; the strength; the tile's switch; direct
#     scanout held off while on (counted on the compositor); none takes it all away;
#   - the reading ruler: band, dim and underline, following the pointer and the focused window, as tall as the text,
#     Super+Ctrl+R pressed;
#   - reading aloud: spd-say for each sentence, the sentence on its standard input and never its command line, the
#     pill with the speed and Stop;
#   - touch mode: the bar, the dock, the menus, the handle, and a swipe in from the right edge opening Today only while
#     touch mode is on, following a stood-in hinge;
#   - the pen's tools: Super+Ctrl+P, the ring with animations forced on, the strip, lighting and running a tool,
#     Escape and a click outside, a right click and the back button where it opened;
#   - the pen's button set up and given back in GNOME's stylus settings;
#   - Focus's grey through the filter's one effect with no filter chosen;
#   - ten disables and enables with everything on, Control and the setup file.
# Phases 1 to 3 end with continue_phase, so that the shell keeps its session from one to the next.
# vmlab-paths: @ui/extension.js @ui/*access* @ui/*touch* @ui/*quick* @ui/*pill* @ui/*screenshot* @ui/*dock* @sf/*themed/shell-access* @sf/*themed/shell-touch* @sf/usr/libexec/amethystora-control-list @sf/*just/desktop.just
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HOME_DIR="/var/home/${TEST_USER}"
ACCESS=org.gnome.shell.extensions.amethystora-ui.access
TOUCH=org.gnome.shell.extensions.amethystora-ui.touch
STYLUS=org.gnome.desktop.peripherals.tablet.stylus
PEN=/org/gnome/desktop/peripherals/stylus/default-056a:00ff/
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
keys() { python3 "$(dirname "$0")/keys.py" "$@"; }
like() { [[ "$1" == $2 ]]; }
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
# A JavaScript expression that is true, asked again on each round of wait_for
is() { [[ "$(ev "$1")" == true ]]; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
A="${UI}.get('access')"
T="${UI}.get('touch')"
# A field of a module's state, as JSON
ast() { ev "JSON.stringify(${A}.state()$1)"; }
tst() { ev "JSON.stringify(${T}.state()$1)"; }
# The pointer and a touch screen of the lab's own, for motion and touches at exact points
MOVE="globalThis.vmlabPointer ??= global.stage.context.get_backend().get_default_seat()
    .create_virtual_device(imports.gi.Clutter.InputDeviceType.POINTER_DEVICE);
    const move = (x, y) => globalThis.vmlabPointer.notify_absolute_motion(imports.gi.GLib.get_monotonic_time(), x, y);"
# A swipe in from the right edge of the primary screen, by a touch screen made for it and dropped after
SWIPE="(async () => { const C = imports.gi.Clutter, G = imports.gi.GLib;
    const d = global.stage.context.get_backend().get_default_seat().create_virtual_device(C.InputDeviceType.TOUCHSCREEN_DEVICE);
    const m = Main.layoutManager.primaryMonitor, y = m.y + m.height / 2, w = ms => new Promise(r => setTimeout(r, ms));
    d.notify_touch_down(G.get_monotonic_time(), 0, m.x + m.width - 1, y); await w(30);
    for (let i = 1; i <= 12; i++) { d.notify_touch_motion(G.get_monotonic_time(), 0, m.x + m.width - 1 - i * 25, y); await w(16); }
    d.notify_touch_up(G.get_monotonic_time(), 0); await w(200); d.run_dispose(); globalThis.vmlabSwiped = true; })(); 'swiping'"
# GNOME's direct scanout held off, counted: the compositor's two calls wrapped for the suite, and taken back at its end.
# Only Amethystora UI's own calls count: GNOME 50's message tray, menus, modal grabs and magnifier make them too, and
# one of theirs that began before the count ends below zero
UNREDIRECT="globalThis.vmlabUnredirect ??= (() => { const c = global.compositor, st = {held: 0};
    st.d = c.disable_unredirect; st.e = c.enable_unredirect;
    const ours = () => (new Error().stack.split(String.fromCharCode(10))[2] ?? '').includes('/amethystora-ui@');
    c.disable_unredirect = function () { if (ours()) st.held++; return st.d.call(this); };
    c.enable_unredirect = function () { if (ours()) st.held--; return st.e.call(this); };
    return st; })(); 'counting'"
held() { ev "globalThis.vmlabUnredirect.held"; }
# The colour at a point: as a screenshot reads it (no screen's paint), or as the screen shows it (seen: the filter
# drawing that one paint too, a hook for the suites)
PICKED=""
pick() {
    local seen="${2:-false}"
    ev "delete globalThis.vmlabPick; (async () => { const shot = new imports.gi.Shell.Screenshot();
        const e = global.stage.get_effect('amethystora-access-filter'); if (e) e.everyPaintForTest = ${seen};
        const c = await new Promise(r => shot.pick_color($1, (o, res) => { const [, k] = shot.pick_color_finish(res);
            r([k.red, k.green, k.blue]); }));
        if (e) e.everyPaintForTest = false;
        globalThis.vmlabPick = JSON.stringify(c); })(); 'started'" >/dev/null
    wait_for 20 is "(globalThis.vmlabPick ?? '').startsWith('[')"
    PICKED="$(ev "globalThis.vmlabPick")"
}
# A pixel of a PNG file, as [r, g, b]
pixel() {
    python3 -c 'import json, sys
import gi
gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf
p = GdkPixbuf.Pixbuf.new_from_file(sys.argv[1]); x, y = int(sys.argv[2]), int(sys.argv[3])
d = p.get_pixels(); o = y * p.get_rowstride() + x * p.get_n_channels()
print(json.dumps(list(d[o:o + 3])))' "$1" "$2" "$3"
}
# What the spec's matrices make of an sRGB colour at a strength, worked out here apart from the module's own code
expected() {
    python3 - "$1" "$2" "$3" <<'EOF'
import json, sys
kind, strength, rgb = sys.argv[1], float(sys.argv[2]), json.loads(sys.argv[3])
SEEN = {
    'protan': [0.152286, 1.052583, -0.204868, 0.114503, 0.786281, 0.099216, -0.003882, -0.048116, 1.051998],
    'deutan': [0.367322, 0.860646, -0.227968, 0.280085, 0.672501, 0.047413, -0.011820, 0.042940, 0.968881],
    'tritan': [1.255528, -0.076749, -0.178779, -0.078411, 0.930809, 0.147602, 0.004733, 0.691367, 0.303900]}
SHIFT = {'protan': [0, 0, 0, .7, 1, 0, .7, 0, 1], 'deutan': [0, 0, 0, .7, 1, 0, .7, 0, 1], 'tritan': [1, 0, .7, 0, 1, .7, 0, 0, 0]}
I = [1, 0, 0, 0, 1, 0, 0, 0, 1]
def mul(a, b):
    return [sum(a[r * 3 + k] * b[k * 3 + c] for k in range(3)) for r in range(3) for c in range(3)]
if kind == 'grey':
    full = [0.2126, 0.7152, 0.0722] * 3
else:
    full = [I[i] + v for i, v in enumerate(mul(SHIFT[kind], [I[i] - SEEN[kind][i] for i in range(9)]))]
m = [I[i] + strength * (full[i] - I[i]) for i in range(9)]
lin = lambda c: c / 12.92 if c < 0.04045 else ((c + 0.055) / 1.055) ** 2.4
enc = lambda c: c * 12.92 if c < 0.0031308 else 1.055 * c ** (1 / 2.4) - 0.055
c = [lin(v / 255) for v in rgb]
out = [min(1, max(0, sum(m[r * 3 + k] * c[k] for k in range(3)))) for r in range(3)]
print(json.dumps([round(enc(v) * 255) for v in out]))
EOF
}
# Two colours within a tolerance of each other, per channel
near() {
    python3 -c 'import json, sys; a, b, t = json.loads(sys.argv[1]), json.loads(sys.argv[2]), int(sys.argv[3])
sys.exit(0 if all(abs(x - y) <= t for x, y in zip(a, b)) else 1)' "$1" "$2" "$3"
}
window_up() { is "global.get_window_actors().some(a => a.meta_window.get_title() === '$1')"; }

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
# A window of one red, a window of selectable text, and a window that says when the back button reaches it
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${HOME_DIR}/.local/share/vmlab-access"
cat >"${HOME_DIR}/.local/share/vmlab-access/win.js" <<'EOF'
import Gdk from 'gi://Gdk?version=4.0';
import Gtk from 'gi://Gtk?version=4.0';
const app = new Gtk.Application({application_id: ARGV[0]});
app.connect('activate', () => {
    const css = new Gtk.CssProvider();
    css.load_from_string('window.red { background: #d93a3a; } label { font-size: 15px; } ' +
        'window.halves { background: linear-gradient(#d93a3a 50%, #3a9a3a 50%); }');
    Gtk.StyleContext.add_provider_for_display(Gdk.Display.get_default(), css, 800);
    const window = new Gtk.ApplicationWindow({application: app, title: ARGV[0], default_width: 640, default_height: 400});
    if (ARGV[1] === 'red') {
        window.add_css_class('red');
    } else if (ARGV[1] === 'halves') {
        window.add_css_class('halves');
    } else if (ARGV[1] === 'back') {
        const back = new Gtk.GestureClick({button: 8});
        back.connect('pressed', () => window.set_title('went back'));
        window.add_controller(back);
    } else {
        window.set_child(new Gtk.Label({selectable: true, wrap: true, label: '-r Most people who read slowly are not slow thinkers. Listening helps them keep their place.'}));
    }
    window.present();
});
app.run([]);
EOF
chown -R "${TEST_USER}:" "${HOME_DIR}/.local/share"
enabled="$(user_bus gsettings get org.gnome.shell enabled-extensions)"
[[ ${enabled} == *vmlab-eval@lab* ]] || user_bus gsettings set org.gnome.shell enabled-extensions "${enabled%]*}, 'vmlab-eval@lab']"
if ((SUITE_PHASE == 1)); then
    for schema in "${ACCESS}" "${TOUCH}"; do
        for key in $(in_session "gsettings list-keys ${schema}"); do gs reset "${schema}" "${key}"; done
    done
    loginctl unlock-sessions
    systemctl restart gdm
fi
ready() { [[ "$(ev "typeof ${A}?.state")" == function && "$(ev "typeof ${T}?.state")" == function ]]; }
if ! wait_for 240 has_session || ! wait_for 120 ready; then
    not_ok "the access and touch modules did not start in the session"
    finish
fi
wait_for 30 is "Main.layoutManager._startingUp === false"
ev "Main.overview.hide(); 'hidden'" >/dev/null
wait_for 10 is "!Main.overview.visible"
ev "${UNREDIRECT}" >/dev/null
cleanup() {
    ev "global.force_animations = true; const u = globalThis.vmlabUnredirect; if (u) { global.compositor.disable_unredirect = u.d; global.compositor.enable_unredirect = u.e; delete globalThis.vmlabUnredirect; } 'restored'" >/dev/null 2>&1
    for schema in "${ACCESS}" "${TOUCH}"; do
        for key in $(in_session "gsettings list-keys ${schema}" 2>/dev/null); do gs reset "${schema}" "${key}"; done
    done
    ev "globalThis.vmlabBlue?.destroy(); delete globalThis.vmlabBlue; 'gone'" >/dev/null 2>&1
    gs reset org.gnome.desktop.a11y.applications screen-magnifier-enabled
    gs reset org.gnome.desktop.a11y.magnifier mag-factor
    gs reset org.gnome.desktop.a11y.magnifier mouse-tracking
    user_bus systemctl --user stop vmlab-red vmlab-text vmlab-back vmlab-halves >/dev/null 2>&1
    in_session "dconf reset -f ${PEN}" >/dev/null 2>&1
    rm -f "${HOME_DIR}/.local/state/amethystora/pen-button" "${HOME_DIR}/.local/state/amethystora/pen-button.bak"
}
trap cleanup EXIT

# The next phase in this session, with the screen as QEMU scans it out handed to it
next_phase() {
    trap - EXIT
    screen_next
    continue_phase
}
SCREEN_PNG=/var/lib/vmlab-tests/screen.png
RED='[217, 58, 58]'
GREEN='[58, 154, 58]'
BLUE='[40, 80, 220]'
# A colour of the screen as QEMU scanned it out, at a place ("x y") in its pixels
shown() {
    local x y
    read -r x y <<<"$1"
    pixel "${SCREEN_PNG}" "${x}" "${y}"
}
# shown_as WHAT PLACE KIND STRENGTH COLOUR: the screen at PLACE shows COLOUR through the matrix, once
shown_as() {
    local seen want
    seen="$(shown "$2")"
    want="$(expected "$3" "$4" "$5")"
    check "$1 (${seen}, wanted ${want})" near "${seen}" "${want}" 3
}
place() { ev "globalThis.vmlabPlaces.$1"; }

if ((SUITE_PHASE == 1)); then
# --- 1-2: the schemas, and off costs nothing -------------------------------------------------------------------------
same "1 the access schema's keys" "$(in_session "gsettings list-keys ${ACCESS}" | sort | paste -sd ' ')" \
    "filter filter-on filter-strength read-aloud read-aloud-key read-aloud-speed ruler ruler-key ruler-shown"
same "1 the touch schema's keys" "$(in_session "gsettings list-keys ${TOUCH}" | sort | paste -sd ' ')" \
    "pen-ring pen-ring-key pen-ring-tools touch-mode"
same "1 shipped off" "$(gs get "${ACCESS}" filter) $(gs get "${ACCESS}" ruler) $(gs get "${ACCESS}" read-aloud) $(gs get "${TOUCH}" touch-mode) $(gs get "${TOUCH}" pen-ring)" \
    "'none' 'off' false 'off' 'off'"
same "2 nothing made while off" "$(ast '') $(tst '')" \
    '{"filter":{"effect":false,"applied":false,"tile":false,"unredirectHeld":false,"bytes":0,"failedScreens":0,"hdrScreens":0,"extras":[]},"ruler":null,"speech":{"reading":false,"left":0,"process":false,"speed":"1"},"keys":[]} {"mode":{"value":"off","active":false,"forced":null,"following":false,"tile":false,"handle":false,"gesture":false,"hint":false},"ring":{"open":false,"virtual":false},"key":false}'
same "2 no shortcut bound" "$(ev "['ruler-key', 'read-aloud-key', 'pen-ring-key'].filter(k => Main.wm._allowedKeybindings[k]).length")" 0
same "2 nothing holds direct scanout off" "$(held)" 0

# --- 3a-3c: what the screen really shows: red over green, deutan, a blue square drawn and moved -----------------------
user_bus systemd-run --user --unit vmlab-halves --collect gjs -m "${HOME_DIR}/.local/share/vmlab-access/win.js" org.vmlab.Halves halves >/dev/null
wait_for 120 window_up org.vmlab.Halves
ev "${MOVE} move(global.stage.width - 2, global.stage.height - 2); 'out of the way'" >/dev/null
# The places, in the stage's coordinates and in the screen's pixels: the square's first and second place, just right of
# its second, far in the red, in the green
ev "globalThis.vmlabPlaces = (() => { const w = global.get_window_actors().find(a => a.meta_window.get_title() === 'org.vmlab.Halves').meta_window;
    w.activate(global.get_current_time()); const r = w.get_frame_rect(), s = global.stage.peek_stage_views()[0].get_scale();
    const p = (x, y) => Math.round(x * s) + ' ' + Math.round(y * s);
    const x = Math.round(r.x + r.width * 0.15), y = Math.round(r.y + r.height * 0.3);
    const far = [Math.round(r.x + r.width * 0.75), Math.round(r.y + r.height * 0.35)];
    return {x, y, far: far.join(' '), first: p(x + 20, y + 20), second: p(x + 120, y + 20), beside: p(x + 142, y + 20),
        red: p(...far), green: p(r.x + r.width * 0.5, r.y + r.height * 0.8), height: r.height, scale: s}; })(); 'placed'" >/dev/null
wait_for 10 is "global.display.focus_window?.get_title() === 'org.vmlab.Halves'"
in_session "ame desktop access filter deutan" >/dev/null
wait_for 10 is "${A}.state().filter.applied"
sleep 2
ev "globalThis.vmlabBlue = new imports.gi.St.Widget({style: 'background-color: #2850dc;', width: 40, height: 40});
    Main.layoutManager.uiGroup.add_child(globalThis.vmlabBlue);
    globalThis.vmlabBlue.set_position(globalThis.vmlabPlaces.x, globalThis.vmlabPlaces.y); 'drawn'" >/dev/null
sleep 1
ev "globalThis.vmlabBlue.set_position(globalThis.vmlabPlaces.x + 100, globalThis.vmlabPlaces.y); 'moved'" >/dev/null
sleep 2
next_phase
fi

if ((SUITE_PHASE == 2)); then
    if [[ -s "${SCREEN_PNG}" ]]; then
        shown_as "3a the screen shows the red filtered once" "$(place red)" deutan 1 "${RED}"
        shown_as "3a ... and the green" "$(place green)" deutan 1 "${GREEN}"
        shown_as "3a where the square was, redrawn red, filtered once" "$(place first)" deutan 1 "${RED}"
        shown_as "3a the square where it moved, filtered once" "$(place second)" deutan 1 "${BLUE}"
        shown_as "3a just beside it, filtered once" "$(place beside)" deutan 1 "${RED}"
    else
        not_ok "3a the lab handed no screen over (run.mjs screen_next)"
    fi
    # Zoom, full screen, twice, the pointer kept at the centre over the red
    gs set org.gnome.desktop.a11y.magnifier mag-factor 2.0
    gs set org.gnome.desktop.a11y.magnifier mouse-tracking centered
    gs set org.gnome.desktop.a11y.applications screen-magnifier-enabled true
    wait_for 10 is "Main.magnifier.isActive()"
    read -r px py <<<"$(place far)"
    ev "${MOVE} move(${px}, ${py}); 'over the red'" >/dev/null
    sleep 1
    ev "${MOVE} move(${px} + 1, ${py}); 'over the red'" >/dev/null
    # The screen's centre shows the pointer's place: 30 px left of it and 20 up, then a quarter of the window lower
    ev "globalThis.vmlabPlaces.zoomed = (() => { const m = Main.layoutManager.primaryMonitor, s = globalThis.vmlabPlaces.scale;
        const c = [m.x + m.width / 2, m.y + m.height / 2], p = (x, y) => Math.round(x * s) + ' ' + Math.round(y * s);
        return {red: p(c[0] - 60, c[1] - 40), green: p(c[0] - 60, c[1] + globalThis.vmlabPlaces.height / 2),
            fits: c[1] + globalThis.vmlabPlaces.height / 2 < m.y + m.height - 10}; })(); 'placed'" >/dev/null
    sleep 2
    same "3b the filter stays applied with Zoom on" "$(ast .filter.applied)" true
    next_phase
fi

if ((SUITE_PHASE == 3)); then
    if [[ -s "${SCREEN_PNG}" ]]; then
        shown_as "3b Zoom's picture of the red, filtered once" "$(place zoomed.red)" deutan 1 "${RED}"
        if [[ "$(place zoomed.fits)" == true ]]; then
            shown_as "3b ... and of the green" "$(place zoomed.green)" deutan 1 "${GREEN}"
        fi
    else
        not_ok "3b the lab handed no screen over (run.mjs screen_next)"
    fi
    gs reset org.gnome.desktop.a11y.applications screen-magnifier-enabled
    gs reset org.gnome.desktop.a11y.magnifier mag-factor
    gs reset org.gnome.desktop.a11y.magnifier mouse-tracking
    wait_for 10 is "!Main.magnifier.isActive()"
    # A screen in HDR is left unfiltered: the primary marked so, as the display configuration would say it
    ev "(() => { const m = Main.layoutManager.primaryMonitor, f = ${A}.filter; f._effect.hdr = new Set([m.x + ',' + m.y]);
        f._failed('hdr', null); global.stage.queue_redraw(); return 'hdr'; })()" >/dev/null
    sleep 2
    same "3c a screen in HDR: the filter on, not applied, said so" "$(ast .filter.effect) $(ast .filter.applied) $(ast .filter.hdrScreens)" \
        "true false 1"
    next_phase
fi

if ((SUITE_PHASE == 4)); then
    if [[ -s "${SCREEN_PNG}" ]]; then
        check "3c ... and that screen shows the apps' own red ($(shown "$(place red)"))" near "$(shown "$(place red)")" "${RED}" 3
    else
        not_ok "3c the lab handed no screen over (run.mjs screen_next)"
    fi
    told() { journalctl --since -15min -q -o cat _UID="$(id -u "${TEST_USER}")" | grep -q 'not drawn on a screen (hdr)'; }
    check "3c ... and the journal says why" wait_for 10 told
    ev "${A}.filter._effect.hdr = new Set(); globalThis.vmlabBlue.destroy(); delete globalThis.vmlabBlue; 'back'" >/dev/null
    in_session "ame desktop access filter none" >/dev/null
    wait_for 10 is "!${A}.state().filter.effect"
    user_bus systemctl --user stop vmlab-halves >/dev/null 2>&1
fi

# --- 3-7: colour filters -----------------------------------------------------------------------------------------------
user_bus systemd-run --user --unit vmlab-red --collect gjs -m "${HOME_DIR}/.local/share/vmlab-access/win.js" org.vmlab.Red red >/dev/null
wait_for 120 window_up org.vmlab.Red
CENTRE="$(ev "(() => { const w = global.get_window_actors().find(a => a.meta_window.get_title() === 'org.vmlab.Red').meta_window;
    w.activate(global.get_current_time()); const r = w.get_frame_rect(); return Math.round(r.x + r.width / 2) + ', ' + Math.round(r.y + r.height / 2 + 20); })()")"
wait_for 10 is "global.display.focus_window?.get_title() === 'org.vmlab.Red'"
pick "${CENTRE}"
ORIGINAL="${PICKED}"
in_session "ame desktop access filter deutan" >/dev/null
wait_for 10 is "${A}.state().filter.effect"
same "3 the filter is on the stage, with its tile, holding scanout off" "$(ast .filter.applied) $(ast .filter.tile) $(held)" \
    "true true 1"
pick "${CENTRE}" true
SEEN="${PICKED}"
WANT="$(expected deutan 1 "${ORIGINAL}")"
check "3 the screen shows the red as the deutan matrix makes it (${ORIGINAL} -> ${SEEN}, wanted ${WANT})" near "${SEEN}" "${WANT}" 2
pick "${CENTRE}"
check "4 a colour picker reads the red as the app draws it (${PICKED})" near "${PICKED}" "${ORIGINAL}" 1
# The screenshot UI (Print) with the filter on: frozen, and a screen capture saved in the apps' own colours
marker="$(mktemp)"
ev "delete globalThis.vmlabUi; (async () => { const ui = Main.screenshotUI; await ui.open();
    globalThis.vmlabUi = !!ui._stageScreenshotContainer?.visible; })().catch(e => (globalThis.vmlabUi = 'error ' + e.message)); 'opening'" >/dev/null
wait_for 20 is "globalThis.vmlabUi !== undefined"
same "4 the screenshot UI opens frozen with the filter on" "$(ev "String(globalThis.vmlabUi)")" true
ev "const ui = Main.screenshotUI; ui._screenButton.checked = true; ui._saveScreenshot(); 'taken'" >/dev/null
newest() { find "${HOME_DIR}" -name 'Screenshot*.png' -newer "${marker}" 2>/dev/null | sort | tail -n1; }
saved_one() { [[ -n "$(newest)" ]]; }
check "4 the screenshot UI saved a screen" wait_for 20 saved_one
ev "Main.screenshotUI.close(); 'closed'" >/dev/null
read -r cx cy <<<"${CENTRE/,/ }"
shot="$(newest)"
[[ -n "${shot}" ]] && check "4 ... in the apps' own colours ($(pixel "${shot}" "${cx}" "${cy}"))" near "$(pixel "${shot}" "${cx}" "${cy}")" "${ORIGINAL}" 1
in_session "gdbus call --session --dest org.gnome.Shell.Screenshot --object-path /org/gnome/Shell/Screenshot --method org.gnome.Shell.Screenshot.Screenshot false false /tmp/vmlab-access.png" >/dev/null
check "4 GNOME's screenshot service saves the apps' own colours too" near "$(pixel /tmp/vmlab-access.png "${cx}" "${cy}")" "${ORIGINAL}" 1
rm -f "${marker}" /tmp/vmlab-access.png
in_session "ame desktop access strength 50" >/dev/null
wait_for 10 is "${A}.settings.get_int('filter-strength') === 50"
pick "${CENTRE}" true
WANT="$(expected deutan 0.5 "${ORIGINAL}")"
check "5 half the strength (${PICKED}, wanted ${WANT})" near "${PICKED}" "${WANT}" 2
in_session "ame desktop access filter grey" >/dev/null
pick "${CENTRE}" true
WANT="$(expected grey 0.5 "${ORIGINAL}")"
check "5 grey at half (${PICKED}, wanted ${WANT})" near "${PICKED}" "${WANT}" 2
ev "${A}.filter._tile.checked = false; ${A}.filter._tile.emit('clicked', 1); 'off'" >/dev/null
wait_for 10 is "!${A}.state().filter.effect"
same "6 the tile's switch takes the filter away and keeps it chosen" "$(gs get "${ACCESS}" filter) $(gs get "${ACCESS}" filter-on) $(ast .filter.effect) $(ast .filter.tile) $(held)" \
    "'grey' false false true 0"
in_session "ame desktop access filter on" >/dev/null
wait_for 10 is "${A}.state().filter.effect"
same "6 on again" "$(ast .filter.applied) $(held)" "true 1"
ev "global.get_window_actors().find(a => a.meta_window.get_title() === 'org.vmlab.Red').meta_window.make_fullscreen(); 'full'" >/dev/null
wait_for 10 is "global.display.focus_window?.is_fullscreen()"
pick "${CENTRE}" true
WANT="$(expected grey 0.5 "${ORIGINAL}")"
check "7 a fullscreen window is filtered too (${PICKED})" near "${PICKED}" "${WANT}" 2
same "7 direct scanout stays held off over it" "$(held)" 1
ev "global.get_window_actors().find(a => a.meta_window.get_title() === 'org.vmlab.Red').meta_window.unmake_fullscreen(); 'back'" >/dev/null
in_session "ame desktop access filter none" >/dev/null
wait_for 10 is "!${A}.state().filter.effect"
same "6 none takes the filter and the tile away, scanout free again" "$(ast .filter) $(held)" \
    '{"effect":false,"applied":false,"tile":false,"unredirectHeld":false,"bytes":0,"failedScreens":0,"hdrScreens":0,"extras":[]} 0'
# Focus's grey: laid over the filter's one effect, here with no filter chosen
ev "${UI}.get('focus')._setGrey(true); 'grey'" >/dev/null
wait_for 10 is "${A}.state().filter.effect"
same "7b Focus's grey goes through the same effect, with no tile and no effect of its own" \
    "$(ast .filter.extras) $(ast .filter.tile) $(ev "${UI}.get('focus').state().greyBy") $(ev "Main.layoutManager.uiGroup.get_effect('amethystora-focus-grey') === null")" \
    '["amethystora-focus-grey"] false access true'
pick "${CENTRE}" true
WANT="$(expected grey 1 "${ORIGINAL}")"
check "7b the screen grey (${PICKED}, wanted ${WANT})" near "${PICKED}" "${WANT}" 2
ev "${UI}.get('focus')._setGrey(false); 'colour'" >/dev/null
wait_for 10 is "!${A}.state().filter.effect"
same "7b gone with it, scanout free again" "$(ast .filter.extras) $(held)" "[] 0"
user_bus systemctl --user stop vmlab-red >/dev/null 2>&1

# --- 8: the reading ruler ----------------------------------------------------------------------------------------------
user_bus systemd-run --user --unit vmlab-text --collect gjs -m "${HOME_DIR}/.local/share/vmlab-access/win.js" org.vmlab.Text >/dev/null
wait_for 120 window_up org.vmlab.Text
FRAME="$(ev "(() => { const w = global.get_window_actors().find(a => a.meta_window.get_title() === 'org.vmlab.Text').meta_window;
    w.activate(global.get_current_time()); const r = w.get_frame_rect(); return [r.x, r.y, r.width, r.height].join(' '); })()")"
read -r fx fy fw fh <<<"${FRAME}"
in_session "ame desktop access ruler band" >/dev/null
wait_for 10 is "${A}.state().ruler !== null"
ev "${MOVE} move(${fx} + 50, ${fy} + 100); 'moved'" >/dev/null
wait_for 10 is "${A}.state().ruler.pieces[0].y === ${fy} + 85"
band="$(ast .ruler)"
check "8 a band of 30 px centred on the pointer, across the focused window ($band)" python3 -c 'import json, sys
r = json.loads(sys.argv[1]); p = r["pieces"]; fx, fy, fw = map(int, sys.argv[2:5])
sys.exit(0 if r["style"] == "band" and len(p) == 1 and p[0]["height"] == 30 and p[0]["y"] == fy + 85 and p[0]["x"] == fx and p[0]["width"] == fw else 1)' "${band}" "${fx}" "${fy}" "${fw}"
same "8 holding scanout off while shown" "$(held)" 1
ev "${MOVE} move(5, 600); 'off the window'" >/dev/null
wait_for 10 is "${A}.state().ruler.pieces[0].x === Main.layoutManager.primaryMonitor.x"
same "8 across the screen off the window" "$(ev "${A}.state().ruler.pieces[0].width === Main.layoutManager.primaryMonitor.width")" true
gs set org.gnome.desktop.interface text-scaling-factor 1.25
ev "${MOVE} move(${fx} + 50, ${fy} + 120); 'moved'" >/dev/null
check "8 as tall as larger text" wait_for 10 is "${A}.state().ruler.pieces[0].height === 38"
gs reset org.gnome.desktop.interface text-scaling-factor
in_session "ame desktop access ruler dim" >/dev/null
# The window's frame again: the larger text and back can move it by a few pixels (it moved 8 down in the lab), and with
# animations on it settles after a moment: read until two reads a second apart agree
frame() { ev "(() => { const r = global.get_window_actors().find(a => a.meta_window.get_title() === 'org.vmlab.Text')
    .meta_window.get_frame_rect(); return [r.x, r.y, r.width, r.height].join(' '); })()"; }
settled_frame() { local was; was="$(frame)"; sleep 1; [[ "$(frame)" == "${was}" ]]; }
wait_for 10 settled_frame
read -r fx fy fw fh <<<"$(frame)"
ev "${MOVE} move(${fx} + 50, ${fy} + 100); 'moved'" >/dev/null
wait_for 10 is "${A}.state().ruler?.style === 'dim' && ${A}.state().ruler.pieces[1]?.y === ${fy} + 115"
dim="$(ast .ruler.pieces)"
check "8 dim: two veils of the window, the line between them clear ($dim)" python3 -c 'import json, sys
p = json.loads(sys.argv[1]); fx, fy, fw, fh = map(int, sys.argv[2:6])
a, b = p
sys.exit(0 if a["y"] == fy and a["height"] == 85 and b["y"] == fy + 115 and a["y"] + a["height"] + 30 == b["y"]
    and b["y"] + b["height"] == fy + fh and a["width"] == fw == b["width"] else 1)' "${dim}" "${fx}" "${fy}" "${fw}" "${fh}"
in_session "ame desktop access ruler underline" >/dev/null
wait_for 10 is "${A}.state().ruler?.style === 'underline'"
same "8 underline: a line 3 px high" "$(ast .ruler.pieces[0].height)" 3
accent="$(in_session "sed -n 's/^accent_bg *= *\"\\(#[0-9a-fA-F]*\\)\"/\\1/p' ~/.config/amethystora/current/theme/palette.toml" | tr 'A-F' 'a-f')"
same "8 in the theme's accent" "$(ev "${A}.ruler._container.get_first_child().get_theme_node().get_background_color().to_string().slice(0, 7)")" "${accent}"
same "8 Super+Ctrl+R is bound" "$(ast .keys)" '["ruler-key"]'
keys super+ctrl+r
check "8 Super+Ctrl+R hides it: nothing made, scanout free" wait_for 10 is "${A}.state().ruler === null && globalThis.vmlabUnredirect.held === 0"
keys super+ctrl+r
check "8 and shows it again" wait_for 10 is "${A}.state().ruler !== null"
in_session "ame desktop access ruler off" >/dev/null
wait_for 10 is "${A}.state().ruler === null"
same "8 off: not bound" "$(ast .keys)" '[]'

# --- 9: reading aloud --------------------------------------------------------------------------------------------------
in_session "ame desktop access read-aloud on" >/dev/null
wait_for 10 is "!!Main.wm._allowedKeybindings['read-aloud-key']"
ev "imports.gi.St.Clipboard.get_default().set_text(imports.gi.St.ClipboardType.PRIMARY, '-r Most people who read slowly are not slow thinkers. Listening helps them keep their place.'); 'selected'" >/dev/null
keys super+ctrl+s
check "9 Super+Ctrl+S: the pill reads" wait_for 10 is "${UI}.get('pill').state().activity === 'reading'"
said="$(pgrep -a -u "${TEST_USER}" spd-say | head -n1)"
check "9 spd-say in pipe mode, the sentence not on its command line ($said)" like "${said}" "*spd-say --pipe-mode --wait --application-name amethystora --rate 0*"
[[ "${said}" == *"slow thinkers"* ]] && not_ok "9 the sentence is on spd-say's command line"
ev "${A}.speech.nextSpeed(); 'faster'" >/dev/null
same "9 the speed chip" "$(ev "${A}.speech._speedChip?.text")" "1.5×"
sentence_two() { pgrep -a -u "${TEST_USER}" spd-say | grep -q -- '--rate 40'; }
check "9 the next sentence at 1.5×" wait_for 30 sentence_two
keys super+ctrl+s
check "9 Super+Ctrl+S again stops: nothing left" wait_for 10 is "!${A}.state().speech.reading"
spd_gone() { [[ "$(pgrep -u "${TEST_USER}" -c spd-say)" == 0 ]]; }
check "9 no spd-say left" wait_for 10 spd_gone
ev "imports.gi.St.Clipboard.get_default().set_text(imports.gi.St.ClipboardType.PRIMARY, ''); ${A}.speech.toggle(); 'empty'" >/dev/null
check "9 nothing selected" wait_for 10 is "${UI}.get('pill').state().text === 'Select some text first'"
in_session "ame desktop access read-aloud off" >/dev/null

# --- 10-12: touch mode -------------------------------------------------------------------------------------------------
BAR="$(ev "Main.panel.height")"
ev "Main.panel.statusArea.dateMenu.menu.close(); 'closed'" >/dev/null
ev "${SWIPE}" >/dev/null
wait_for 10 is "globalThis.vmlabSwiped === true"
same "12 no Today from the edge while touch mode is off" "$(ev "Main.panel.statusArea.dateMenu.menu.isOpen")" false
in_session "ame desktop touch mode on" >/dev/null
check "10 the bar 12 px taller" wait_for 10 is "Main.panel.height === ${BAR} + 12"
same "10 the dock's icons 64 px" "$(ev "${UI}.get('dock')._sites[0].dash.iconSize")" 64
row="$(ev "(() => { const b = new imports.gi.St.BoxLayout({style_class: 'popup-menu-item'}); Main.layoutManager.uiGroup.add_child(b);
    const [, h] = b.get_preferred_height(-1); b.destroy(); return h; })()")"
check "10 a menu row 44 px at least ($row)" test "${row%.*}" -ge 44
same "10 the handle, the gesture and the hint" "$(tst .mode.handle) $(tst .mode.gesture) $(tst .mode.hint)" "true true true"
check "10 the hint goes after about four seconds" wait_for 10 is "!${T}.state().mode.hint"
ev "delete globalThis.vmlabSwiped; ${SWIPE}" >/dev/null
check "12 a swipe in from the right edge opens Today" wait_for 10 is "Main.panel.statusArea.dateMenu.menu.isOpen"
ev "Main.panel.statusArea.dateMenu.menu.close(); 'closed'" >/dev/null
in_session "ame desktop touch mode follow" >/dev/null
wait_for 10 is "${T}.state().mode.value === 'follow'"
ev "${T}.setSeatForTest(false); 'laptop'" >/dev/null
same "11 follow, laptop: off" "$(tst .mode.active)" false
ev "${T}.setSeatForTest(true); 'tablet'" >/dev/null
same "11 follow, folded: on" "$(tst .mode.active)" true
ev "${T}.mode.force(); 'forced'" >/dev/null
same "11 the tile forces it off" "$(tst .mode.active) $(gs get "${TOUCH}" touch-mode)" "false 'follow'"
ev "${T}.setSeatForTest(false); ${T}.setSeatForTest(true); 'moved'" >/dev/null
same "11 until the hinge moves" "$(tst .mode.active)" true
ev "${T}.setSeatForTest(null); 'done'" >/dev/null
in_session "ame desktop touch mode off" >/dev/null
check "10 off: all back" wait_for 10 is "Main.panel.height === ${BAR} && ${UI}.get('dock')._sites[0].dash.iconSize === 48 && !${T}.state().mode.handle"

# --- 13-14: the pen's tools --------------------------------------------------------------------------------------------
in_session "ame desktop touch pen-ring ring" </dev/null >/dev/null
wait_for 10 is "${T}.state().key"
ev "global.force_animations = true; ${MOVE} move(900, 600); 'moved'" >/dev/null
keys super+ctrl+p
check "13 Super+Ctrl+P opens the ring" wait_for 10 is "${T}.state().ring.open"
same "13 the ring holds scanout off" "$(held)" 1
ring="$(tst .ring)"
# Scribble only where a module offers it: round 4's present mode does, as the image ships
scribble="$(ev "[...${UI}.values()].some(m => typeof m?.toggleScribble === 'function')")"
check "13 the ring at the pen, its tools 58 px, 70 px out, the slots with nothing to run left empty ($ring)" python3 -c 'import json, math, sys
r = json.loads(sys.argv[1]); cx, cy = r["centre"]
ids = [t["id"] for t in r["tools"]]
ok = r["open"] and r["variant"] == "ring" and (cx, cy) == (900, 600) and ("scribble" in ids) == (sys.argv[2] == "true")
ok = ok and {"area", "right-click", "back"} <= set(ids)
for t in r["tools"]:
    ok = ok and t["width"] == 58 and abs(math.hypot(t["x"] + 29 - cx, t["y"] + 29 - cy) - 70) <= 1
sys.exit(0 if ok else 1)' "${ring}" "${scribble}"
ev "${MOVE} move(960, 590); 'toward area'" >/dev/null
check "13 moving toward Area lights it" wait_for 10 is "${T}.state().ring.lit === 'area'"
keys esc
check "13 Escape closes it, scanout free" wait_for 10 is "!${T}.state().ring.open && globalThis.vmlabUnredirect.held === 0"
ev "${MOVE} move(900, 600); 'moved'" >/dev/null
keys super+ctrl+p
wait_for 10 is "${T}.state().ring.open"
keys click:0.05,0.9
check "13 a click outside closes it" wait_for 10 is "!${T}.state().ring.open"
keys super+ctrl+p
wait_for 10 is "${T}.state().ring.open"
ev "${T}.ring.runForTest('area'); 'run'" >/dev/null
check "13 Area: the screenshot UI on a selection" wait_for 10 is "Main.screenshotUI.visible && Main.screenshotUI._selectionButton.checked"
ev "Main.screenshotUI.close(); global.force_animations = true; 'closed'" >/dev/null
in_session "ame desktop touch pen-ring strip" </dev/null >/dev/null
keys super+ctrl+p
check "13 the strip at the right edge" wait_for 10 is "${T}.state().ring.variant === 'strip'"
keys esc
in_session "ame desktop touch pen-ring ring" </dev/null >/dev/null
POINT="$(ev "(() => { const w = global.get_window_actors().find(a => a.meta_window.get_title() === 'org.vmlab.Text').meta_window;
    w.activate(global.get_current_time()); const r = w.get_frame_rect(); return [Math.round(r.x + r.width / 2), Math.round(r.y + r.height / 2)].join(','); })()")"
ev "${MOVE} move(${POINT}); ${T}.openRing(); ${T}.ring.runForTest('right-click'); 'clicked'" >/dev/null
menu_up() { is "global.get_window_actors().some(a => a.meta_window.get_window_type() === imports.gi.Meta.WindowType.POPUP_MENU)"; }
check "14 a right click where the ring opened opens the app's menu" wait_for 10 menu_up
check "14 the click's pointer is dropped after it" wait_for 10 is "!${T}.state().ring.virtual && !${T}.ring.clicking"
keys esc
user_bus systemctl --user stop vmlab-text >/dev/null 2>&1
user_bus systemd-run --user --unit vmlab-back --collect gjs -m "${HOME_DIR}/.local/share/vmlab-access/win.js" org.vmlab.Back back >/dev/null
wait_for 120 window_up org.vmlab.Back
POINT="$(ev "(() => { const w = global.get_window_actors().find(a => a.meta_window.get_title() === 'org.vmlab.Back').meta_window;
    w.activate(global.get_current_time()); const r = w.get_frame_rect(); return [Math.round(r.x + r.width / 2), Math.round(r.y + r.height / 2)].join(','); })()")"
ev "${MOVE} move(${POINT}); ${T}.openRing(); ${T}.ring.runForTest('back'); 'back'" >/dev/null
check "14 Back: the back button reaches the app where the ring opened" wait_for 10 window_up "went back"
user_bus systemctl --user stop vmlab-back >/dev/null 2>&1

# --- 15: the pen's button ----------------------------------------------------------------------------------------------
in_session "gsettings set ${STYLUS}:${PEN} button-action back" >/dev/null
out="$(in_session "ame desktop touch pen-button on")"
same "15 the pen's button sends the tools' shortcut" "$(gs get "${STYLUS}:${PEN}" button-action) $(gs get "${STYLUS}:${PEN}" button-keybinding)" \
    "'keybinding' '<Super><Control>p'"
check "15 it says a pen used here was set" grep -q "1 pen used here" <<<"${out}"
in_session "ame desktop touch pen-button on" >/dev/null
in_session "ame desktop touch pen-button off" >/dev/null
same "15 off: what it did, once again" "$(gs get "${STYLUS}:${PEN}" button-action) $(gs get "${STYLUS}:${PEN}" button-keybinding)" "'back' ''"
check "15 the kept file is gone" test ! -e "${HOME_DIR}/.local/state/amethystora/pen-button"
in_session "ame desktop touch pen-button on" >/dev/null
in_session "gsettings set ${STYLUS}:${PEN} button-action right" >/dev/null
in_session "ame desktop touch pen-button off" >/dev/null
same "15 off leaves a button changed since alone" "$(gs get "${STYLUS}:${PEN}" button-action)" "'right'"

# --- 16: ten disables and enables with everything on -----------------------------------------------------------------
in_session "ame desktop access filter tritan; ame desktop access ruler band; ame desktop access read-aloud on; ame desktop touch mode on" >/dev/null
wait_for 10 is "${A}.state().filter.effect && ${T}.state().mode.active"
CYCLE="(async () => { const uuid = '${UUID}';
    const count = () => Main.uiGroup.get_n_children() + global.window_group.get_n_children();
    const w = ms => new Promise(r => setTimeout(r, ms));
    Main.extensionManager.disableExtension(uuid); await w(500); const before = count();
    const actions = global.stage.get_actions().length;
    for (let i = 0; i < 10; i++) { Main.extensionManager.enableExtension(uuid); await w(600);
        Main.extensionManager.lookup(uuid).stateObj.modules?.get('touch')?.openRing(); await w(100);
        Main.extensionManager.disableExtension(uuid); await w(500); }
    /* Fewer than before is no leak: an actor from before the cycles (a closing card) may go during them */
    const left = [Math.max(0, count() - before), Main.layoutManager.uiGroup.get_effects().length + global.stage.get_effects().length,
        Main.layoutManager.uiGroup.has_style_class_name('amethystora-touch'), global.stage.get_actions().length - actions,
        ['ruler-key', 'read-aloud-key', 'pen-ring-key'].filter(k => Main.wm._allowedKeybindings[k]).length,
        globalThis.vmlabUnredirect.held];
    Main.extensionManager.enableExtension(uuid);
    globalThis.vmlabAccessCycle = left.join(' '); })(); 'started'"
ev "${CYCLE}" >/dev/null
wait_for 90 is "globalThis.vmlabAccessCycle !== undefined"
same "16 nothing left behind: actors, effects, the class, gestures, shortcuts, scanout" "$(ev "globalThis.vmlabAccessCycle")" "0 0 false 0 0 0"
wait_for 30 ready
check "16 on again after them" wait_for 30 is "${A}.state().filter.applied && ${T}.state().mode.active"
check "16 no JS ERROR" test -z "$(journalctl --since "-10min" -q -o cat _UID="$(id -u "${TEST_USER}")" | grep -E 'JS ERROR|Gjs-CRITICAL' | grep -iE 'access|touch|quick' | head -5)"

# --- 17: Control and the setup file -------------------------------------------------------------------------------------
control="$(in_session "/usr/libexec/amethystora-control-list --json" | jq -c '[.items[] | select(.id | IN("colour-filter", "colour-filter-strength", "reading-ruler", "read-aloud", "touch-mode", "pen-ring", "pen-button")) | .id] | length')"
same "17 Control has the seven entries" "${control}" 7
saved="$(in_session "ame setup save - 2>/dev/null" | jq -c '[.settings.access, .settings.touch]')"
check "17 the setup file carries them, never filter-on or ruler-shown ($saved)" python3 -c 'import json, sys
a, t = json.loads(sys.argv[1])
sys.exit(0 if a.get("filter") == "tritan" and "filter-on" not in a and "ruler-shown" not in a and t.get("touch-mode") == "on" else 1)' "${saved}"

finish
