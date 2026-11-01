#!/usr/bin/bash
# vmlab-vm: r4-privacy
# Round 4's privacy group (plans/beyond/spec-r4-privacy.md, section 8), in the test account's session, asked of GNOME
# Shell itself through Eval, which a lab-only extension of the test account opens (never in the image):
#   - the privacy module's keys and place, and nothing running while nothing is shared;
#   - the share guard: a whole screen shared through Mutter's own screen cast service covers the chosen apps' windows at
#     once (Security itself, a Wayland window and an X11 window known only by their class), the cover opaque until the
#     blur is drawn and above an X11 surface, a screenshot unreadable, a real held click uncovering one window for that
#     share only, card and off, a window shared on its own uncovered once Mutter says so, a virtual monitor and a remote
#     desktop with no stream covered, a Files window in ~/Private, unredirection given back;
#   - the shield: the tile, every line's page, the report for wheel (report.json root:wheel 0640) with a check failing,
#     never the duress checks, the mark and the mark only when needed, off;
#   - network by app: a program and a name from /etc/hosts in the network menu, only while it is open;
#   - copy without hidden data: a photo and a PDF cleaned, a folder, an unknown kind and odd names, the notification;
#   - walk-away lock: a stand-in phone paired and connected, then gone for a minute while nobody touches the machine:
#     the card and the lock; switch off, or the phone link turned off with the phone there: never;
#   - ten disables and enables in the middle of a share with the shield's menu open; Control and the setup file.
# vmlab-paths: @ui/extension.js @ui/*privacy* @ui/modules/meters.js @ui/modules/connect.js @ui/modules/quick.js @sf/*themed/shell-privacy* @sf/usr/libexec/amethystora-clean-copy @sf/usr/libexec/amethystora-security-config @sf/usr/libexec/amethystora-security-watch @sf/usr/libexec/amethystora-control-list @sf/usr/libexec/amethystora-setup* @sf/*just/desktop.just @sf/*just/security.just @sf/*24-files-menu.sh apps/security/* apps/agent-tools/src/never.rs
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HOME_DIR="/var/home/${TEST_USER}"
HERE="$(dirname "$(readlink -f "$0")")"
WORK=/run/vmlab-53
PRIVACY=org.gnome.shell.extensions.amethystora-ui.privacy
REPORT=/var/lib/amethystora/security/report.json
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
PV="${UI}.get('privacy')"
# One field of the privacy module's state, as JSON
st() { ev "JSON.stringify(${PV}.state()$1)"; }

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
# A window known only by its class: GTK without an application (no GTK application id), on Wayland or, with
# GDK_BACKEND=x11 and GTK 3, on X11. Its text fills it.
cat >"${WORK}/classwin.js" <<'EOF'
const [version, klass, title] = ARGV;
imports.gi.versions.Gtk = version;
const {GLib, Gtk} = imports.gi;
GLib.set_prgname(klass);
const text = Array(16).fill('Bank 4471 0098 7721 · door code 2591').join('\n');
const loop = new GLib.MainLoop(null, false);
if (version === '3.0') {
    Gtk.init(null);
    const w = new Gtk.Window({title, default_width: 640, default_height: 420});
    w.set_wmclass(klass, klass);
    w.add(new Gtk.Label({label: text}));
    w.connect('destroy', () => loop.quit());
    w.show_all();
} else {
    Gtk.init();
    const w = new Gtk.Window({title, default_width: 640, default_height: 420});
    w.set_child(new Gtk.Label({label: text}));
    w.connect('close-request', () => { loop.quit(); return false; });
    w.present();
}
loop.run();
EOF
# Mutter's screen cast and remote desktop, as the portal and gnome-remote-desktop ask them: one window, a virtual
# monitor, or a remote desktop session with no stream at all
cat >"${WORK}/cast.py" <<'EOF'
import sys
from gi.repository import Gio, GLib
kind, seconds = sys.argv[1], int(sys.argv[2])
c = Gio.bus_get_sync(Gio.BusType.SESSION, None)
def call(bus, path, iface, method, args, reply):
    return c.call_sync(bus, path, iface, method, args, GLib.VariantType(reply) if reply else None, Gio.DBusCallFlags.NONE, 10000, None)
SC = "org.gnome.Mutter.ScreenCast"
RD = "org.gnome.Mutter.RemoteDesktop"
if kind == "remote":
    (session,) = call(RD, "/org/gnome/Mutter/RemoteDesktop", RD, "CreateSession", None, "(o)").unpack()
    call(RD, session, RD + ".Session", "Start", None, None)
else:
    (session,) = call(SC, "/org/gnome/Mutter/ScreenCast", SC, "CreateSession", GLib.Variant("(a{sv})", ({},)), "(o)").unpack()
    if kind == "virtual":
        call(SC, session, SC + ".Session", "RecordVirtual", GLib.Variant("(a{sv})", ({},)), "(o)")
    else:
        call(SC, session, SC + ".Session", "RecordWindow", GLib.Variant("(a{sv})", ({"window-id": GLib.Variant("t", int(kind))},)), "(o)")
    call(SC, session, SC + ".Session", "Start", None, None)
