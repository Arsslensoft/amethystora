#!/usr/bin/bash
# The dock beyond (plans/beyond/spec-dock.md, section 7.1), one screen, asked of the session through GNOME Shell's Eval,
# which a lab-only extension of the test account opens (never in the image). The pointer is moved inside the shell by a
# virtual pointer for the motion's numbers (test code only), and by keys.py where a person's click or drag matters:
#   - the thirteen keys and their defaults; ame desktop dock sets each, the shell follows, a wrong value is refused;
#   - Swell's scales and translations against the spec's numbers, Lift, Still, back to rest, nothing while away, sharp
#     icons, the frame time;
#   - Halo, Hop and Breathe while an app starts, ended by its first window, or after six seconds;
#   - the bar, the dots and the glow; recent apps; progress; an app greyed while Flatpak updates it; previews; the fan; folders; stacks; drops onto a stack
#     through Files; the phone (when the phone link's stand-in is up); the dissolve;
#   - the numbers while Super is held; the edges; hiding when touched; this workspace's apps only;
#   - disable and enable ten times leaving nothing; every key off leaves the base connections; lock and unlock.
# Four screens are 11-dock-screens.sh's.
# vmlab-paths: @ui/extension.js @ui/*dock* @sf/*themed/shell-dock* @sf/*just/desktop.just
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HOME_DIR="/var/home/${TEST_USER}"
DOCK_SCHEMA=org.gnome.shell.extensions.amethystora-ui.dock
keys() { python3 "$(dirname "$0")/keys.py" "$@"; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
DOCK="${UI}.get('dock')"
S="${DOCK}.state()"
# Before every expression: the virtual pointer of the tests, made once in the shell, and the dock's items
VP="const Clutter = imports.gi.Clutter; const GLib = imports.gi.GLib;
global._vmlabVp ??= global.stage.context.get_backend().get_default_seat().create_virtual_device(Clutter.InputDeviceType.POINTER_DEVICE);
const move = (x, y) => global._vmlabVp.notify_absolute_motion(GLib.get_monotonic_time(), x, y);
const click = (x, y, b = 1) => { move(x, y); const t = GLib.get_monotonic_time(); global._vmlabVp.notify_button(t, b, Clutter.ButtonState.PRESSED); global._vmlabVp.notify_button(t + 30000, b, Clutter.ButtonState.RELEASED); };
const items = () => ${S}.docks[0].items; const item = id => items().find(i => i.id === id); const mid = r => [r.x + r.width / 2, r.y + r.height / 2];"
# Eval's answer is JSON.stringify of the value, which gdbus prints as a GVariant string: decoded once here, a string
# printed as itself and anything else as compact JSON. A throw prints nothing, says why on stderr and fails.
EV_DECODE='
import ast, json, re, sys
text = sys.stdin.read().strip()
answer = re.fullmatch(r"\((true|false), (.*)\)", text, re.S)
if not answer:
    sys.exit("Eval answered: " + text)
value = ast.literal_eval(answer.group(2))
if answer.group(1) == "false":
    sys.exit("the shell threw: " + value)
try:
    value = json.loads(value)
except ValueError:
    pass
print(value if isinstance(value, str) else json.dumps(value, separators=(",", ":"), ensure_ascii=False))'
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval \
        "${VP} $1" | python3 -I -c "${EV_DECODE}"
}
vp() { ev "$1"; }
dset() { in_session "gsettings set ${DOCK_SCHEMA} $1 $2" >/dev/null 2>&1; }
dget() { in_session "gsettings get ${DOCK_SCHEMA} $1" 2>/dev/null; }
# |A - B| <= TOL
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN { d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t) }'; }
settled() { [[ "$(ev "${S}.frameClockRunning")" == false ]]; }
# Predicates for wait_for, which runs them again each time
ev_is() { [[ "$(ev "$1")" == "$2" ]]; }
ev_ge() { local v; v="$(ev "$1")"; [[ "${v}" =~ ^[0-9]+$ ]] && ((v >= $2)); }
settle() { sleep 0.3; wait_for 2 settled; }
away() { vp "move(960, 400); 1" >/dev/null; settle; }
since="$(date '+%Y-%m-%d %H:%M:%S')"

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
# A test app: opens WINDOWS windows after DELAY ms, under its own app id, and the launchers that start it
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${HOME_DIR}/.local/share/vmlab-dock" "${HOME_DIR}/.local/share/applications"
cat >"${HOME_DIR}/.local/share/vmlab-dock/win.js" <<'EOF'
import Adw from 'gi://Adw?version=1';
import GLib from 'gi://GLib';
const [id, delay = '0', count = '1', title = 'Window'] = ARGV;
const app = new Adw.Application({application_id: id});
app.connect('activate', () => {
    app.hold();
    GLib.timeout_add(GLib.PRIORITY_DEFAULT, Number(delay), () => {
        for (let i = 0; i < Number(count); i++)
            new Adw.ApplicationWindow({application: app, default_width: 640, default_height: 400, title: `${title} ${i + 1}`}).present();
        app.release();
        return GLib.SOURCE_REMOVE;
    });
});
app.run([]);
EOF
# A stand-in for an app that says its progress through the launcher protocol, and stays on the bus SECONDS
cat >"${HOME_DIR}/.local/share/vmlab-dock/entry.js" <<'EOF'
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
const [id, progress, seconds] = ARGV;
Gio.DBus.session.emit_signal(null, '/vmlab', 'com.canonical.Unity.LauncherEntry', 'Update',
    new GLib.Variant('(sa{sv})', [`application://${id}`, {
        'progress': new GLib.Variant('d', Number(progress)),
        'progress-visible': new GLib.Variant('b', true),
    }]));
const loop = new GLib.MainLoop(null, false);
GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, Number(seconds), () => {
    loop.quit();
    return GLib.SOURCE_REMOVE;
});
loop.run();
EOF
for name in A B C D; do
    printf '[Desktop Entry]\nType=Application\nName=Test %s\nExec=gjs -m %s org.test.%s 1500 1 Test%s\nIcon=applications-science\n' \
        "${name}" "${HOME_DIR}/.local/share/vmlab-dock/win.js" "${name}" "${name}" \
        >"${HOME_DIR}/.local/share/applications/org.test.${name}.desktop"
done
printf '[Desktop Entry]\nType=Application\nName=Sleeper\nExec=sleep 60\nIcon=applications-utilities\n' \
    >"${HOME_DIR}/.local/share/applications/org.test.Sleep.desktop"
chown -R "${TEST_USER}:" "${HOME_DIR}/.local/share"
enabled="$(user_bus gsettings get org.gnome.shell enabled-extensions)"
if [[ ${enabled} != *vmlab-eval@lab* ]]; then
    user_bus gsettings set org.gnome.shell enabled-extensions "${enabled%]*}, 'vmlab-eval@lab']"
    loginctl unlock-sessions
    systemctl restart gdm
    wait_for 240 has_session
    sleep 10
fi
if [[ "$(ev "Boolean(${DOCK})")" != true ]]; then
    not_ok "the dock module is not running" "$(ev "${UI} ? [...${UI}.keys()].join(' ') : 'no modules'")"
    finish
fi
ev "Main.overview.hide(); 1" >/dev/null
# The lab renders in software, where GNOME turns animations off and comfort then reports reduce motion: the suite
# turns them on for its animated checks, and puts them back however it ends (plans/beyond/IMPLEMENTERS.md)
ev "global._vmlabForced = !imports.gi.St.Settings.get().enable_animations; global.force_animations = true; if (global._vmlabForced) imports.gi.St.Settings.get().uninhibit_animations(); 1" >/dev/null
restore_animations() {
    ev "global.force_animations = true; if (global._vmlabForced) imports.gi.St.Settings.get().inhibit_animations(); global._vmlabForced = false; 1" >/dev/null 2>&1
}
trap restore_animations EXIT
sleep 1
if [[ "$(ev "${DOCK}.motionLevel()")" != full ]]; then
    not_ok "the dock does not move in full here, with animations forced on" "$(ev "${DOCK}.motionLevel()")"
    finish
