#!/usr/bin/bash
# vmlab-vm: motion
# The droplet, the workspace switch with depth, and workspaces named after their apps (plans/beyond/spec-motion.md,
# section 7), in the test account's session, asked of GNOME Shell itself through Eval, which a lab-only extension of
# the test account opens (never in the image). The lab draws without a GPU, so GNOME turns its animations off: every
# animated check forces them on first (global.force_animations) and puts them back after.
#   - a droplet minimize and its way back end exactly as GNOME leaves a window, with the tiling and in the classic
#     layout; with animations off, with shrink and with Reduce motion they end the same, by GNOME's path or a fade;
#   - turned round part way, any number of times, the window ends as the last call wanted, and Mutter's accounting of
#     its effects never complains; a window closed in flight leaves nothing behind;
#   - an app opened from the dock grows out of its icon, one opened elsewhere does not;
#   - a step of a flight costs under a millisecond, and the ring ripples round the dock icon as the drop lands;
#   - a workspace switch dips the windows to 94 % and moves the wallpaper a third as far, in 0.6 x 560 ms; slide is
#     GNOME's own; Reduce motion is a crossfade;
#   - a workspace is named after its main app, a name of the account's own wins, space-names off takes it away, and
#     a window dragged onto a label lights it up, says where it goes, and lands there;
#   - the three ame desktop commands set their keys and refuse a wrong value;
#   - ten disable/enable cycles mid-flight leave GNOME's handlers working and nothing of the motion behind;
#   - nothing of Amethystora UI throws meanwhile.
# Not covered here (spec 7): 6 (a strip of three, the neighbours' places), 15 (frame intervals against shrink), 21 (four
# screens), 23's tie-break (a Text Editor window), 27 (GNOME's thumbnails), 30 (the power saver), 31 (Control).
# It leaves the account as it found it, however it ends: every key reset, animations as GNOME has them.
# vmlab-paths: @ui/modules/pill/spring.js @ui/modules/pill/clock.js @ui/extension.js @ui/*motion* @ui/*workspaces* @sf/*themed/shell-workspaces* @sf/*just/desktop.just @ui/*pill*
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HOME_DIR="/var/home/${TEST_USER}"
MOTION=org.gnome.shell.extensions.amethystora-ui.motion
WORKSPACES=org.gnome.shell.extensions.amethystora-ui.workspaces
keys() { python3 "$(dirname "$0")/keys.py" "$@"; }
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        sed -E -e "s/^\(true, '(.*)'\)\$/\1/" -e 's/^"(.*)"$/\1/'
}
gs() { in_session "gsettings $*" 2>/dev/null; }
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
MO="${UI}.get('motion')"
V="globalThis.vmlabMotion"
module_up() { [[ "$(ev "typeof ${MO}?.flying")" == function ]]; }
two_files() { [[ "$(ev "global.get_window_actors().filter(a => a.meta_window.get_wm_class() === 'org.gnome.Nautilus').length")" -ge 2 ]]; }
settled() { [[ "$(ev "${V}.settled()")" == true ]]; }
# Put back whatever the suite changed, however it ends
cleanup() {
    ev "global.force_animations = true; delete globalThis.vmlabMotion; 1" >/dev/null 2>&1
    gs reset "${MOTION}" minimize
    gs reset "${MOTION}" workspace-switch
    gs reset "${WORKSPACES}" auto-names
    gs reset org.gnome.desktop.a11y.interface reduced-motion
    gs reset org.gnome.desktop.wm.preferences workspace-names
}
since="$(date '+%Y-%m-%d %H:%M:%S')"

if ! has_session; then
    bash "$(dirname "$0")/10-account.sh" >/dev/null
fi
if ! wait_for 300 has_session; then
    not_ok "no session for ${TEST_USER}"
    finish
fi

gs reset "${MOTION}" minimize
gs reset "${MOTION}" workspace-switch
gs reset "${WORKSPACES}" auto-names
gs reset org.gnome.desktop.a11y.interface reduced-motion
gs reset org.gnome.desktop.wm.preferences workspace-names

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
wait_for 240 has_session && wait_for 120 module_up
sleep 10
check "the motion module runs" module_up