print("started", flush=True)
loop = GLib.MainLoop()
GLib.timeout_add_seconds(seconds, loop.quit)
loop.run()
EOF
# How much detail a part of a screenshot has: the mean step between neighbouring pixels
cat >"${WORK}/detail.py" <<'EOF'
import sys
import gi
gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf
p = GdkPixbuf.Pixbuf.new_from_file(sys.argv[1])
w, h, n, stride, data = p.get_width(), p.get_height(), p.get_n_channels(), p.get_rowstride(), p.get_pixels()
total = count = 0
for y in range(0, h, 2):
    # Not the band in the middle, where the cover's own words stay sharp
    if abs(y - h / 2) < 80:
        continue
    row = y * stride
    for x in range(0, w - 1, 2):
        a, b = data[row + x * n], data[row + (x + 1) * n]
        total += abs(a - b)
        count += 1
print(round(total / max(count, 1), 2))
EOF
cp "${HERE}/sharecast.py" "${HERE}/phonepeer.py" "${WORK}/"
chown -R "${TEST_USER}:" "${HOME_DIR}/.local/share/gnome-shell" "${WORK}"
enabled="$(user_bus gsettings get org.gnome.shell enabled-extensions)"
[[ ${enabled} == *vmlab-eval@lab* ]] || user_bus gsettings set org.gnome.shell enabled-extensions "${enabled%]*}, 'vmlab-eval@lab']"
for key in $(in_session "gsettings list-keys ${PRIVACY}"); do gs reset "${PRIVACY}" "${key}"; done
loginctl unlock-sessions
systemctl restart gdm
privacy_ready() { [[ "$(ev "typeof ${PV}?.state")" == function ]]; }
if ! wait_for 240 has_session || ! wait_for 120 privacy_ready; then
    not_ok "the privacy module did not start in the session"
    finish
fi
sleep 8
ev "Main.overview.hide(); 'hidden'" >/dev/null
SESSION="$(loginctl show-user "${TEST_USER}" -p Display --value)"
NS=vmphone53
REPORT_BEFORE="$(cat "${REPORT}" 2>/dev/null)"
cleanup() {
    for key in $(in_session "gsettings list-keys ${PRIVACY}" 2>/dev/null); do gs reset "${PRIVACY}" "${key}"; done
    user_bus systemctl --user stop vmlab-share vmlab-cast vmlab-security vmlab-wayland vmlab-x11 vmlab-files vmlab-hold \
        >/dev/null 2>&1
    /usr/libexec/amethystora-security-config set WALK_AWAY off >/dev/null 2>&1
    [[ -n "${PEER_PID:-}" ]] && kill "${PEER_PID}" 2>/dev/null
    in_session "ame desktop phone off" >/dev/null 2>&1
    firewall-cmd --zone=amethystora-home --remove-interface=vmphone0 >/dev/null 2>&1
    ip link del vmphone0 2>/dev/null
    ip netns del "${NS}" 2>/dev/null
    rm -f "${HOME_DIR}/.config/amethystora/connect/devices.json"
    sed -i '/ vmlab-53[.]example$/d' /etc/hosts
    if [[ -n "${REPORT_BEFORE}" ]]; then printf '%s\n' "${REPORT_BEFORE}" >"${REPORT}"; else rm -f "${REPORT}"; fi
    loginctl unlock-sessions
}
trap cleanup EXIT
has_window() { [[ "$(ev "global.display.list_all_windows().some(w => w.get_title() === '$1')")" == true ]]; }
covered() { [[ "$(st ".guard.covers.map(c => c.title).sort().join(',')")" == "\"$1\"" ]]; }
# The frame of a window, as x y w h
frame() { ev "(() => { const r = global.display.list_all_windows().find(w => w.get_title() === '$1').get_frame_rect(); return [r.x, r.y, r.width, r.height].join(' '); })()"; }
# A window brought to the front, as a person would before looking at it or holding a click on it: the tiling strip
# hides the windows it has scrolled away from, and their covers with them
bring() {
    ev "(() => { global.display.list_all_windows().find(w => w.get_title() === '$1').activate(global.get_current_time()); return 'up'; })()" >/dev/null
    shown() { [[ "$(ev "global.display.list_all_windows().find(w => w.get_title() === '$1').get_compositor_private().visible")" == true ]]; }
    wait_for 10 shown "$1"
    sleep 1
}
# A screenshot of a window's frame, and how much detail it has outside the cover's words (which stay sharp), with no
# banner over it (GNOME's own notices come and go during a session)
detail_of() {
    local x y w h
    bring "$1"
    read -r x y w h <<<"$(frame "$1")"
    rm -f "${WORK}/shot.png"
    ev "Main.messageTray.bannerBlocked = true; 'blocked'" >/dev/null
    sleep 1
    user_bus gdbus call --session --dest org.gnome.Shell.Screenshot --object-path /org/gnome/Shell/Screenshot \
        --method org.gnome.Shell.Screenshot.ScreenshotArea "$((x + 20))" "$((y + 60))" "$((w - 40))" "$((h - 120))" false \
        "${WORK}/shot.png" >/dev/null 2>&1
    ev "Main.messageTray.bannerBlocked = false; 'unblocked'" >/dev/null
    python3 "${WORK}/detail.py" "${WORK}/shot.png" 2>/dev/null || echo 0
}
share() { # UNIT ARGS...
    local unit="$1"
    shift
    rm -f "${WORK}/${unit}.log"
    user_bus systemd-run --user --collect --unit="${unit}" --property=StandardOutput=file:"${WORK}/${unit}.log" "$@" >/dev/null
}
started() { grep -q started "${WORK}/$1.log" 2>/dev/null; }