fi
FAVORITES="$(user_bus gsettings get org.gnome.shell favorite-apps)"

# --- Keys, ame desktop dock ------------------------------------------------------------------------------------------
# What a run stopped part way left behind is put back first: a reset gives what the image ships, overrides included
in_session "gsettings list-keys ${DOCK_SCHEMA} | while read -r key; do gsettings reset ${DOCK_SCHEMA} \"\${key}\"; done"
# Read again until they settle: the shell may still be writing a key it follows when the reset reaches it
dock_defaults() {
    [[ "$(in_session "gsettings list-recursively ${DOCK_SCHEMA}" | cut -d' ' -f2- | LC_ALL=C sort | paste -sd ';')" == "$1" ]]
}
check "the dock's keys have the spec's defaults" wait_for 10 dock_defaults \
    "autohide 'off';edge 'bottom';folders @a{sas} {};icon-size 48;indicator 'bar';launch 'halo';magnify 'swell';magnify-amount 1.6000000000000001;previews true;recent true;screens 'primary';stacks ['xdg:DOWNLOAD'];updating true;workspace-apps false"
bad=""
while read -r setting value key want shown; do
    in_session "ame desktop dock ${setting} ${value}" >/dev/null 2>&1 || bad+="ame desktop dock ${setting} ${value} failed; "
    [[ "$(dget "${key}")" == "${want}" ]] || bad+="${key} is $(dget "${key}"), not ${want}; "
    sleep 1
    shown="${shown//D./${DOCK}.}"
    [[ "$(ev "${shown}")" == "${want//\'/}" ]] || bad+="the shell has $(ev "${shown}") for ${key}; "
done <<'EOF'
magnify lift magnify 'lift' D.settings.get_string('magnify')
amount 200 magnify-amount 2.0 D.settings.get_double('magnify-amount').toFixed(1)
launch hop launch 'hop' D.settings.get_string('launch')
indicator glow indicator 'glow' D.settings.get_string('indicator')
recent off recent false String(Boolean(D.recent))
previews off previews false String(D.settings.get_boolean('previews'))
edge left edge 'left' D.state().docks[0].edge
screens every screens 'every' D.settings.get_string('screens')
autohide touch autohide 'touch' (D.state().docks[0].touch ? 'touch' : 'off')
workspace-apps on workspace-apps true String(D.workspaceApps)
updating off updating false String(D.settings.get_boolean('updating'))
EOF
for key in magnify magnify-amount launch indicator recent previews edge screens autohide workspace-apps updating; do
    in_session "gsettings reset ${DOCK_SCHEMA} ${key}"
done
sleep 2
if [[ -z "${bad}" ]]; then ok "every ame desktop dock setting sets its key and the shell follows"; else not_ok "ame desktop dock" "${bad}"; fi
wrong=""
for args in "magnify wobble" "amount 300" "launch fly" "indicator star" "edge top" "screens some" "autohide maybe" "recent maybe"; do
    out="$(in_session "ame desktop dock ${args}" 2>&1)"
    code=$?
    [[ ${code} == 64 && ${out} == *"usage: ame desktop dock"* ]] || wrong+="${args}: ${code} ${out}; "
done
if [[ -z "${wrong}" ]]; then ok "a wrong value exits 64 with the usage line"; else not_ok "wrong values" "${wrong}"; fi
check "ame desktop dock alone prints fourteen settings" test "$(in_session "ame desktop dock" | wc -l)" == 14

# --- Swell -----------------------------------------------------------------------------------------------------------
user_bus gsettings set org.gnome.shell favorite-apps "['org.mozilla.firefox.desktop', 'org.gnome.Nautilus.desktop', \
'amethystora-manual.desktop', 'amethystora-update.desktop', 'amethystora-backups.desktop', 'amethystora-security.desktop', \
'kitty.desktop', 'amethystora-notes.desktop', 'amethystora-control.desktop']"
sleep 2
away
# Item N's state as "scale translation away"
apps_at() { wait_for 10 ev_ge "items().filter(i => i.kind === 'app').length" $(($1 + 1)); vp "const a = items().filter(i => i.kind === 'app'); const p = mid(a[$1].rest); move(p[0] + ($2), p[1]); 1" >/dev/null; settle; }
row() { ev "JSON.stringify(items().filter(i => i.kind === 'app').map(i => [i.scale, i.translation, i.translationAway]))"; }
apps_at 4 0
values="$(row)"
pitch="$(ev "${S}.docks[0].pitch")"
want_s=(1 1 1.0307 1.3679 1.6 1.3679 1.0307 1 1)
want_t=(-41.92 -41.92 -40.99 -29.04 0 29.04 40.99 41.92 41.92)
bad=""
for i in 0 1 2 3 4 5 6 7 8; do
    s="$(jq ".[${i}][0]" <<<"${values}")"
    t="$(jq ".[${i}][1]" <<<"${values}")"
    near "${s}" "${want_s[i]}" 0.01 || bad+="item ${i} scale ${s}; "
    near "${t}" "${want_t[i]}" 1 || bad+="item ${i} moved ${t}; "
done
if [[ -z "${bad}" ]]; then ok "Swell at item 4: the spec's scales and translations (pitch ${pitch})"; else not_ok "Swell at item 4" "${bad}" "${values}"; fi
no_overlap="$(ev "(() => { const it = items(); for (let i = 1; i < it.length; i++) if (it[i - 1].shown.x + it[i - 1].shown.width > it[i].shown.x + 0.5) return 'overlap at ' + i; return 'none'; })()")"
check "no two items overlap while swollen" test "${no_overlap}" == none
pill="$(ev "(() => { const d = ${S}.docks[0]; const it = d.items; const T = it[it.length - 1].rest.x + it[it.length - 1].rest.width - it[0].rest.x; const P = d.pill.width - T; const swollen = it.reduce((a, i) => a + i.rest.width * i.scale, 0) + d.gaps.reduce((a, g) => a + g, 0); return [(swollen + P) / (T + P), d.pill.scaleLong].join(' '); })()")"
check "the pill lengthens with the row" near "${pill% *}" "${pill#* }" 0.005
# An item gone since the row was measured still counts, or the pill stays one item short when the row comes to rest
gone="$(ev "(() => { const d = ${DOCK}.docks[0].dash; const it = d.swell.items[0]; const was = d.pill.scale_x; it.actor._amethystoraGone = true; d.swell._write(); const now = d.pill.scale_x; it.actor._amethystoraGone = false; d.swell._write(); return [was, now].join(' '); })()")"
check "an item gone in the last frame leaves the pill's length alone" near "${gone% *}" "${gone#* }" 0.005
tex="$(ev "${S}.docks[0].items.filter(i => i.kind === 'app')[4].iconTexture")"
check "the icon is drawn at the size it grows to (77 px)" test "${tex}" == 77
apps_at 4 32
values="$(row)"
check "half a pitch away, items 4 and 5 are 1.535" sh -c "$(declare -f near); near $(jq '.[4][0]' <<<"${values}") 1.535 0.01 && near $(jq '.[5][0]' <<<"${values}") 1.535 0.01"
apps_at 0 0
values="$(row)"
check "at item 0: 1.6, moved -12, item 1 +17" sh -c "$(declare -f near); near $(jq '.[0][0]' <<<"${values}") 1.6 0.01 && near $(jq '.[0][1]' <<<"${values}") -11.96 1 && near $(jq '.[1][1]' <<<"${values}") 17.08 1"
in_session "ame desktop dock amount 200" >/dev/null
apps_at 4 0
values="$(row)"
check "at 200 per cent: 2.0, neighbours 1.613" sh -c "$(declare -f near); near $(jq '.[4][0]' <<<"${values}") 2 0.01 && near $(jq '.[3][0]' <<<"${values}") 1.613 0.01"
in_session "ame desktop dock amount 160" >/dev/null
apps_at 4 0
# Back to rest within 300 ms of leaving
vp "move(960, 400); 1" >/dev/null
sleep 0.3
rest="$(ev "JSON.stringify([items().every(i => Math.abs(i.scale - 1) <= 0.005 && Math.abs(i.translation) <= 0.5), ${S}.frameClockRunning])")"
check "leaving, everything is at rest within 300 ms and the clock stops" test "${rest}" == "[true,false]"
# Nothing runs while the pointer is away
count="$(ev "${S}.motionCount")"
running=false
for i in $(seq 1 12); do
    vp "move(960 + 300 * Math.cos(${i}), 400 + 200 * Math.sin(${i})); 1" >/dev/null
    [[ "$(ev "${S}.frameClockRunning")" == true ]] && running=true
    sleep 0.25