# The lab's helpers, in the shell: the two Files windows, a recorder of flights, and the end state of a window
ev "globalThis.vmlabMotion = (() => {
    const {GLib, GObject, Graphene, Meta} = imports.gi;
    const M = () => ${MO};
    const files = () => global.get_window_actors().filter(a => a.meta_window.get_wm_class() === 'org.gnome.Nautilus')
        .map(a => a.meta_window).sort((a, b) => a.get_stable_sequence() - b.get_stable_sequence());
    const self = {
        runs: [], steps: [],
        win: i => files()[i],
        // Records each flight: its kind, its length, every step's cost, and the scale and translation it wrote
        record() {
            self.runs = []; self.steps = [];
            for (const s of ['minimize', 'unminimize', 'map']) {
                global.window_manager.connectObject(s, (wm, a) => {
                    const fl = M().flight(a);
                    const run = {sig: s, kind: fl?.kind ?? 'gnome', t0: GLib.get_monotonic_time(), scale: [], tx: [], gnome: Main.wm._minimizing.has(a)};
                    self.runs.push(run);
                    if (!fl) return;
                    const step = fl.step.bind(fl);
                    fl.step = u => { const t = GLib.get_monotonic_time(); step(u); self.steps.push(GLib.get_monotonic_time() - t);
                        run.scale.push(a.scale_x); run.tx.push(a.translation_x); run.u = u; };
                    fl.timeline?.connect('completed', () => { run.ms = (GLib.get_monotonic_time() - run.t0) / 1000; });
                }, self);
            }
            return 1;
        },
        stop() { global.window_manager.disconnectObject(self); return 1; },
        settled: () => files().every(w => { const a = w.get_compositor_private();
            return !M().flying(a) && !Main.wm._minimizing.has(a) && !Main.wm._unminimizing.has(a) && !Main.wm._mapping.has(a); }),
        // What is left on a window after its flight, as a word: clean, or what is not
        end(i) {
            const w = files()[i]; const a = w.get_compositor_private(); const bad = [];
            if (a.scale_x !== 1 || a.scale_y !== 1) bad.push('scale');
            if (a.opacity !== 255) bad.push('opacity');
            if (a.pivot_point.x !== 0 || a.pivot_point.y !== 0) bad.push('pivot');
            if (a.get_effect('amethystora-motion-droplet')) bad.push('effect');
            if (Main.wm._minimizing.has(a) || Main.wm._unminimizing.has(a) || Main.wm._mapping.has(a)) bad.push('sets');
            if (M().flying(a)) bad.push('flying');
            if (Main.layoutManager.uiGroup.get_children().some(c => /Motion/.test(c.constructor.name))) bad.push('copy');
            return bad.length ? bad.join(',') : (w.minimized ? 'minimized' : (a.visible ? 'shown' : 'hidden'));
        },
        rect(i) { const f = files()[i].get_frame_rect(); return [f.x, f.y, f.width, f.height].join(','); },
        tr(i) { const a = files()[i].get_compositor_private(); return Math.round(a.translation_x) + ',' + Math.round(a.translation_y); },
        // Drawn where the tiling's strip has it (frame and translation), or, in the classic layout, where its frame is
        placed(i) { const w = files()[i]; const a = w.get_compositor_private(); const f = w.get_frame_rect();
            const info = ${UI}.get('tiling')?.engine?.info?.(w);
            const want = info?.strip && info.vis ? [info.vis.x - f.x, info.vis.y - f.y] : [0, 0];
            return Math.abs(a.translation_x - want[0]) < 1 && Math.abs(a.translation_y - want[1]) < 1; },
        p95() { const s = self.steps.slice().sort((x, y) => x - y); return s.length ? s[Math.floor(s.length * 0.95)] : -1; },
    };
    return self;
})(); 1" >/dev/null

ev "Main.overview.hide(); 1" >/dev/null
user_bus systemd-run --user --collect --unit="vmlab-files1-$$" nautilus --new-window >/dev/null
sleep 4
user_bus systemd-run --user --collect --unit="vmlab-files2-$$" nautilus --new-window >/dev/null
wait_for 60 two_files
trap cleanup EXIT
ev "global.force_animations = true; 1" >/dev/null
sleep 1
same "animations forced on for the suite" "$(ev "imports.gi.St.Settings.get().enable_animations")" true
minimize() { ev "${V}.win($1).minimize(); 1" >/dev/null; }
restore() { ev "${V}.win($1).activate(global.get_current_time()); 1" >/dev/null; }
placed() { [[ "$(ev "${V}.placed($1)")" == true ]]; }
cycle_ends() {
    local label="$1" i="$2" before
    before="$(ev "${V}.rect(${i})")"
    minimize "${i}"
    wait_for 3 settled
    same "${label}: minimized, nothing left on the window" "$(ev "${V}.end(${i})")" minimized
    restore "${i}"
    wait_for 3 settled
    same "${label}: back, nothing left on the window" "$(ev "${V}.end(${i})")" shown
    same "${label}: the same frame as before" "$(ev "${V}.rect(${i})")" "${before}"
    check "${label}: drawn where the layout puts it" wait_for 3 placed "${i}"
}