# --- 1-2: the keys, the place, and nothing while nothing is shared ---------------------------------------------------
same "1 the schema's keys" "$(GSETTINGS_BACKEND=memory gsettings list-keys "${PRIVACY}" | sort | paste -sd ' ')" \
    "network-apps share-guard share-guard-apps share-guard-private shield"
same "1 the defaults" "$(for k in share-guard share-guard-private shield network-apps; do GSETTINGS_BACKEND=memory gsettings get "${PRIVACY}" "${k}"; done | paste -sd ' ')" \
    "'blur' true 'tile' false"
order="$(ev "[...${UI}.keys()].join(' ')")"
[[ " ${order} " =~ \ connect\ (.*\ )?privacy\ (.*\ )?focus\ $ ]] && ok "1 privacy runs after connect and before focus, focus last" ||
    not_ok "1 the modules' order" "${order}"
same "2 nothing shared: no cover, no window watched, no Files proxy, unredirection untouched" \
    "$(st ".guard.covering") $(st ".guard.watched") $(st ".guard.files") $(st ".guard.unredirectDisabled")" "false 0 false false"

# --- 3-9: the share guard ---------------------------------------------------------------------------------------------
user_bus systemd-run --user --collect --unit=vmlab-security -- /usr/bin/amethystora-security >/dev/null
user_bus systemd-run --user --collect --unit=vmlab-wayland -- gjs "${WORK}/classwin.js" 4.0 org.keepassxc.KeePassXC "Lab vault" >/dev/null
wait_for 60 has_window "Lab vault" || not_ok "3 the Wayland test window did not open"
security_title() { ev "global.display.list_all_windows().find(w => w.get_wm_class() === 'org.amethystora.Security')?.get_title() ?? ''"; }
security_up() { [[ -n "$(security_title)" ]]; }
wait_for 60 security_up || not_ok "3 Security did not open"
SECURITY_TITLE="$(security_title)"
before="$(detail_of "Lab vault")"
share vmlab-share python3 "${WORK}/sharecast.py" 300
if wait_for 20 started vmlab-share; then
    at_once() { [[ "$(st ".guard.covering")" == true ]]; }
    check "3 a share covers at once, before Mutter has said what it shows" wait_for 2 at_once
    both() { covered "$(printf '%s\n' "Lab vault" "${SECURITY_TITLE}" | sort | paste -sd ',')"; }
    check "3 Security itself and a window known only by its class are covered" wait_for 10 both
    blurred() { [[ "$(st ".guard.covers.every(c => c.blur && !c.opaque)")" == true ]]; }
    check "3 ...blurred, the cover a tint once the blur is drawn" wait_for 10 blurred
    same "4 the pill was told what is covered" "$(st ".guard.announced" | grep -c 'Covered while you share')" 1
    after="$(detail_of "Lab vault")"
    if python3 -c "import sys; sys.exit(0 if float('${after}') * 4 < float('${before}') else 1)"; then
        ok "5 a screenshot of the covered window has a quarter of the detail at most (${before} → ${after})"
    else
        not_ok "5 the covered window is still detailed in a screenshot" "before ${before}, during ${after}"
    fi
    user_bus systemd-run --user --collect --unit=vmlab-x11 --setenv=GDK_BACKEND=x11 -- \
        gjs "${WORK}/classwin.js" 3.0 org.keepassxc.KeePassXC "Lab X11 vault" >/dev/null
    x11_covered() { [[ "$(st ".guard.covers.filter(c => c.title === 'Lab X11 vault').map(c => c.onTop).join()")" == '"true"' ]]; }
    check "3 an X11 window known only by its class, opening during the share, is covered, the cover above its surface" \
        wait_for 60 x11_covered
    # A held click on the vault's cover, as a person's: 800 ms down, then up (outside the overview, whose pictures of
    # the windows take the pointer)
    ev "Main.overview.hide(); 'hidden'" >/dev/null
    sleep 1
    bring "Lab vault"
    ev "(() => { const Clutter = imports.gi.Clutter; const GLib = imports.gi.GLib;
        const c = ${PV}.guard.coverOf('Lab vault'); const [x, y] = c.get_transformed_position(); const [w, h] = c.get_transformed_size();
        const seat = global.stage.context.get_backend().get_default_seat();
        const d = seat.create_virtual_device(Clutter.InputDeviceType.POINTER_DEVICE);
        const t = () => GLib.get_monotonic_time();
        d.notify_absolute_motion(t(), x + w / 2, y + h / 2);
        GLib.timeout_add(GLib.PRIORITY_DEFAULT, 200, () => { d.notify_button(t(), Clutter.BUTTON_PRIMARY, Clutter.ButtonState.PRESSED); return false; });
        GLib.timeout_add(GLib.PRIORITY_DEFAULT, 1000, () => { d.notify_button(t(), Clutter.BUTTON_PRIMARY, Clutter.ButtonState.RELEASED); return false; });
        return 'held'; })()" >/dev/null
    sleep 2
    uncovered() { [[ "$(st ".guard.covers.some(c => c.title === 'Lab vault')")" == false ]]; }
    check "6 holding a click on a cover for 600 ms uncovers that window" uncovered
    same "6 ...and only that one" "$(st ".guard.covers.length")" 2
    gs set "${PRIVACY}" share-guard card
    card() { [[ "$(st ".guard.covers.every(c => c.look === 'card' && !c.blur && c.opaque)")" == true ]]; }
    check "7 ame desktop share-guard card: a solid card, no blur" wait_for 5 card
    gs set "${PRIVACY}" share-guard off
    none() { [[ "$(st ".guard")" == null && "$(ev "global.get_window_actors().every(a => !a.get_children().some(c => c.has_style_class_name?.('amethystora-privacy-cover')))")" == true ]]; }
    check "7 off: every cover gone" wait_for 5 none
    gs set "${PRIVACY}" share-guard blur
    recovered() { [[ "$(st ".guard.covers.length")" -ge 2 ]]; }
    check "7 blur again: covered again" wait_for 10 recovered
    # A Files window showing ~/Private is the Private folder's
    install -d -o "${TEST_USER}" -g "${TEST_USER}" "${HOME_DIR}/Private"
    user_bus systemd-run --user --collect --unit=vmlab-files -- nautilus --new-window "${HOME_DIR}/Private" >/dev/null
    files_covered() { [[ "$(st ".guard.covers.map(c => c.app).join(',')")" == *org.gnome.Nautilus.desktop* ]]; }
    check "9 a Files window in ~/Private is covered" wait_for 60 files_covered
    gs set "${PRIVACY}" share-guard-private false
    files_free() { [[ "$(st ".guard.covers.map(c => c.app).join(',')")" != *org.gnome.Nautilus.desktop* ]]; }
    check "9 ...and not with share-guard private off" wait_for 5 files_free
    gs reset "${PRIVACY}" share-guard-private
    user_bus systemctl --user stop vmlab-files >/dev/null 2>&1
    # Files closed goes into the dock's recent apps, and GNOME's own dash redraws only when asked: both settled first
    sleep 3
    ev "Main.overview.dash._redisplay(); 'drawn'" >/dev/null
    # Ten disables and enables in the middle of the share, the shield's menu open
    ev "Main.panel.statusArea.quickSettings.menu.open(false); ${PV}.shield.toggle.menu.open(false); 'open'" >/dev/null
    # The shell's own actors in its top layer (the extension's own, which some modules make only when first needed, may
    # be fewer after a restart, never more), the top bar's right box and the quick settings' grid
    ours="Main.uiGroup.get_children().filter(c => (c.style_class ?? '').includes('amethystora-')).length"
    counts="(Main.uiGroup.get_n_children() - ${ours}) + ' ' + Main.panel._rightBox.get_n_children() + ' ' + Main.panel.statusArea.quickSettings.menu._grid.get_n_children()"
    ui_before="$(ev "${counts}")"
    names="Main.uiGroup.get_children().map(c => c.constructor.name + ':' + (c.name || '') + ':' + (c.style_class || ''))"
    ev "globalThis.vmlab53 = ${names}; 'kept'" >/dev/null
    ours_before="$(ev "${ours}")"
    for _ in $(seq 10); do
        ev "Main.extensionManager.disableExtension('${UUID}'); 'off'" >/dev/null
        ev "Main.extensionManager.enableExtension('${UUID}'); 'on'" >/dev/null
    done
    sleep 3
    ev "Main.panel.statusArea.quickSettings.menu.close(false); 'closed'" >/dev/null
    ev "Main.overview.dash._redisplay(); 'drawn'" >/dev/null
    ui_after="$(ev "${counts}")"
    if [[ "${ui_after}" == "${ui_before}" ]]; then
        ok "17 ten disables and enables: the shell's own actors as before"
    else
        not_ok "17 ten disables and enables: the shell's own actors as before" "expected: ${ui_before}" "found: ${ui_after}" \
            "$(ev "(() => { const n = l => l.reduce((m, k) => (m[k] = (m[k] || 0) + 1, m), {}); const a = n(globalThis.vmlab53), b = n(${names}); return Object.keys({...a, ...b}).filter(k => a[k] !== b[k]).map(k => k + ' ' + (a[k] || 0) + ' -> ' + (b[k] || 0)).join(', '); })()")"
    fi
    ours_after="$(ev "${ours}")"
    [[ "${ours_after}" -le "${ours_before}" ]] && ok "17 ...and no more of the extension's own (${ours_before}, then ${ours_after})" ||
        not_ok "17 the extension's own actors grew" "${ours_before}, then ${ours_after}"
    leftover="$(ev "global.get_window_actors().map(a => a.get_children().filter(c => c.has_style_class_name?.('amethystora-privacy-cover')).length).reduce((a, b) => a + b, 0)")"
    covers="$(st ".guard.covers.length")"
    if [[ "${covers}" -ge 2 && "${leftover}" == "${covers}" ]]; then
        ok "17 ...one cover per covered window after them (${covers}), none left over"
    else
        not_ok "17 the covers after ten disables and enables" "covers ${covers}, in the windows ${leftover}"
    fi
    same "17 ...the vault covered again: an uncover lasts one share of one module's life" "$(st ".guard.covers.some(c => c.title === 'Lab vault')")" true
    user_bus systemctl --user stop vmlab-share >/dev/null 2>&1
    lifted() { [[ "$(st ".guard.covering") $(st ".guard.unredirectDisabled")" == "false false" ]]; }
    check "8 the share ends: every cover gone, unredirection given back" wait_for 10 lifted
    # A held cover is uncovered for that share only
    share vmlab-share python3 "${WORK}/sharecast.py" 60
    wait_for 20 started vmlab-share
    vault_back() { [[ "$(st ".guard.covers.some(c => c.title === 'Lab vault')")" == true ]]; }
    check "6 the next share covers the vault again" wait_for 10 vault_back
    user_bus systemctl --user stop vmlab-share >/dev/null 2>&1
    wait_for 10 lifted
    # One window shared on its own: covered while Mutter is asked, then not
    id="$(ev "String(global.display.list_all_windows().find(w => w.get_title() === 'Lab vault').get_id())")"
    share vmlab-cast python3 "${WORK}/cast.py" "${id}" 30
    if wait_for 20 started vmlab-cast; then
        window_only() { [[ "$(st ".guard.sharing") $(st ".guard.covering") $(st ".guard.unredirectDisabled")" == "true false false" ]]; }
        check "8 a window shared on its own: nothing covered once Mutter has said so" wait_for 5 window_only
    else
        not_ok "8 the window share did not start" "$(cat "${WORK}/vmlab-cast.log" 2>&1)"
    fi
    user_bus systemctl --user stop vmlab-cast >/dev/null 2>&1
    wait_for 10 lifted
    # A virtual monitor, and a remote desktop with no stream: both a screen
    for kind in virtual remote; do
        share vmlab-cast python3 "${WORK}/cast.py" "${kind}" 20
        if wait_for 20 started vmlab-cast; then
            sleep 3
            same "8 a ${kind} share counts as a screen: covered" "$(st ".guard.covering")" true
            [[ "${kind}" == remote ]] && same "6 a remote desktop controls this computer: holding uncovers nothing" \
                "$(st ".guard.controlled")" true
        else
            skip "8 a ${kind} share could not start here: $(head -c 200 "${WORK}/vmlab-cast.log" 2>&1)"
        fi
        user_bus systemctl --user stop vmlab-cast >/dev/null 2>&1
        wait_for 10 lifted
    done