done
sleep 5
check "a pointer elsewhere costs nothing (the motion count and the clock)" test "$(ev "${S}.motionCount") ${running}" == "${count} false"
# The frame budget over a sweep
ev "${DOCK}._frames = {count: 0, sum: 0, max: 0}; const it = items(); const y = mid(it[0].rest)[1]; let x = it[0].rest.x - 10; const end = it[it.length - 1].rest.x + 70; let dir = 4; const t0 = GLib.get_monotonic_time(); GLib.timeout_add(GLib.PRIORITY_HIGH, 16, () => { x += dir; if (x > end || x < it[0].rest.x - 10) dir = -dir; move(x, y); if (GLib.get_monotonic_time() - t0 > 5000000) { move(960, 400); return false; } return true; }); 1" >/dev/null
sleep 6.5
frames="$(ev "JSON.stringify(${S}.frameMs)")"
# The spec's budget is a machine's with a GPU. Rendered in software (the lab, as GNOME's own check for animations
# finds), llvmpipe's threads share the CPU and a collection can pause a frame: there the work stays under 1 ms on
# average and no frame of it over one frame at 60 Hz
if [[ "$(ev "global.backend.is_rendering_hardware_accelerated()")" == false ]]; then
    avg=1.0 max=16.7 where="software rendering"
else
    avg=0.3 max=1.0 where="the spec"
fi
check "the swell's frames: some drawn, under ${avg} ms on average and ${max} ms at most (${where}: ${frames})" sh -c "$(declare -f near); test $(jq .count <<<"${frames}") -gt 0 && near $(jq .avg <<<"${frames}") 0 ${avg} && near $(jq .max <<<"${frames}") 0 ${max}"
# Lift, then Still
in_session "ame desktop dock magnify lift" >/dev/null
apps_at 4 0
values="$(row)"
check "Lift: 1.22 and 10 px up, neighbours 1.04 and 3 px" sh -c "$(declare -f near); near $(jq '.[4][0]' <<<"${values}") 1.22 0.01 && near $(jq '.[4][2]' <<<"${values}") -10 0.5 && near $(jq '.[3][0]' <<<"${values}") 1.04 0.01 && near $(jq '.[5][2]' <<<"${values}") -3 0.5 && near $(jq '.[0][0]' <<<"${values}") 1 0.001"
in_session "ame desktop dock magnify off" >/dev/null
away
count="$(ev "${S}.motionCount")"
apps_at 4 0
# ...with the pointer on item 4, or nothing growing would prove nothing
check "Still: nothing grows, and the motion is not even listened to" test "$(ev "(() => { const [x, y] = global.get_pointer(); const r = items().filter(i => i.kind === 'app')[4].rest; return JSON.stringify([x >= r.x && x <= r.x + r.width && y >= r.y && y <= r.y + r.height, items().every(i => i.scale === 1), ${S}.motionCount === ${count}]); })()")" == "[true,true,true]"
in_session "ame desktop dock magnify swell" >/dev/null
away

# --- Starting an app -------------------------------------------------------------------------------------------------
# A click on an item, as a person makes it
click_item() {
    local where
    where="$(ev "(() => { const r = ${S}.docks[0].items.find(i => i.id === '$1').rest; return ((r.x + r.width / 2) / global.stage.width).toFixed(4) + ',' + ((r.y + r.height / 2) / global.stage.height).toFixed(4); })()")"
    keys "${2:-click}:${where}" >/dev/null
}
user_bus gsettings set org.gnome.shell favorite-apps "['org.mozilla.firefox.desktop', 'org.gnome.Nautilus.desktop', \
'amethystora-manual.desktop', 'org.test.A.desktop', 'org.test.Sleep.desktop']"
sleep 2
# The ring sampled every 20 ms from before the click: the Manual's window can come before any one late look
ev "global._vmlabRing = []; let n = 0; GLib.timeout_add(0, 20, () => { global._vmlabRing.push([GLib.get_monotonic_time(), item('amethystora-manual.desktop')?.ringOpacity ?? 0]); return ++n < 500; }); 1" >/dev/null
click_item amethystora-manual.desktop
wait_for 40 ev_is "${S}.lastLaunch.firstWindow > 0" true
ring="$(ev "(() => { const L = ${S}.lastLaunch; const s = global._vmlabRing.find(s => s[1] > 0 && s[0] >= L.started); return L.app === 'amethystora-manual.desktop' && s ? Math.round((s[0] - L.started) / 1000) : 'none'; })()")"
check "Halo: a ring shows within 100 ms of the Manual's start (${ring} ms, looked at every 20 ms)" \
    bash -c "[[ '${ring}' =~ ^[0-9]+\$ ]] && (( ${ring//none/999} <= 120 ))"
launch="$(ev "JSON.stringify(${S}.lastLaunch)")"
check "the ring ends with the Manual's first window" sh -c "test \$(jq '.ended - .firstWindow' <<<'${launch}') -le 200000 && test \$(jq .firstWindow <<<'${launch}') -gt 0 || { echo '${launch}'; false; }"
sleep 0.4
check "the ring is gone 300 ms later" test "$(ev "${S}.docks[0].items.find(i => i.id === 'amethystora-manual.desktop').ringOpacity")" == 0
ev "global.get_window_actors().map(a => a.meta_window).filter(w => w.get_wm_class()?.includes('Manual')).forEach(w => w.delete(global.display.get_current_time_roundtrip())); 1" >/dev/null
# Hop and Breathe on the test app, which opens its window 1.5 s after it starts
for kind in hop breathe; do
    in_session "ame desktop dock launch ${kind}" >/dev/null
    in_session "pkill -f 'win.js org.test.A '" >/dev/null 2>&1
    sleep 1
    ev "global._vmlabSamples = []; const G = imports.gi.GLib; let n = 0; G.timeout_add(0, 50, () => { const i = ${S}.docks[0].items.find(i => i.id === 'org.test.A.desktop'); global._vmlabSamples.push([i.iconBin.translation, i.iconBin.opacity]); return ++n < 200; }); 1" >/dev/null
    click_item org.test.A.desktop
    wait_for 40 ev_is "${S}.lastLaunch.app === 'org.test.A.desktop' && ${S}.lastLaunch.firstWindow > 0" true
    sleep 0.5
    samples="$(ev "JSON.stringify(global._vmlabSamples)")"
    if [[ ${kind} == hop ]]; then
        check "Hop: the icon rises 26 px" sh -c "$(declare -f near); near $(jq '[.[][0]] | min' <<<"${samples}") -26 2"
        check "Hop: it is back at 0 after the window" test "$(jq '.[-1][0]' <<<"${samples}")" == 0
    else
        check "Breathe: the picture dims to 179" sh -c "$(declare -f near); near $(jq '[.[][1]] | min' <<<"${samples}") 179 10"
        check "Breathe: it is whole again after the window" test "$(jq '.[-1][1]' <<<"${samples}")" == 255
    fi
done
in_session "pkill -f 'win.js org.test.A '" >/dev/null 2>&1
in_session "ame desktop dock launch halo" >/dev/null
click_item org.test.Sleep.desktop
sleep 6.6
launch="$(ev "JSON.stringify(${S}.lastLaunch)")"
check "a start that opens no window ends after 6 s" sh -c "test \"\$(jq -r .app <<<'${launch}')\" = org.test.Sleep.desktop && $(declare -f near); near \$(jq '(.ended - .started) / 1000000' <<<'${launch}') 6 0.3"