# --- 7.1 Ends exact ---------------------------------------------------------------------------------------------------
ev "${V}.record()" >/dev/null
cycle_ends "1-2 droplet, tiling" 1
same "1-2 both were droplet flights" "$(ev "${V}.runs.map(r => r.kind).join(' ')")" "minimize unminimize"
# The budget is 1 ms with a GPU. The lab draws in software and runs JavaScript several times slower than a real machine
# (the headless shell on the same host measures a 95th percentile of about 0.45 ms): there it is held to 3 ms, which
# still catches a step that does real work every frame, and the value is printed
BUDGET="$(ev "global.backend.is_rendering_hardware_accelerated() ? 1000 : 3000")"
same "14 a step costs under ${BUDGET} us (95th percentile of at least 20 steps: $(ev "${V}.p95()") us)" \
    "$(ev "${V}.steps.length >= 20 && ${V}.p95() < ${BUDGET}")" true
check "the droplet took 0.6 x 912 ms (+-3 frames): $(ev "${V}.runs[0].ms") ms" test "$(ev "Math.abs(${V}.runs[0].ms - 547) < 50")" == true
ev "${V}.stop()" >/dev/null

in_session "ame desktop layout classic" >/dev/null 2>&1
sleep 1
cycle_ends "7 droplet, classic" 1
same "7 classic: no translation" "$(ev "${V}.tr(1)")" "0,0"
in_session "ame desktop layout tiling" >/dev/null 2>&1
sleep 1

gs set org.gnome.desktop.interface enable-animations false
sleep 1
ev "${V}.record()" >/dev/null
minimize 1
same "3 animations off: no flight is ever made" "$(ev "${MO}.flying(${V}.win(1).get_compositor_private())")" false
wait_for 3 settled
restore 1
wait_for 3 settled
same "3 animations off: ends clean" "$(ev "${V}.end(1)")" shown
same "3 animations off: GNOME's path" "$(ev "${V}.runs.map(r => r.kind).join(' ')")" "gnome gnome"
ev "${V}.stop(); 1" >/dev/null
gs reset org.gnome.desktop.interface enable-animations
sleep 1

check "4 ame desktop minimize shrink" in_session "ame desktop minimize shrink"
same "4 the key says shrink" "$(gs get "${MOTION}" minimize)" "'shrink'"
ev "${V}.record()" >/dev/null
cycle_ends "4 shrink" 1
same "4 shrink: GNOME's own minimize took the window" "$(ev "${V}.runs[0].kind + ' ' + ${V}.runs[0].gnome")" "gnome true"
ev "${V}.stop()" >/dev/null

gs set org.gnome.desktop.a11y.interface reduced-motion reduce
sleep 1
ev "${V}.record()" >/dev/null
cycle_ends "5 reduce motion with shrink" 1
same "5 reduce motion takes over with shrink: two fades" "$(ev "${V}.runs.map(r => r.kind).join(' ')")" "fade-out fade-in"
same "5 a fade never scales or moves the window" \
    "$(ev "${V}.runs.every(r => r.scale.every(s => s === 1) && new Set(r.tx).size <= 1)")" true
check "5 a fade takes 0.6 x 150 ms (+-2 frames)" test "$(ev "${V}.runs.every(r => Math.abs(r.ms - 90) < 40)")" == true
ev "${V}.stop()" >/dev/null
gs reset "${MOTION}" minimize
gs reset org.gnome.desktop.a11y.interface reduced-motion
sleep 1

# --- 7.2 Cut short ----------------------------------------------------------------------------------------------------
since_cut="$(date '+%Y-%m-%d %H:%M:%S')"
ev "${V}.record(); const w = ${V}.win(1); w.minimize();
    imports.gi.GLib.timeout_add(0, Math.round(0.4 * 0.6 * 912), () => { w.activate(global.get_current_time()); return false; }); 1" >/dev/null