else
    not_ok "3 the screen share did not start" "$(cat "${WORK}/vmlab-share.log" 2>&1)"
fi
user_bus systemctl --user stop vmlab-security vmlab-wayland vmlab-x11 >/dev/null 2>&1

# --- 10-12: the shield ---------------------------------------------------------------------------------------------
same "10 the shield is a tile in quick settings" "$(ev "${UI}.get('quick').tileIds().includes('shield')")" true
systemctl start amethystora-security-watch.service
same "11 report.json is root:wheel 0640" "$(stat -c '%U:%G %a' "${REPORT}" 2>&1)" "root:wheel 640"
# A check failing, and a duress check failing beside it, written as the watcher writes the file
jq -n '[{id: "firewall", state: "ok", text: "The firewall is on"}, {id: "backup-runs", state: "check", text: "Backups: none for 9 days"},
    {id: "duress-finger", state: "check", text: "The duress finger would not run its action"}]' >"${REPORT}.new"
chgrp wheel "${REPORT}.new" && chmod 0640 "${REPORT}.new" && mv -f "${REPORT}.new" "${REPORT}"
ev "Main.panel.statusArea.quickSettings.menu.open(false); 'open'" >/dev/null
refreshed() { [[ "$(st ".shield.refreshed")" -ge 1 && "$(st ".shield.state")" != '"none"' ]]; }
wait_for 10 refreshed
if id -nG "${TEST_USER}" | grep -qw wheel; then
    same "11 one thing needs a look, the backups: the duress check neither said nor counted" "$(st ".shield.header")" \
        '["One thing needs a look","Backups: none for 9 days"]'
    same "11 ...the tile amber" "$(ev "${PV}.shield.toggle.has_style_class_name('amethystora-privacy-tile-look')")" true