# --- What the dock holds ---------------------------------------------------------------------------------------------
user_bus gsettings set org.gnome.shell favorite-apps "${FAVORITES}"
in_session "ame desktop dock indicator bar" >/dev/null
user_bus systemd-run --user --collect --unit="vmlab-dock-files1-$$" nautilus --new-window >/dev/null
wait_for 60 ev_ge "global.get_window_actors().filter(a => a.meta_window.get_wm_class() === 'org.gnome.Nautilus').length" 1
sleep 1
mark="$(ev "JSON.stringify(${S}.docks[0].items.find(i => i.id === 'org.gnome.Nautilus.desktop').mark)")"
check "Bar: Files with one window has a 9 x 4 bar" test "$(jq -c '[.width, .height, .visible]' <<<"${mark}")" == "[9,4,true]"
user_bus systemd-run --user --collect --unit="vmlab-dock-files2-$$" nautilus --new-window >/dev/null
wait_for 60 ev_ge "global.get_window_actors().filter(a => a.meta_window.get_wm_class() === 'org.gnome.Nautilus').length" 2
sleep 1
geometry="$(ev "(() => { const d = ${S}.docks[0]; const i = d.items.find(i => i.id === 'org.gnome.Nautilus.desktop'); return [i.mark.width, i.mark.height, Math.round(d.pill.y + d.pill.height - i.mark.y - i.mark.height)].join(' '); })()")"
check "Bar: two windows, an 18 x 4 bar 3 px above the pill's edge" test "${geometry}" == "18 4 3"
vp "const p = mid(item('org.gnome.Nautilus.desktop').rest); move(p[0] + 20, p[1]); 1" >/dev/null
settle
centred="$(ev "(() => { const i = item('org.gnome.Nautilus.desktop'); return Math.abs(i.mark.x + i.mark.width / 2 - (i.shown.x + i.shown.width / 2)); })()")"
check "the bar stays under the swollen icon" near "${centred}" 0 0.5
away
in_session "ame desktop dock indicator dot" >/dev/null
sleep 1
check "Dot: one for each window" test "$(ev "[...${DOCK}.docks[0].dash._items.values()].find(s => s.key.get_id?.() === 'org.gnome.Nautilus.desktop' && !s.underFolder).marks.mark.get_n_children()")" == 2
in_session "ame desktop dock indicator glow" >/dev/null
sleep 1
check "Glow: 40 x 13" test "$(ev "(() => { const m = item('org.gnome.Nautilus.desktop').mark; return m.width + ' ' + m.height; })()")" == "40 13"
in_session "ame desktop dock indicator bar" >/dev/null

# Previews: a rest of 300 ms shows nothing, 500 ms the two windows 14 px above the pill
vp "const p = mid(item('org.gnome.Nautilus.desktop').rest); move(p[0], p[1]); 1" >/dev/null
sleep 0.3
vp "move(960, 400); 1" >/dev/null
sleep 0.5
check "a rest of 300 ms shows no previews" test "$(ev "${S}.previews.shown")" == false
vp "const p = mid(item('org.gnome.Nautilus.desktop').rest); move(p[0], p[1]); 1" >/dev/null
sleep 0.9
previews="$(ev "(() => { const s = ${S}; const p = s.previews; return JSON.stringify({shown: p.shown, windows: p.windows, gap: Math.round(s.docks[0].pill.y - p.card.y - p.card.height), tiles: ${DOCK}.docks[0].dash.previews.card.get_children().map(t => t.width + 'x' + t.height)}); })()")"
check "previews: two windows, 150 x 96, 14 px above the pill" test "$(jq -c . <<<"${previews}")" == '{"shown":true,"windows":2,"gap":14,"tiles":["150x96","150x96"]}'
before="$(ev "global.display.focus_window?.get_id()")"
vp "const t = ${DOCK}.docks[0].dash.previews.card.get_children()[1]; const [x, y] = t.get_transformed_position(); click(x + 75, y + 50); 1" >/dev/null
sleep 0.8
check "a click on the second preview gives that window the focus and closes the card" test \
    "$(ev "JSON.stringify([global.display.focus_window?.get_id() !== ${before:-0}, ${S}.previews.shown])")" == "[true,false]"
in_session "ame desktop dock previews off" >/dev/null
vp "const p = mid(item('org.gnome.Nautilus.desktop').rest); move(p[0] + 1, p[1]); 1" >/dev/null
sleep 1
check "with previews off, resting shows nothing" test "$(ev "${S}.previews.shown")" == false
in_session "ame desktop dock previews on" >/dev/null
away

# The fan: a right click on Files, Right, Space; Escape on another; an app with no window keeps GNOME's menu
vp "const p = mid(item('org.gnome.Nautilus.desktop').rest); click(p[0], p[1], 3); 1" >/dev/null
sleep 0.2
sleep 1
fan="$(ev "JSON.stringify({fan: ${S}.fan, menu: ${DOCK}.fan._menu?._getMenuItems().filter(i => i.actor.visible).map(i => i.label?.text).filter(Boolean), titles: ${DOCK}.fan.windows.map(w => w.title)})")"
check "the fan: open with two windows, GNOME's menu without the window list, with Quit 2 windows" \
    sh -c "jq -e '.titles as \$titles | .fan.open and .fan.windows == 2 and (.menu | index(\"Quit 2 windows\")) and (.titles | length == 2) and ([.menu[] | select(. as \$m | \$titles | index(\$m))] | length == 0)' <<<'${fan}' >/dev/null"
keys right >/dev/null
check "Right steps the ring to the second window" test "$(ev "${S}.fan.ringed")" == 1
second="$(ev "${DOCK}.fan.windows[1]?.get_id()")"
keys space >/dev/null
sleep 0.5
check "Space gives the ringed window the focus and closes the fan" test "$(ev "JSON.stringify([global.display.focus_window?.get_id(), ${S}.fan.open])")" == "[${second},false]"
vp "const p = mid(item('org.gnome.Nautilus.desktop').rest); click(p[0], p[1], 3); 1" >/dev/null
sleep 1
focus="$(ev "global.display.focus_window?.get_id()")"
keys esc >/dev/null
sleep 0.5
check "Escape closes the fan and changes nothing" test "$(ev "JSON.stringify([global.display.focus_window?.get_id(), ${S}.fan.open, Main.modalCount])")" == "[${focus},false,0]"
# A line of its menu, by the pointer: the click goes on after the fan closes, so an "already disposed" fails this suite
pinned="$(user_bus gsettings get org.gnome.shell favorite-apps)"
vp "const p = mid(item('org.gnome.Nautilus.desktop').rest); click(p[0], p[1], 3); 1" >/dev/null
sleep 1
vp "const line = ${DOCK}.fan._menu._toggleFavoriteItem; const [x, y] = line.get_transformed_position(); const [w, h] = line.get_transformed_size(); click(x + w / 2, y + h / 2, 1); 1" >/dev/null
sleep 0.5
check "a click on a line of the fan's menu closes the fan" test "$(ev "JSON.stringify([${S}.fan.open, Main.modalCount])")" == "[false,0]"
user_bus gsettings set org.gnome.shell favorite-apps "${pinned}"
settle
# Firefox, pinned by the image and without a window here
vp "const p = mid(item('org.mozilla.firefox.desktop').rest); click(p[0], p[1], 3); 1" >/dev/null
sleep 0.8
check "an app with no window opens GNOME's menu, not the fan" test "$(ev "JSON.stringify([${S}.fan.open, Boolean([...${DOCK}.docks[0].dash._items.values()].find(s => s.key.get_id?.() === 'org.mozilla.firefox.desktop')?.button._menu?.isOpen)])")" == "[false,true]"
# The icon swelled under the pointer and went back to rest as the menu opened: laid out again, the menu stays where it is
menu_box="[...${DOCK}.docks[0].dash._items.values()].find(s => s.key.get_id?.() === 'org.mozilla.firefox.desktop').button._menu.actor"
menu_at="$(ev "Math.round(${menu_box}.get_transformed_position()[1])")"
ev "${menu_box}.queue_relayout(); 1" >/dev/null
sleep 0.3
check "...and it has settled by then: laid out again, it does not move" test "$(ev "Math.round(${menu_box}.get_transformed_position()[1])")" == "${menu_at}"
keys esc >/dev/null
skip "AT-SPI reading the fan's menu items: GNOME Shell's menus are checked by 27-amethystora-ui.sh"