sleep 2
wait_for 3 settled
same "8 turned round at 40 %: back, clean" "$(ev "${V}.end(1)")" shown
check "8 the way back was shorter than a whole one" test "$(ev "(${V}.runs[1]?.ms ?? 999) < 547")" == true
ev "${V}.stop(); ${V}.record(); const w = ${V}.win(1); w.minimize(); 1" >/dev/null
wait_for 3 settled
ev "const w = ${V}.win(1); w.activate(global.get_current_time());
    imports.gi.GLib.timeout_add(0, Math.round(0.5 * 0.6 * 912), () => { w.minimize(); return false; }); 1" >/dev/null
sleep 2
wait_for 3 settled
same "9 turned round at 50 % on the way back: minimized, clean" "$(ev "${V}.end(1)")" minimized
ev "${V}.stop(); let n = 0; const w = ${V}.win(1); globalThis.vmlabLast = null;
    const go = () => { const down = n % 2 === 1; if (down) w.minimize(); else w.activate(global.get_current_time());
        globalThis.vmlabLast = down; if (++n < 10) imports.gi.GLib.timeout_add(0, 50 + Math.floor(Math.random() * 350), () => { go(); return false; }); };
    go(); 1" >/dev/null
sleep 6
wait_for 3 settled
want="$(ev "globalThis.vmlabLast ? 'minimized' : 'shown'")"
same "10 ten turns at random moments: the last call wins, clean" "$(ev "${V}.end(1)")" "${want}"
restore 1
wait_for 3 settled
accounting="$(journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --since "${since_cut}" --no-pager -o cat 2>/dev/null |
    grep -E "(minimize|unminimize|map) accounting" | head -n 3)"
if [[ -z ${accounting} ]]; then ok "8-10 Mutter's accounting of effects never complained"; else not_ok "Mutter's effect accounting" "${accounting}"; fi
user_bus systemd-run --user --collect --unit="vmlab-files3-$$" nautilus --new-window >/dev/null
three_files() { [[ "$(ev "global.get_window_actors().filter(a => a.meta_window.get_wm_class() === 'org.gnome.Nautilus').length")" == 3 ]]; }
wait_for 60 three_files
since_close="$(date '+%Y-%m-%d %H:%M:%S')"
ev "const w = ${V}.win(2); w.minimize(); imports.gi.GLib.timeout_add(0, 200, () => { w.delete(global.get_current_time()); return false; }); 1" >/dev/null
sleep 3
same "11 a window closed in flight is gone, and the module holds no flight" \
    "$(ev "global.get_window_actors().filter(a => a.meta_window.get_wm_class() === 'org.gnome.Nautilus').length + ' ' + ${MO}._flights.size")" "2 0"
closed="$(journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --since "${since_close}" --no-pager -o cat 2>/dev/null |
    grep -E "accounting|already disposed|JS ERROR" | head -n 3)"
if [[ -z ${closed} ]]; then ok "11 nothing complained about the closed window"; else not_ok "11 the closed window" "${closed}"; fi

# --- 7.3 Open from the dock -------------------------------------------------------------------------------------------
ev "for (const w of global.display.list_all_windows()) if (w.get_wm_class() === 'org.gnome.Nautilus') w.delete(global.get_current_time()); 1" >/dev/null
sleep 3
ev "globalThis.vmlabOpen = null; const app = imports.gi.Shell.AppSystem.get_default().lookup_app('org.gnome.Nautilus.desktop');
    const dock = ${UI}.get('dock'); const box = dock.iconBoxForApp(app, 0);
    global.window_manager.connectObject('map', (wm, a) => { if (a.meta_window.get_wm_class() !== 'org.gnome.Nautilus') return;
        global.window_manager.disconnectObject(globalThis); const fl = ${MO}.flight(a);
        globalThis.vmlabOpen = {kind: fl?.kind ?? 'gnome', near: null};
        const t0 = imports.gi.GLib.get_monotonic_time();
        const id = global.stage.connect('after-paint', () => { const e = a.get_transformed_extents();
            if (imports.gi.GLib.get_monotonic_time() - t0 > 100000) { global.stage.disconnect(id); return; }
            const cx = e.origin.x + e.size.width / 2, cy = e.origin.y + e.size.height / 2;
            if (globalThis.vmlabOpen.near === null) { globalThis.vmlabOpen.near = Math.hypot(cx - box.x - box.width / 2, cy - box.y - box.height / 2) < box.width;
                const engine = ${UI}.get('tiling').engine, info = engine?.info(a.meta_window);
                globalThis.vmlabOpen.frame = info?.strip?.selected === a.meta_window && !!engine._boxes.get(info.strip)?.visible; } }); }, globalThis);
    dock._sites[0].dash._clicks.appTargetFor(app).launch(); 1" >/dev/null