else
    same "11 another account's header" "$(st ".shield.header")" '["Security","Open Security to see the report"]'
fi
[[ "$(st ".shield")" != *uress* ]] && ok "11 nothing of the shield names the duress feature" || not_ok "11 the shield names it" "$(st ".shield")"
same "10 the menu's six lines" "$(st ".shield.rows.map(r => r.name).join(' ')")" '"network dns usb private camera microphone"'
same "10 it is never switched on" "$(st ".shield.checked")" false
for pair in header: network:settings/network dns: usb: camera:settings/devices microphone:settings/devices; do
    ev "${PV}.shield.openForTest('${pair%%:*}'); 'opened'" >/dev/null
    same "10 the ${pair%%:*} line opens Security on '${pair#*:}'" "$(st ".shield.opened")" "\"${pair#*:}\""
done
security_open() { pgrep -u "${TEST_USER}" -f amethystora-security >/dev/null; }
check "10 ...and Security opens" wait_for 20 security_open
user_bus pkill -f /usr/lib/amethystora-security >/dev/null 2>&1
ev "Main.panel.statusArea.quickSettings.menu.close(false); 'closed'" >/dev/null
in_session "ame desktop shield attention" >/dev/null
marked() { [[ "$(st ".shield.mark.visible")" == true && "$(st ".shield.tile")" == false ]]; }
check "12 attention, a check failing: the mark in the bar" wait_for 5 marked
jq -n '[{id: "firewall", state: "ok", text: "The firewall is on"}]' >"${REPORT}.new"
chgrp wheel "${REPORT}.new" && chmod 0640 "${REPORT}.new" && mv -f "${REPORT}.new" "${REPORT}"
unmarked() { [[ "$(st ".shield.mark.visible")" == false ]]; }
if id -nG "${TEST_USER}" | grep -qw wheel; then
    check "12 the report fine again: the mark goes by itself" wait_for 10 unmarked