# Progress: the launcher protocol's progress on a pinned app, from a sender that stays on the bus (the dock forgets what
# an app said once it leaves, and gdbus emit leaves at once), for 8 s
user_bus gjs -m "${HOME_DIR}/.local/share/vmlab-dock/entry.js" org.mozilla.firefox.desktop 0.5 8 >/dev/null 2>&1 &
sleep 1.5
progress="$(ev "(() => { const d = ${DOCK}.docks[0].dash; const s = [...d._items.values()].find(s => s.key.get_id?.() === 'org.mozilla.firefox.desktop'); const p = s.badge.progress; const f = p.get_first_child(); const icon = s.button.icon.icon; return [p.visible, Math.round(p.height), Math.round(f.width / (p.width - 2 * 0) * 100), Math.round(icon.width - p.width)].join(' '); })()")"
check "progress: a 5 px bar at half, 4 px in from each side of the icon" test "${progress}" == "true 5 50 8"

# An app Flatpak is updating goes grey, from the two things a real update leaves on disk (dock/flatpak.js; the real ones
# were watched on 2026-10-31, D-GRID-07 in tests/real/checklist): a download's folder named for the app, with no system
# helper behind it here, so that the slow check takes it for one that died; and a staging folder whose commit names the
# app, under a held lock
FP_TMP="${HOME_DIR}/.local/share/flatpak/repo/tmp"
GREY="[...${DOCK}.docks[0].dash._items.values()].find(s => s.key.get_id?.() === 'org.mozilla.firefox.desktop').badge._baseIcon._iconBin.opacity"
cat >"${HOME_DIR}/.local/share/vmlab-dock/commit.js" <<'EOF'
import GLib from 'gi://GLib';
const commit = new GLib.Variant('(a{sv}aya(say)sstayay)', [{'ostree.ref-binding': new GLib.Variant('as', ['app/org.mozilla.firefox/x86_64/stable'])}, [], [], '', '', 0, [], []]);
GLib.file_set_contents(ARGV[0], commit.get_data_as_bytes().get_data());
EOF
chown "${TEST_USER}:" "${HOME_DIR}/.local/share/vmlab-dock/commit.js"
# A repository's folder that was not there at the start counts from the next start of the watcher
in_session "mkdir -p ${FP_TMP} /var/tmp/flatpak-cache-vmlab; ame desktop dock updating off; ame desktop dock updating on" >/dev/null
sleep 1
in_session "mkdir /var/tmp/flatpak-cache-vmlab/org.mozilla.firefox-ABCDEF"
sleep 1
check "a download's folder greys the app" test "$(ev "${GREY}")" == 128
check "...and does not stop it starting" test "$(ev "String(Boolean(${DOCK}.launcher.get('org.mozilla.firefox.desktop').updating))")" == false
check "...and one with nothing behind it stops counting within seconds" wait_for 10 ev_is "${GREY}" 255
in_session "rm -rf /var/tmp/flatpak-cache-vmlab"
start_staging() {
    in_session "mkdir -p ${FP_TMP}/staging-vmlab$1/ab; touch ${FP_TMP}/staging-vmlab$1-lock"
    user_bus python3 -c 'import fcntl,os,sys,time; fd=os.open(sys.argv[1],os.O_RDWR); fcntl.flock(fd,fcntl.LOCK_EX); time.sleep(int(sys.argv[2]))' "${FP_TMP}/staging-vmlab$1-lock" "$2" &
    sleep 0.5
    in_session "gjs -m ${HOME_DIR}/.local/share/vmlab-dock/commit.js ${FP_TMP}/staging-vmlab$1/ab/cd.commit"
    sleep 1
}
start_staging 1 12
check "a staging folder whose commit names the app greys it" test "$(ev "${GREY}")" == 128
sleep 6
check "...and stays grey past the slow check while its lock is held" test "$(ev "${GREY}")" == 128
in_session "rm -rf ${FP_TMP}/staging-vmlab1 ${FP_TMP}/staging-vmlab1-lock"
check "...and the lock's deletion ends it" wait_for 4 ev_is "${GREY}" 255
start_staging 2 2
check "an update that died (its lock let go, its folder left behind) stops counting within seconds" wait_for 12 ev_is "${GREY}" 255
in_session "rm -rf ${FP_TMP}/staging-vmlab2 ${FP_TMP}/staging-vmlab2-lock; ame desktop dock updating off"
in_session "mkdir -p /var/tmp/flatpak-cache-vmlab/org.mozilla.firefox-ABCDEF"
sleep 1
check "with the switch off nothing greys" test "$(ev "${GREY}")" == 255
in_session "rm -rf /var/tmp/flatpak-cache-vmlab; ame desktop dock updating on" >/dev/null

# Recent: A, B, C, D opened and quit in that order show D, C, B; off, and with GNOME's usage history off, none
for name in A B C D; do
    ev "imports.gi.Shell.AppSystem.get_default().lookup_app('org.test.${name}.desktop').activate(); 1" >/dev/null
    wait_for 40 ev_is "global.get_window_actors().filter(a => a.meta_window.title === 'Test${name} 1').length" 1
    ev "Main.activateWindow(global.get_window_actors().map(a => a.meta_window).find(w => w.title === 'Test${name} 1')); 1" >/dev/null
    sleep 1
    ev "imports.gi.Shell.AppSystem.get_default().lookup_app('org.test.${name}.desktop').request_quit(); 1" >/dev/null
    wait_for 20 ev_is "imports.gi.Shell.AppSystem.get_default().lookup_app('org.test.${name}.desktop').state" 0
done
sleep 1
check "recent apps: D, C, B, newest first" test "$(ev "JSON.stringify(${S}.recent.slice(0, 3))")" == \
    '["org.test.D.desktop","org.test.C.desktop","org.test.B.desktop"]'
# A row redrawn while a recent app pops in takes its icon out of the row and ends the pop: the icon still comes to rest,
# never left clear in its slot. Quit and redrawn just after, by what a focus change or another app ending would do
ev "imports.gi.Shell.AppSystem.get_default().lookup_app('org.test.A.desktop').activate(); 1" >/dev/null
wait_for 40 ev_is "global.get_window_actors().filter(a => a.meta_window.title === 'TestA 1').length" 1
ev "Main.activateWindow(global.get_window_actors().map(a => a.meta_window).find(w => w.title === 'TestA 1')); 1" >/dev/null
sleep 1
ev "(() => { const {Shell, GLib} = imports.gi; const app = Shell.AppSystem.get_default().lookup_app('org.test.A.desktop'); const id = app.connect('notify::state', () => { if (app.state !== Shell.AppState.STOPPED) return; app.disconnect(id); GLib.timeout_add(GLib.PRIORITY_DEFAULT, 30, () => { ${DOCK}.docks[0].dash._queueRedisplay(); return GLib.SOURCE_REMOVE; }); }); app.request_quit(); return 1; })()" >/dev/null
wait_for 20 ev_is "imports.gi.Shell.AppSystem.get_default().lookup_app('org.test.A.desktop').state" 0
sleep 1
check "a recent app's icon comes to rest though the row is redrawn as it pops in" test "$(ev "(() => { const d = ${DOCK}.docks[0].dash; const icon = [...d._recentItems].find(([app]) => app.get_id() === 'org.test.A.desktop')[1].child; return [icon.opacity, icon.scale_x, Boolean(icon._amethystoraBusy)].join(' '); })()")" == "235 1 false"
in_session "ame desktop dock recent off" >/dev/null
sleep 1
check "recent off: no recent items and no focus signal" test "$(ev "JSON.stringify([${S}.recentShown.length, ${DOCK}.recent === null])")" == "[0,true]"
in_session "ame desktop dock recent on" >/dev/null
user_bus gsettings set org.gnome.desktop.privacy remember-app-usage false
sleep 1
check "with GNOME's usage history off, there are none" test "$(ev "${S}.recentShown.length")" == 0
user_bus gsettings reset org.gnome.desktop.privacy remember-app-usage