opened() { [[ "$(ev "globalThis.vmlabOpen?.kind ?? ''")" != "" ]]; }
wait_for 20 opened
same "12 an app opened from the dock grows out of its icon" "$(ev "globalThis.vmlabOpen.kind + ' ' + globalThis.vmlabOpen.near")" "open true"
same "12 the strip's focus box stands no sheet behind it meanwhile" "$(ev "globalThis.vmlabOpen.frame")" false
sleep 2
wait_for 3 settled
same "12 and settles clean" "$(ev "${V}.end(0)")" shown
ev "globalThis.vmlabOpen = null; global.window_manager.connectObject('map', (wm, a) => { if (a.meta_window.get_wm_class() !== 'org.gnome.Nautilus') return;
    global.window_manager.disconnectObject(globalThis); globalThis.vmlabOpen = {kind: ${MO}.flight(a)?.kind ?? 'gnome'}; }, globalThis);
    imports.gi.Shell.AppSystem.get_default().lookup_app('org.gnome.Nautilus.desktop').open_new_window(-1); 1" >/dev/null
wait_for 20 opened
same "13 one opened elsewhere is GNOME's map" "$(ev "globalThis.vmlabOpen.kind")" gnome
wait_for 60 two_files

# --- 7.5 The ring -----------------------------------------------------------------------------------------------------
ev "globalThis.vmlabRing = null; const w = ${V}.win(1); const box = ${UI}.get('dock').iconBoxFor(w);
    global.window_manager.connectObject('minimize', () => { global.window_manager.disconnectObject(globalThis);
        const id = imports.gi.GLib.timeout_add(0, 10, () => { const r = Main.layoutManager.uiGroup.get_children()
            .find(c => c.style_class === 'amethystora-motion-ripple' && c.visible);
            if (r && box) { const e = r.get_transformed_extents();
                globalThis.vmlabRing = Math.round(Math.hypot(e.origin.x + e.size.width / 2 - box.x - box.width / 2,
                    e.origin.y + e.size.height / 2 - box.y - box.height / 2)); return false; }
            return globalThis.vmlabRing === null; }); }, globalThis);
    w.minimize(); 1" >/dev/null
sleep 2
same "16 the ring ripples round the icon, within 1 px of its centre" "$(ev "globalThis.vmlabRing !== null && globalThis.vmlabRing <= 1")" true
same "16 and is gone a second after" "$(ev "Main.layoutManager.uiGroup.get_children().filter(c => c.style_class === 'amethystora-motion-ripple').length")" 0
restore 1
wait_for 3 settled

# --- 17 No icon: the drop goes to the bottom centre of its screen, without a ring ----------------------------------------
ev "globalThis.vmlabNoIcon = null; const dock = ${UI}.get('dock'); dock.iconBoxFor = () => null; const w = ${V}.win(1);
    w.set_icon_geometry(null);
    global.window_manager.connectObject('minimize', (wm, a) => { if (a.meta_window !== w) return;
        global.window_manager.disconnectObject(globalThis); const fl = ${MO}.flight(a); if (!fl) return;
        fl.timeline.pause(); fl.step(35 / 38);
        const f = w.get_frame_rect(), b = w.get_buffer_rect();
        const p = (x, y) => a.apply_relative_transform_to_point(null, new imports.gi.Graphene.Point3D({x, y, z: 0}));
        const p1 = p(f.x - b.x, f.y - b.y), p2 = p(a.width - (b.x + b.width - f.x - f.width), a.height - (b.y + b.height - f.y - f.height));
        const m = Main.layoutManager.monitors[w.get_monitor()]; const sf = imports.gi.St.ThemeContext.get_for_stage(global.stage).scale_factor;
        globalThis.vmlabNoIcon = Math.round(Math.hypot((p1.x + p2.x) / 2 - m.x - m.width / 2, (p1.y + p2.y) / 2 - m.y - m.height + 32 * sf));
        fl.timeline.start(); }, globalThis);
    w.minimize(); 1" >/dev/null