fi
in_session "ame desktop shield mark" >/dev/null
mark_shown() { [[ "$(st ".shield.mark.visible")" == true ]]; }
check "12 mark: always there" wait_for 5 mark_shown
in_session "ame desktop shield off" >/dev/null
gone() { [[ "$(st ".shield")" == null ]]; }
check "12 off: no shield" wait_for 5 gone
in_session "ame desktop shield tile" >/dev/null

# --- 13: network by app --------------------------------------------------------------------------------------------
ADDR="$(ip -4 -o route get 1.1.1.1 2>/dev/null | sed -n 's/.* src \([0-9.]*\).*/\1/p')"
if [[ -n "${ADDR}" ]]; then
    printf '%s vmlab-53.example\n' "${ADDR}" >>/etc/hosts
    python3 -m http.server --bind "${ADDR}" 18053 >/dev/null 2>&1 &
    SERVER=$!
    # The program connects once the server listens: a refused connection leaves nothing to list
    server_up() { [[ -n "$(ss -Htln "sport = :18053")" ]]; }
    wait_for 20 server_up
    user_bus systemd-run --user --collect --unit=vmlab-hold -- python3 -c "import socket, time; s = socket.create_connection(('${ADDR}', 18053)); time.sleep(120)" >/dev/null
    in_session "ame desktop network-apps on" >/dev/null
    ev "Main.panel.statusArea['amethystora-meters-network'].menu.open(false); 'open'" >/dev/null
    listed() { [[ "$(st ".connections.shown.map(a => a.name + ' ' + a.places.join('|')).join(',')")" == *python3*"vmlab-53.example on this network"* ]]; }
    if wait_for 15 listed; then
        ok "13 the network menu lists the program, its place named from /etc/hosts, on this network"
    else
        not_ok "13 the network menu lists the program, its place named from /etc/hosts, on this network" \
            "$(st ".connections")" "$(ss -Htn "dport = :18053")"
    fi
    ev "Main.panel.statusArea['amethystora-meters-network'].menu.close(false); 'closed'" >/dev/null
    same "13 closed: nothing is read" "$(st ".connections.reading")" false
    in_session "ame desktop network-apps off" >/dev/null
    same "13 off: no section" "$(st ".connections")" null
    user_bus systemctl --user stop vmlab-hold >/dev/null 2>&1
    kill "${SERVER}" 2>/dev/null
else
    skip "13 the machine has no address of its own"
fi