# Folders: B's icon dragged onto the middle of the Manual's and held, then released
user_bus gsettings set org.gnome.shell favorite-apps "${FAVORITES}"
user_bus gsettings set org.gnome.shell favorite-apps "['org.mozilla.firefox.desktop', 'org.gnome.Nautilus.desktop', \
'amethystora-manual.desktop', 'amethystora-notes.desktop', 'amethystora-control.desktop']"
sleep 2
DRAG="global._vmlabDrag = (from, to, hold) => { move(...from); global._vmlabVp.notify_button(GLib.get_monotonic_time(), 1, Clutter.ButtonState.PRESSED); let i = 0; GLib.timeout_add(0, 16, () => { i++; move(from[0] + (to[0] - from[0]) * i / 25, from[1] + (to[1] - from[1]) * i / 25); if (i < 25) return true; GLib.timeout_add(0, hold, () => { global._vmlabVp.notify_button(GLib.get_monotonic_time(), 1, Clutter.ButtonState.RELEASED); return false; }); return false; }); };"
vp "${DRAG} global._vmlabDrag(mid(item('amethystora-notes.desktop').rest), mid(item('amethystora-manual.desktop').rest), 100); 1" >/dev/null
sleep 2
check "a drop released at once is GNOME's reordering, not a folder" test "$(dget folders)" == "@a{sas} {}"
user_bus gsettings set org.gnome.shell favorite-apps "['org.mozilla.firefox.desktop', 'org.gnome.Nautilus.desktop', \
'amethystora-manual.desktop', 'amethystora-notes.desktop', 'amethystora-control.desktop']"
sleep 2
vp "${DRAG} global._vmlabDrag(mid(item('amethystora-notes.desktop').rest), mid(item('amethystora-manual.desktop').rest), 400); 1" >/dev/null
sleep 2.5
check "held 400 ms on the middle of another, it makes a folder (the target first)" test "$(dget folders)" == \
    "{'Folder': ['amethystora-manual.desktop', 'amethystora-notes.desktop']}"
check "the row shows one folder where the Manual was, the favourites unchanged" test \
    "$(ev "items().map(i => i.kind).slice(0, 4).join(' ')") $(user_bus gsettings get org.gnome.shell favorite-apps | grep -o desktop | wc -l)" == "app app folder app 5"
vp "click(...mid(item('Folder').rest)); 1" >/dev/null
sleep 0.8
check "a click opens the folder's card" test "$(ev "${S}.folderCard.open")" == true
vp "const n = ${DOCK}._card.card.get_last_child().child; const [x, y] = n.get_transformed_position(); click(x + n.width / 2, y + n.height / 2); 1" >/dev/null
sleep 0.5
# keys.py's text: types lower case only: the capital is a chord of its own
keys ctrl+a shift+w text:riting return >/dev/null
sleep 1
check "its name is changed by typing on it" test "$(dget folders)" == "{'Writing': ['amethystora-manual.desktop', 'amethystora-notes.desktop']}"
keys esc >/dev/null
sleep 0.5
ev "(() => { const st = [...${DOCK}.docks[0].dash._items.values()].find(s => s.key.folder); st.button.popupMenu(); const take = st.button._menu._getMenuItems().find(i => i.label?.text === 'Take the folder apart'); take.activate(null); })(); 1" >/dev/null
sleep 1.5
check "Take the folder apart puts both back" test "$(dget folders) $(ev "items().filter(i => i.kind === 'app').length")" == "@a{sas} {} 5"
keys esc >/dev/null

# Stacks: the twelve newest of Downloads, newest first, no hidden file, a name shown as text; read only on opening
in_session "rm -rf ~/Downloads/vmlab; mkdir -p ~/Downloads" >/dev/null
in_session 'cd ~/Downloads && for i in $(seq 1 15); do echo $i >"file-$i.txt"; touch -d "-$((16 - i)) minutes" "file-$i.txt"; done; echo h >.hidden; echo x >"<b>bold<i> & co.txt"; touch -d "-30 seconds" "<b>bold<i> & co.txt"'
reads="$(ev "${S}.stackReads")"
vp "click(...mid(item('Downloads').rest)); 1" >/dev/null
sleep 1.2
stack="$(ev "JSON.stringify(${S}.stackCard)")"
check "the stack: twelve entries, newest first, no hidden one" sh -c "jq -e '.open and (.entries | length == 12) and .entries[0] == \"<b>bold<i> & co.txt\" and .entries[1] == \"file-15.txt\" and (.entries | index(\".hidden\") | not)' <<<'${stack}' >/dev/null"
check "a name with markup is shown as its characters" test "$(ev "${DOCK}._card.card.get_child_at_index(1).get_children()[0].accessible_name")" == "<b>bold<i> & co.txt"
keys esc >/dev/null
in_session 'cd ~/Downloads && echo new >newest.txt'
sleep 10
check "a shut stack reads nothing" test "$(ev "${S}.stackReads")" == $((reads + 1))
vp "click(...mid(item('Downloads').rest)); 1" >/dev/null
sleep 1.2
check "reopened, it shows the new file first" test "$(ev "${S}.stackCard.entries[0]")" == newest.txt
keys esc >/dev/null

# Drops onto a stack, through Files: a move, a copy, and nothing for files already there
in_session 'rm -rf ~/Documents/vmlab ~/Pictures/vmlab-*; mkdir -p ~/Documents/vmlab ~/Pictures && for n in 1 2 3; do echo $n >~/Documents/vmlab/vmlab-$n.txt; done'
in_session "ame desktop dock stack add ~/Pictures" >/dev/null
sleep 1
ev "(() => { const d = ${DOCK}.docks[0].dash; const st = [...d._items.values()].find(s => s.key.stack && s.key.path.endsWith('/Pictures')); const files = [1, 2, 3].map(n => { const p = imports.gi.GLib.get_home_dir() + '/Documents/vmlab/vmlab-' + n + '.txt'; return {uri: imports.gi.Gio.File.new_for_path(p).get_uri(), path: p}; }); ${DOCK}.drops.drop({kind: 'stack', dash: d, state: st, actor: st.item}, files, false); })(); 1" >/dev/null
# The +3 shows once Files has taken the files, for 2 s and a fade of 0.3 s: looked for as soon as the drop is made
badge=missing
for _ in $(seq 1 60); do
    [[ "$(ev "${DOCK}.drops._badges?.length ?? 0")" =~ ^[1-9] ]] && { badge=shown; break; }
    sleep 0.2
done
sleep 2
check "a drop of three files on the Pictures stack moves them, through Files" \
    test "$(in_session 'ls ~/Pictures | grep -c vmlab-; ls ~/Documents/vmlab | wc -l' | xargs)" == "3 0"