sleep 2
wait_for 3 settled
same "17 no icon: the drop lands at the bottom centre of the screen (within 2 px)" "$(ev "globalThis.vmlabNoIcon !== null && globalThis.vmlabNoIcon <= 2")" true
same "17 no icon: no ring, and the window ends clean" \
    "$(ev "Main.layoutManager.uiGroup.get_children().filter(c => c.style_class === 'amethystora-motion-ripple').length + ' ' + ${V}.end(1)")" "0 minimized"
ev "delete ${UI}.get('dock').iconBoxFor; 1" >/dev/null
restore 1
wait_for 3 settled

# --- 7.6 Workspaces ---------------------------------------------------------------------------------------------------
switch_sample() {
    # Switches to workspace $1, holds the switch at exactly half of its time (after the motion has retimed it), reads
    # the first group there, and lets it go on
    ev "globalThis.vmlabDip = null; const L = imports.gi.Meta.LaterType.BEFORE_REDRAW; const laters = global.compositor.get_laters();
        global.workspace_manager.get_workspace_by_index($1).activate(global.get_current_time());
        laters.add(L, () => { laters.add(L, () => {
            const g = Main.uiGroup.get_children().find(c => c.constructor.name.includes('MonitorGroup'));
            const t = g?.get_transition('progress');
            if (!g || !t) { globalThis.vmlabDip = {dur: -1, scales: [], op: [], bg: false, pinned: false}; return false; }
            const dur = t.get_duration(); t.pause(); t.advance(Math.round(dur / 2)); t.emit('new-frame', t.get_elapsed_time());
            const x0 = g._container.x;
            globalThis.vmlabDip = {dur,
                scales: g._workspaceGroups.flatMap(w => w._windowRecords.map(r => +r.clone.scale_x.toFixed(4))),
                bg: g._workspaceGroups.every(w => Math.abs((w._background?.translation_x ?? 0) + (2 / 3) * (w.x + x0)) < 1.5),
                pinned: g._workspaceGroups.every(w => Math.abs(w.x + x0 + w.translation_x) < 1),
                op: g._workspaceGroups.map(w => w.opacity)};
            t.start(); return false; }); return false; }); 1" >/dev/null
    sleep 2
}
switch_sample 1
same "18 depth: the switch took 0.6 x 560 ms" "$(ev "globalThis.vmlabDip?.dur")" 336
same "18 depth: the windows at 94 % at the middle (scales: $(ev "globalThis.vmlabDip.scales.join(' ')"))" \
    "$(ev "globalThis.vmlabDip.scales.length > 0 && globalThis.vmlabDip.scales.every(s => Math.abs(s - 0.94) <= 0.01)")" true
same "18 depth: the wallpaper a third as far" "$(ev "globalThis.vmlabDip.bg")" true
same "18 after it: on workspace 2, no group left, GNOME's switch done" \
    "$(ev "global.workspace_manager.get_active_workspace_index() + ' ' + Main.uiGroup.get_children().filter(c => c.constructor.name.includes('MonitorGroup')).length + ' ' + (Main.wm._workspaceAnimation._switchData === null)")" "1 0 true"
check "19 ame desktop workspace-switch slide" in_session "ame desktop workspace-switch slide"
switch_sample 0
same "19 slide is GNOME's: 0.6 x 250 ms, nothing scaled" \
    "$(ev "globalThis.vmlabDip.dur + ' ' + globalThis.vmlabDip.scales.every(s => s === 1)")" "150 true"
gs reset "${MOTION}" workspace-switch
gs set org.gnome.desktop.a11y.interface reduced-motion reduce
sleep 1
switch_sample 1
same "20 reduce motion: half way, one workspace shows whole and the other part way (opacities: $(ev "globalThis.vmlabDip.op.join(' ')"))" \
    "$(ev "globalThis.vmlabDip.op.includes(255) && globalThis.vmlabDip.op.some(o => o > 0 && o < 255)")" true
same "20 reduce motion: a crossfade of 0.6 x 150 ms, nothing travels" \
    "$(ev "globalThis.vmlabDip.dur + ' ' + globalThis.vmlabDip.pinned")" "90 true"
gs reset org.gnome.desktop.a11y.interface reduced-motion
sleep 1
ev "global.workspace_manager.get_workspace_by_index(0).activate(global.get_current_time()); 1" >/dev/null
sleep 2

# --- 22 Disabled in the middle of a switch: it ends on the right workspace, nothing left -------------------------------
ev "global.workspace_manager.get_workspace_by_index(1).activate(global.get_current_time());
    imports.gi.GLib.timeout_add(0, 168, () => { Main.extensionManager.disableExtension('${UUID}'); return false; });
    imports.gi.GLib.timeout_add(0, 600, () => { Main.extensionManager.enableExtension('${UUID}'); return false; }); 1" >/dev/null