# --- 14: copy without hidden data ------------------------------------------------------------------------------------
PHOTOS="${HOME_DIR}/vmlab-53"
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${PHOTOS}" "${PHOTOS}/folder"
as_user "cd '${PHOTOS}' && magick -size 64x64 xc:red photo.jpg && exiftool -q -overwrite_original -GPSLatitude=48.85 \
    -GPSLatitudeRef=N -GPSLongitude=2.35 -GPSLongitudeRef=E -Make=Canon -Model=R5 -Artist=Jane photo.jpg &&
    magick -size 300x100 -pointsize 20 label:Hello doc.pdf && exiftool -q -overwrite_original -Author=Jane doc.pdf &&
    echo hi >notes.xyz && cp photo.jpg ./-dash.jpg && cp photo.jpg \"\$(printf 'new\\nline.jpg')\"" >/dev/null 2>&1
before="$(sha256sum <"${PHOTOS}/photo.jpg")"
files_said() { ev "JSON.stringify(Main.messageTray.getSources().filter(s => s.title === 'Files').flatMap(s => s.notifications.map(n => [n.title, n.body])))"; }
clean() { as_user "cd '${PHOTOS}' && /usr/libexec/amethystora-clean-copy -- $*" >/dev/null 2>&1; }
clean photo.jpg
if [[ -f "${PHOTOS}/photo (clean).jpg" ]]; then
    same "14 the photo's copy has no place, camera or author" \
        "$(exiftool -s -s -s -GPSLatitude -Make -Model -Artist "${PHOTOS}/photo (clean).jpg" | paste -sd ' ')" ""
    same "14 the original is unchanged" "$(sha256sum <"${PHOTOS}/photo.jpg")" "${before}"
    said() { [[ "$(files_said)" == *'"photo (clean).jpg","Removed: place, camera'* ]]; }
    check "14 the notification names the copy and what was taken out" wait_for 5 said
else
    not_ok "14 no clean copy of the photo" "$(ls -la "${PHOTOS}")"
fi
clean doc.pdf folder notes.xyz
# poppler's pdfinfo: ExifTool reads no compressed stream without Compress::Zlib, which the image does not carry
same "14 the PDF's copy has no author" "$(pdfinfo "${PHOTOS}/doc (clean).pdf" 2>&1 | grep -E '^Author:|Error')" ""
mixed() { [[ "$(files_said)" == *'2 files were left as they were: not a kind it can clean.'* ]]; }
check "14 a folder and a .xyz are left as they were, and said" wait_for 5 mixed
same "14 ...no copy of either" "$(ls "${PHOTOS}" | grep -c 'clean).xyz\|folder (clean)')" 0
clean ./-dash.jpg "\"\$(printf 'new\\nline.jpg')\""
same "14 a name starting with a dash and one with a newline" \
    "$(test -f "${PHOTOS}/-dash (clean).jpg" && test -f "${PHOTOS}/new"$'\n'"line (clean).jpg" && echo both)" both
same "14 no work folder is left beside them" "$(find "${PHOTOS}" -maxdepth 1 -name '.clean-copy.*' | wc -l)" 0
same "14 Files' scripts menu has the entry" \
    "$(test -x "${HOME_DIR}/.local/share/nautilus/scripts/Copy without hidden data" && echo yes)" yes
rm -rf "${PHOTOS}"

# --- 15: walk-away lock ----------------------------------------------------------------------------------------------
same "15 off as the image ships, in Security's logins" \
    "$(/usr/libexec/amethystora-security-config WALK_AWAY) $(/usr/libexec/amethystora-security-config --list --json | jq -r '.settings[] | select(.key == "WALK_AWAY") | .group')" \
    "off logins"
PEER="${WORK}/phone"
HOST_IP=10.77.53.1
ip netns del "${NS}" 2>/dev/null
ip link del vmphone0 2>/dev/null
ip netns add "${NS}"
ip link add vmphone0 type veth peer name vmphone1
ip link set vmphone1 netns "${NS}"
nmcli device set vmphone0 managed no 2>/dev/null
ip addr add "${HOST_IP}/24" dev vmphone0
ip link set vmphone0 up
ip -n "${NS}" addr add 10.77.53.2/24 dev vmphone1
ip -n "${NS}" link set vmphone1 up
ip -n "${NS}" link set lo up
firewall-cmd --zone=amethystora-home --add-interface=vmphone0 >/dev/null
CONNECT_DIR="${HOME_DIR}/.config/amethystora/connect"
in_session "ame desktop phone on" >/dev/null 2>&1
wait_for 30 test -s "${CONNECT_DIR}/certificate.pem" || not_ok "15 the phone link made no certificate"
in_session "ame desktop phone off" >/dev/null 2>&1
python3 "${WORK}/phonepeer.py" init "${PEER}" >/dev/null
PHONE_ID="$(cat "${PEER}/id")"
PIN="$(python3 "${WORK}/phonepeer.py" pin "${PEER}")"
jq -n --arg id "${PHONE_ID}" --arg pin "${PIN}" '{version: 1, devices: {($id): {name: "Lab phone", type: "phone", protocol: 8,
    incoming: ["kdeconnect.ping"], outgoing: ["kdeconnect.battery"], pin: $pin, address: null, port: null, paired_at: 1}}}' \
    >"${CONNECT_DIR}/devices.json"