check "the drop is recorded as three moved" test "$(ev "JSON.stringify([${S}.lastDrop.copy, ${S}.lastDrop.sent])")" == "[false,3]"
check "the stack shows +3" test "${badge}" == shown
sleep 0.6
check "the +3 is gone after 2.5 s" test "$(ev "${DOCK}.drops._badges?.length ?? 0")" == 0
in_session 'mkdir -p ~/Documents/vmlab && echo c >~/Documents/vmlab/vmlab-copy.txt'
ev "(() => { const d = ${DOCK}.docks[0].dash; const st = [...d._items.values()].find(s => s.key.stack && s.key.path.endsWith('/Pictures')); const p = imports.gi.GLib.get_home_dir() + '/Documents/vmlab/vmlab-copy.txt'; ${DOCK}.drops.drop({kind: 'stack', dash: d, state: st, actor: st.item}, [{uri: imports.gi.Gio.File.new_for_path(p).get_uri(), path: p}], true); })(); 1" >/dev/null
sleep 4
check "with Ctrl a drop copies" test "$(in_session 'ls ~/Pictures/vmlab-copy.txt ~/Documents/vmlab/vmlab-copy.txt 2>/dev/null' | wc -l)" == 2
skip "a drag from a Files window onto the stack, and Escape over it: the lab's keys.py drives the pointer, but the selection's place in Files is not known on Wayland; the drop path itself is checked above (spec D13)"
in_session "ame desktop dock stack remove ~/Pictures" >/dev/null
check "a stack taken away leaves its folder" test -d "${HOME_DIR}/Pictures"
check "Stacks: removing one changes only the key" test "$(dget stacks)" == "['xdg:DOWNLOAD']"

# The phone: with the phone link off, the Phone item says so and takes nothing
if user_bus systemctl --user is-active --quiet amethystora-connect.service; then
    skip "a drop on the phone with the phone link up: 95-phone-link.sh's stand-in phone is needed"
else
    why="$(ev "(() => { const d = ${DOCK}.docks[0].dash; return ${DOCK}.drops._phones().why; })()")"
    check "with the phone link off, the Phone item says so" test "${why}" == "Phone pairing is off"
fi

# Dissolve: Notes dragged to the middle of the screen and let go
user_bus gsettings set org.gnome.shell favorite-apps "['org.mozilla.firefox.desktop', 'org.gnome.Nautilus.desktop', \
'amethystora-manual.desktop', 'amethystora-notes.desktop', 'amethystora-control.desktop']"
sleep 2
vp "${DRAG} global._vmlabDrag(mid(item('amethystora-notes.desktop').rest), [960, 500], 50); 1" >/dev/null
sleep 0.7
specks="$(ev "${S}.dissolving")"
sleep 1.2
check "dragged off and let go, Notes is unpinned and dissolves" test \
    "$(user_bus gsettings get org.gnome.shell favorite-apps | grep -c amethystora-notes) ${specks} $(ev "${S}.dissolving")" == "0 true false"
check "it is still in the app grid" test "$(ev "Boolean(imports.gi.Shell.AppSystem.get_default().lookup_app('amethystora-notes.desktop'))")" == true
vp "${DRAG} global._vmlabDrag(mid(item('amethystora-control.desktop').rest), [mid(item('amethystora-control.desktop').rest)[0], ${S}.docks[0].pill.y - 10], 50); 1" >/dev/null
sleep 2
check "let go 10 px above the pill, it stays pinned" test "$(user_bus gsettings get org.gnome.shell favorite-apps | grep -c amethystora-control)" == 1

# --- Numbers ---------------------------------------------------------------------------------------------------------
user_bus gsettings set org.gnome.shell favorite-apps "['org.mozilla.firefox.desktop', 'org.gnome.Nautilus.desktop', \
'amethystora-manual.desktop', 'amethystora-notes.desktop', 'amethystora-control.desktop']"
in_session "ame desktop dock folder Pair amethystora-manual.desktop amethystora-notes.desktop" >/dev/null
sleep 2
ev "${DOCK}.showNumbers(); 1" >/dev/null
sleep 0.6
numbers="$(ev "JSON.stringify(${DOCK}.numberedItems().map(n => [n.number, n.disc ? Math.round(n.disc.width) : 0]))")"
check "numbers 1 to 5 in the favourites' order, the folder's two members numbered but drawn nowhere" test "${numbers}" == "[[1,24],[2,24],[3,0],[4,0],[5,24]]"
corner="$(ev "(() => { const n = ${DOCK}.numberedItems()[0]; const st = [...${DOCK}.docks[0].dash._items.values()].find(s => s.key.get_id?.() === n.appId); const b = ${DOCK}.docks[0].dash.iconRestRect(st); return [Math.round(n.disc.x + 12 - b.x - b.width), Math.round(b.y - n.disc.y - 12)].join(' '); })()")"
check "a disc's centre is 6 px beyond the icon's top far corner" test "${corner}" == "6 6"
ev "${DOCK}.hideNumbers(); 1" >/dev/null
sleep 0.25
check "hidden within 200 ms" test "$(ev "[...${DOCK}.docks[0].dash._items.values()].filter(s => s.marks?.numberShown).length")" == 0
in_session "ame desktop dock folder Pair" >/dev/null
keys alt+3 >/dev/null
sleep 3
check "Alt+3 opens the app numbered 3" test "$(ev "imports.gi.Shell.AppSystem.get_default().lookup_app('amethystora-manual.desktop').state")" -gt 0
ev "global.get_window_actors().map(a => a.meta_window).filter(w => w.get_wm_class()?.includes('Manual')).forEach(w => w.delete(global.display.get_current_time_roundtrip())); 1" >/dev/null

# --- Where the dock lives --------------------------------------------------------------------------------------------
work() { ev "(() => { const a = global.workspace_manager.get_active_workspace().get_work_area_for_monitor(0); const s = ${S}.docks[0].strip; return JSON.stringify({a: [a.x, a.y, a.width, a.height], s: [s.x, s.y, s.width, s.height]}); })()"; }
w="$(work)"
check "bottom: the work area ends at the strip" sh -c "$(declare -f near); near $(jq '.a[1] + .a[3]' <<<"${w}") $(jq '.s[1]' <<<"${w}") 1"
in_session "ame desktop dock edge left" >/dev/null
sleep 2
w="$(work)"
check "left: the work area starts beside the strip" sh -c "$(declare -f near); near $(jq '.a[0]' <<<"${w}") $(jq '.s[2]' <<<"${w}") 1"
vp "const a = items().filter(i => i.kind === 'app'); const p = mid(a[2].rest); move(p[0], p[1]); 1" >/dev/null
settle
check "left: Swell runs along y" sh -c "$(declare -f near); near $(ev "items().filter(i => i.kind === 'app')[2].scale") 1.6 0.01"
away
in_session "ame desktop dock edge right" >/dev/null
sleep 2
w="$(work)"
check "right: the work area ends beside the strip" sh -c "$(declare -f near); near $(jq '.a[0] + .a[2]' <<<"${w}") $(jq '.s[0]' <<<"${w}") 1"
in_session "ame desktop dock edge bottom" >/dev/null
sleep 2

# Hiding when touched
in_session "ame desktop dock autohide touch" >/dev/null
sleep 2
w="$(work)"
check "touch: no strip is kept free" test "$(jq '.a[1] + .a[3]' <<<"${w}")" == "$(ev "global.stage.height")"
ev "const w = global.get_window_actors().map(a => a.meta_window).find(w => w.get_wm_class() === 'org.gnome.Nautilus'); w.maximize(imports.gi.Meta.MaximizeFlags.BOTH); Main.activateWindow(w); 1" >/dev/null
sleep 1
hidden="$(ev "JSON.stringify([${S}.docks[0].hidden, ${S}.docks[0].strip.translationY, ${S}.docks[0].strip.height, ${DOCK}.docks[0].autohide._hint.opacity])")"
check "a window touching it hides it, and the hint shows" sh -c "jq -e '.[0] and .[1] == .[2] + 4 and .[3] > 200' <<<'${hidden}' >/dev/null"
keys move:0.5,0.98 rel:0,20 rel:0,20 rel:0,20 rel:0,20 rel:0,20 rel:0,20 rel:0,20 rel:0,20 rel:0,20 rel:0,20 >/dev/null
sleep 0.6
check "pushing against the edge brings it back" test "$(ev "${S}.docks[0].hidden")" == false
keys move:0.5,0.5 >/dev/null
sleep 1.4
check "leaving, it hides again" test "$(ev "${S}.docks[0].hidden")" == true
ev "Main.overview.show(); 1" >/dev/null
sleep 1.5
check "the overview shows it" test "$(ev "${S}.docks[0].hidden")" == false
ev "Main.overview.hide(); global.get_window_actors().map(a => a.meta_window).filter(w => w.get_wm_class() === 'org.gnome.Nautilus').forEach(w => w.unmaximize(imports.gi.Meta.MaximizeFlags.BOTH)); 1" >/dev/null
check "the barrier exists only with touch" test "$(ev "${S}.docks[0].barrier")" == true
in_session "ame desktop dock autohide off" >/dev/null
sleep 1