sleep 2
wait_for 30 module_up
no_group() { [[ "$(ev "Main.uiGroup.get_children().filter(c => c.constructor.name.includes('MonitorGroup')).length")" == 0 ]]; }
wait_for 5 no_group
same "22 disabled mid-switch: on workspace 2, no group left, the wallpapers and windows as GNOME left them" \
    "$(ev "global.workspace_manager.get_active_workspace_index() + ' ' + Main.uiGroup.get_children().filter(c => c.constructor.name.includes('MonitorGroup')).length + ' ' + global.get_window_actors().every(a => a.scale_x === 1)")" "1 0 true"
ev "global.workspace_manager.get_workspace_by_index(0).activate(global.get_current_time()); 1" >/dev/null
sleep 2

# --- 7.7 Names and drops ----------------------------------------------------------------------------------------------
LABELS="Main.panel.statusArea['amethystora-workspaces'].box.get_children()"
label() { ev "${LABELS}[$1].labelText + ' ' + ${LABELS}[$1]._name.has_style_class_name('amethystora-workspaces-auto')"; }
same "23 an empty workspace is its number" "$(label 2)" "3 false"
ev "${V}.win(0).change_workspace_by_index(2, false); ${V}.win(1).change_workspace_by_index(2, false); 1" >/dev/null
files_name() { [[ "$(label 2)" == "3 $(ev "imports.gi.Shell.AppSystem.get_default().lookup_app('org.gnome.Nautilus.desktop').get_name()") true" ]]; }
check "23 two Files windows there: named after Files, in the lighter type" wait_for 2 files_name
WS="${UI}.get('workspaces')"
NAMES="new imports.gi.Gio.Settings({schema_id: 'org.gnome.desktop.wm.preferences'}).get_strv('workspace-names')"
ev "global.workspace_manager.get_workspace_by_index(2).activate(global.get_current_time()); 1" >/dev/null
sleep 2
ev "${WS}.openMenu(); 1" >/dev/null
sleep 1
same "24 the menu's field suggests the app's name, and offers no way back yet" \
    "$(ev "${WS}._entry.hint_text + ' ' + ${WS}._unname.visible")" "$(ev "imports.gi.Shell.AppSystem.get_default().lookup_app('org.gnome.Nautilus.desktop').get_name()") false"
ev "${WS}._entry.text = 'Research'; ${WS}._menu.close(); 1" >/dev/null
sleep 1
same "24 renamed through its menu: the name of the account's own wins" "$(label 2)" "3 Research false"
same "24 and GNOME's list holds it" "$(ev "${NAMES}[2]")" Research
ev "${WS}.openMenu(); 1" >/dev/null
sleep 1
same "24 Use the app's name is offered" "$(ev "${WS}._unname.visible")" true
ev "${WS}._unname.activate(imports.gi.Clutter.get_current_event()); 1" >/dev/null
sleep 1
check "24 Use the app's name: named after Files again" wait_for 2 files_name
same "24 and GNOME's list no longer holds a name for it" "$(ev "${NAMES}[2] ?? ''")" ""
ev "global.workspace_manager.get_workspace_by_index(0).activate(global.get_current_time()); 1" >/dev/null
sleep 1
check "25 ame desktop space-names off" in_session "ame desktop space-names off"
sleep 1
same "25 an unnamed workspace is its number again" "$(label 2)" "3 false"
check "25 ame desktop space-names on" in_session "ame desktop space-names on"
ev "${V}.win(0).change_workspace_by_index(0, false); ${V}.win(1).change_workspace_by_index(0, false); 1" >/dev/null
ev "Main.overview.show(); 1" >/dev/null
sleep 2
ev "Main.overview.emit('window-drag-begin', null); ${WS}._dragMotion({targetActor: ${LABELS}[4]}); 1" >/dev/null
sleep 1
same "26 the label under a dragged window lights up and grows" \
    "$(ev "${LABELS}[4].has_style_class_name('amethystora-workspaces-hit') + ' ' + ${LABELS}[4].scale_x.toFixed(2)")" "true 1.12"