chown "${TEST_USER}:" "${CONNECT_DIR}/devices.json"
chmod 0600 "${CONNECT_DIR}/devices.json"
link_on() {
    in_session "ame desktop phone on" >/dev/null 2>&1
    listening() { ss -Hltn "sport = :1716" | grep -q .; }
    wait_for 30 listening
}
phone_up() {
    : >"${PEER}/commands"
    ip netns exec "${NS}" python3 "${WORK}/phonepeer.py" run "${PEER}" "${HOST_IP}" >"${WORK}/peer.out" 2>&1 &
    PEER_PID=$!
}
phone_leaves() {
    printf 'quit\n' >>"${PEER}/commands"
    wait_for 30 ready_is 0
}
ready_is() { [[ "$(st ".away.ready")" == "$1" ]]; }
locked_is() { [[ "$(loginctl show-session "${SESSION}" -p LockedHint --value)" == "$1" ]]; }
link_on
phone_up
if wait_for 30 ready_is 1; then
    ok "15 the stand-in phone is paired and connected, as the shell sees it"
    phone_leaves
    sleep 70
    check "15 with the switch off, the phone leaving locks nothing" locked_is no
    # What ame security walk-away on runs, as root (its sudo would ask in a terminal)
    /usr/libexec/amethystora-security-config set WALK_AWAY on >/dev/null
    same "15 the switch on" "$(/usr/libexec/amethystora-security-config WALK_AWAY)" on
    # The phone link turned off with the phone there is not the phone leaving
    phone_up
    wait_for 30 ready_is 1
    in_session "ame desktop phone off" >/dev/null 2>&1
    wait_for 30 ready_is 0
    sleep 130
    check "15 the phone link turned off with the phone there never locks" locked_is no
    link_on
    phone_up
    wait_for 30 ready_is 1
    cards="$(st ".away.cardsShown")"
    phone_leaves
    waiting() { [[ "$(st ".away.waiting")" == true ]]; }
    check "15 the phone left: a minute's wait starts" wait_for 10 waiting
    check "15 a minute later, nobody at the machine: the screen locks" wait_for 90 locked_is yes
    # Unlocked once the shell has finished locking: GNOME says locked when its curtain starts down, and an unlock
    # before it is down is undone a second later (the extension, off on the lock screen, then stays off)
    lock_down() { [[ "$(ev "Main.sessionMode.currentMode")" == unlock-dialog ]]; }
    wait_for 20 lock_down
    loginctl unlock-sessions
    wait_for 30 locked_is no
    wait_for 60 privacy_ready
    [[ "$(st ".away.cardsShown")" -gt "${cards:-0}" ]] && ok "15 the card showed before the lock" ||
        not_ok "15 no card before the lock" "$(st ".away")"
    sleep 70
    check "15 after unlocking it waits for the phone to come back and leave again" locked_is no
else
    not_ok "15 the stand-in phone did not connect" "$(cat "${PEER}/log" "${WORK}/peer.out" 2>&1 | tail -n 20)"
fi
/usr/libexec/amethystora-security-config set WALK_AWAY off >/dev/null 2>&1

# --- 19: Control and the setup file --------------------------------------------------------------------------------
entries="$(user_bus /usr/libexec/amethystora-control-list --json 2>/dev/null |
    jq -r '[.items[] | select(.id | IN("share-guard", "share-guard-apps", "share-guard-private", "shield", "network-apps")) | .id] | sort | join(" ")')"
same "19 Control lists the five" "${entries}" "network-apps share-guard share-guard-apps share-guard-private shield"
in_session "ame desktop shield mark" >/dev/null
same "19 ame setup save carries the shield" "$(in_session "ame setup save -" 2>/dev/null | jq -c .settings.privacy)" '{"shield":"mark"}'
in_session "ame desktop shield tile" >/dev/null
printf '%s\n' '{"version": 1, "settings": {"privacy": {"shield": "loud"}}}' >"${WORK}/bad.json"
chown "${TEST_USER}:" "${WORK}/bad.json"
if in_session "ame setup diff ${WORK}/bad.json" >/dev/null 2>&1; then
    not_ok "19 the setup file took a shield it does not know"
else
    ok "19 the setup file refuses a shield it does not know"
fi

check "no JS ERROR from the privacy module" test -z "$(journalctl -b --since "-40 min" --no-pager -o cat 2>/dev/null |
    grep -E 'JS ERROR|Amethystora UI: (the share guard|the shield|network by app|walk-away lock)' | grep -iv 'card after an error' | grep -i 'privacy\|share guard\|shield\|network by app\|walk-away')"
finish