# This workspace's apps only: Files on workspace 2 is not on workspace 1's dock
ev "global.get_window_actors().map(a => a.meta_window).filter(w => w.get_wm_class() === 'org.gnome.Nautilus').forEach(w => w.change_workspace_by_index(1, false)); 1" >/dev/null
user_bus gsettings set org.gnome.shell favorite-apps "['org.mozilla.firefox.desktop']"
sleep 1
in_session "ame desktop dock workspace-apps on" >/dev/null
sleep 1.5
check "with workspace-apps on, an app with windows elsewhere leaves the dock" test "$(ev "Boolean(item('org.gnome.Nautilus.desktop'))")" == false
in_session "ame desktop dock workspace-apps off" >/dev/null
sleep 1.5
check "...and comes back with it off" test "$(ev "Boolean(item('org.gnome.Nautilus.desktop'))")" == true
check "pinned apps stay either way" test "$(ev "Boolean(item('org.mozilla.firefox.desktop'))")" == true
user_bus gsettings set org.gnome.shell favorite-apps "${FAVORITES}"

# --- Teardown and cost -----------------------------------------------------------------------------------------------
in_session "ame desktop dock autohide touch" >/dev/null
sleep 1
# GNOME's own dash, hidden while the dock is on, redraws only once it is shown (deferred work): it is brought up to date
# before each count, so that its items for the favourites of an earlier step are not taken for the dock's leftovers.
# GNOME 50 turns an extension on and off through its settings, a moment later: each step waits for the state it asked
# for (1 on, 2 or 6 off), and a step that never comes, or a throw, is the answer instead of a hang
ev "global._vmlabCycle = 'running'; (async () => { try { const E = Main.extensionManager; const U = '${UUID}'; const wait = ms => new Promise(r => imports.gi.GLib.timeout_add(0, ms, () => { r(); return false; })); const until = async on => { for (let n = 0; n < 300; n++) { const s = E.lookup(U).state; if (on ? s === 1 : s === 2 || s === 6) return; await wait(100); } throw new Error('the extension did not turn ' + (on ? 'on' : 'off')); }; const count = () => { Main.overview.dash._redisplay(); return countNow(); }; const countNow = () => [Main.layoutManager.uiGroup.get_n_children(), Main.layoutManager._trackedActors.length, Main.overview.dash.visible, Main.modalCount].join(' '); const st = () => { const s = E.lookup(U).stateObj?.modules?.get('dock')?.state(); return s ? JSON.stringify([s.docks.map(d => [d.edge, d.touch, d.items.map(i => i.kind + i.id).join()]), s.folders, s.stacks, s.magnify]) : 'none'; }; const before = st(); E.disableExtension(U); await until(false); await wait(500); const off0 = count(); const names = () => [...Main.layoutManager.uiGroup.get_children(), ...Main.layoutManager._trackedActors.map(t => t.actor)].map(a => a.constructor.name + (a.name ? '#' + a.name : '') + (a.get_style_class_name?.() ? '.' + a.get_style_class_name() : '') + (a instanceof imports.gi.St.Label ? '[' + a.text + ']' + (a.visible ? '+' : '-') : '')); const n0 = names(); const run = () => imports.gi.Shell.AppSystem.get_default().get_running().map(x => x.get_id()).sort().join(','); const r0 = run(); const g0 = Main.overview.dash._box.get_children().map(c => c.child?.app?.get_id?.() ?? c.child?.constructor?.name).join(','); await wait(3000); const n0b = names(); for (let i = 0; i < 10; i++) { E.enableExtension(U); await until(true); await wait(300); E.disableExtension(U); await until(false); await wait(300); } await wait(500); const off10 = count(); const n10 = names(); const r10 = run(); const g10 = Main.overview.dash._box.get_children().map(c => c.child?.app?.get_id?.() ?? c.child?.constructor?.name).join(','); const late = n0.filter((v, i) => n0.indexOf(v) === i && n0.filter(x => x === v).length !== n0b.filter(x => x === v).length); const left = n0.filter((v, i) => n0.indexOf(v) === i && n0.filter(x => x === v).length !== n10.filter(x => x === v).length); E.enableExtension(U); await until(true); for (let n = 0; n < 50 && st() !== before; n++) await wait(100); global._vmlabCycle = [off0 === off10, st() === before, off0, off10, left.join(' ') + (late.length ? ' (3 s later already gone: ' + late.join(' ') + ')' : ''), (r0 === r10 ? '' : 'running ' + r0 + ' -> ' + r10) + (g0 === g10 ? '' : ' overview dash ' + g0 + ' -> ' + g10), st() === before ? '' : before + ' -> ' + st()].join(';'); } catch (e) { global._vmlabCycle = 'failed: ' + e.message; } })(); 1" >/dev/null
wait_for 300 ev_is "global._vmlabCycle === 'running'" false
cycle="$(ev "global._vmlabCycle")"
check "disabled and enabled ten times: nothing left behind (${cycle})" test "$(cut -d';' -f1 <<<"${cycle}")" == true
check "...and the dock's state is as before" test "$(cut -d';' -f2 <<<"${cycle}")" == true
in_session "ame desktop dock autohide off" >/dev/null
sleep 1
all_off() {
    for key in "magnify off" "launch off" "recent off" "previews off" "workspace-apps off" "autohide off" "screens primary"; do
        in_session "ame desktop dock ${key}" >/dev/null
    done
    sleep 2
}
all_off
base="$(ev "${S}.connections")"
for key in "magnify swell" "launch halo" "recent on" "previews on" "workspace-apps on" "autohide touch"; do
    in_session "ame desktop dock ${key}" >/dev/null
done
sleep 1
on="$(ev "${S}.connections")"
all_off
after="$(ev "${S}.connections")"
# Three answers, each a count: an error is no answer and never equal to another
check "with every key off the dock keeps its base connections (${base}, ${on} with them on), and comes back to them" \
    bash -c "[[ '${base}' =~ ^[0-9]+\$ && '${on}' =~ ^[0-9]+\$ && '${after}' == '${base}' && ${on:-0} -gt ${base:-0} ]]"
for key in magnify magnify-amount launch indicator recent previews edge screens autohide workspace-apps updating folders stacks; do
    in_session "gsettings reset ${DOCK_SCHEMA} ${key}"
done
user_bus gsettings set org.gnome.shell favorite-apps "${FAVORITES}"
sleep 2
loginctl lock-sessions
sleep 4
loginctl unlock-sessions
# GNOME turns the extensions on again after unlocking, a moment later
dock_back() { ev_is "JSON.stringify([Boolean(${DOCK}), ${S}.magnify, ${S}.docks.length > 0])" '[true,"swell",true]'; }
check "locked and unlocked, the dock is back with its keys" wait_for 30 dock_back

log="$(journalctl --since "${since}" "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --no-pager -o cat 2>/dev/null)"
errors="$(grep -E 'JS ERROR|Amethystora UI: the [a-z]+ module did not' <<<"${log}" | head -n 10)"
if [[ -z "${errors}" ]]; then ok "nothing threw in the shell meanwhile"; else not_ok "the shell's log has errors" "${errors}"; fi
user_bus systemctl --user stop "vmlab-dock-files1-$$" "vmlab-dock-files2-$$" >/dev/null 2>&1
finish