same "26 the note says where it goes" "$(ev "${WS}._hint?.text")" "Move to 5"
ev "${LABELS}[4].acceptDrop({metaWindow: ${V}.win(1)}); Main.overview.emit('window-drag-end', null); 1" >/dev/null
same "26 dropped: the window is on 5, the workspace you are on unchanged, +1 on the label, the note gone" \
    "$(ev "${V}.win(1).get_workspace().index() + ' ' + global.workspace_manager.get_active_workspace_index() + ' ' + ${LABELS}[4].badge?.actor.text + ' ' + !!${WS}._hint")" \
    "4 0 +1 false"
sleep 3
same "26 the count is gone 2.5 s later" "$(ev "String(${LABELS}[4].badge)")" null
ev "${V}.win(1).change_workspace_by_index(0, false); Main.overview.hide(); 1" >/dev/null
sleep 2

# --- 7.8 Keys, commands, live changes ---------------------------------------------------------------------------------
for cmd in "minimize droplet" "minimize shrink" "workspace-switch slide" "workspace-switch depth" "space-names off" "space-names on"; do
    check "28 ame desktop ${cmd}" in_session "ame desktop ${cmd}"
done
for cmd in "minimize bounce" "workspace-switch cube" "space-names maybe"; do
    in_session "ame desktop ${cmd}" >/dev/null 2>&1
    same "28 ame desktop ${cmd} is refused with 64" "$?" 64
done
gs reset "${MOTION}" minimize
gs reset "${MOTION}" workspace-switch
gs reset "${WORKSPACES}" auto-names
ev "${V}.record()" >/dev/null
gs set org.gnome.desktop.a11y.interface reduced-motion reduce
sleep 1
minimize 1
wait_for 3 settled
gs reset org.gnome.desktop.a11y.interface reduced-motion
sleep 1
restore 1
wait_for 3 settled
same "29 Reduce motion applies at once, and so does its end" "$(ev "${V}.runs.map(r => r.kind).join(' ')")" "fade-out unminimize"
ev "${V}.stop()" >/dev/null

# --- 32 Disable and enable mid-flight ---------------------------------------------------------------------------------
for round in 1 2 3 4 5 6 7 8 9 10; do
    if [[ $((round % 2)) == 1 ]]; then minimize 1; else restore 1; fi
    sleep 0.2
    ev "Main.extensionManager.disableExtension('${UUID}'); 1" >/dev/null
    sleep 0.3
    ev "Main.extensionManager.enableExtension('${UUID}'); 1" >/dev/null
    sleep 0.5
done
wait_for 3 settled
restore 1
wait_for 3 settled
same "32 ten cycles mid-flight: the window ends clean" "$(ev "${V}.end(1)")" shown
# GNOME 51 turns an extension off once the Eval that asked has returned (its manager awaits a lock): one step each
ev "Main.extensionManager.disableExtension('${UUID}'); 1" >/dev/null
sleep 1
ev "globalThis.vmlabGnome = null;
    global.window_manager.connectObject('minimize', (wm, a) => { global.window_manager.disconnectObject(globalThis);
        globalThis.vmlabGnome = Main.wm._minimizing.has(a); }, globalThis); ${V}.win(1).minimize(); 1" >/dev/null
sleep 1
same "32 disabled, GNOME's own minimize runs again" "$(ev "globalThis.vmlabGnome")" true
ev "Main.extensionManager.enableExtension('${UUID}'); 1" >/dev/null
wait_for 30 module_up
restore 1
wait_for 3 settled

# --- Leave the account as it was --------------------------------------------------------------------------------------
cleanup
ev "for (const w of global.display.list_all_windows()) if (w.get_wm_class() === 'org.gnome.Nautilus') w.delete(global.get_current_time()); 1" >/dev/null

errors="$(journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --since "${since}" --no-pager -o cat 2>/dev/null |
    awk -v uuid="${UUID}" '/JS ERROR/ { block = $0; next } block && /@(resource|file):\/\// { block = block "\n" $0; next }
        block { if (index(block, uuid)) print block; block = "" } /Amethystora UI: .*(failed|threw|motion)/ { print }
        END { if (block && index(block, uuid)) print block }' | head -n 20)"
if [[ -z ${errors} ]]; then ok "nothing of Amethystora UI threw"; else not_ok "Amethystora UI threw" "${errors}"; fi
disposed="$(journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --since "${since}" --no-pager -o cat 2>/dev/null |
    grep -B2 -A6 "already disposed" | grep -m3 "modules/motion")"
if [[ -z ${disposed} ]]; then ok "the motion never touched a window's actor after it was gone"; else not_ok "a disposed actor was touched" "${disposed}"; fi
finish
