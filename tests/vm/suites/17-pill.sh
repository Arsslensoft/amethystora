#!/usr/bin/bash
# vmlab-vm: pill
# The live pill, the privacy dots, the recording glow, the battery and the phone's call and link in the pill
# (plans/beyond/spec-pill.md, section 7.1), in the test account's session, asked of GNOME Shell itself through Eval,
# which a lab-only extension of the test account opens (never in the image):
#   - idle, the pill is GNOME's clock; a test media player makes it "now playing", with its card and Pause;
#   - the plate changes width without a relayout per frame, and with reduce motion only fades;
#   - the quick timer from the calendar, over the music; it rings, and it survives the screen being locked;
#   - a recording through GNOME's own recorder: the pill, the screen's dot, the glow and quiet, and Stop;
#   - sharing (Mutter's screen cast), the microphone, the camera and the location in the dots;
#   - a link and a call from a stand-in phone, through the phone link's listeners;
#   - an update staged as suite 89 stages one, never by hand;
#   - the battery's view, live off and on, and ten disables and enables that leave nothing behind.
# The lab draws without a GPU, where GNOME turns animations off by itself: the motion steps force them on
# (global.force_animations) and put the switch back.
# vmlab-paths: @ui/extension.js @ui/*pill* @sf/*themed/shell-pill* @sf/*just/desktop.just
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@iarsslen.github.io
HOME_DIR="/var/home/${TEST_USER}"
PILL=org.gnome.shell.extensions.amethystora-ui.pill
A11Y=org.gnome.desktop.a11y.interface
HERE="$(dirname "$(readlink -f "$0")")"
WORK=/run/vmlab-17
# Eval's answer, decoded once: Eval gives JSON of the value, and gdbus prints that as a GVariant string. A string comes
# back as it is, anything else as compact JSON; a throw prints ERROR and fails
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        python3 -c '
import ast, json, sys
text = sys.stdin.read().strip()
if not text.startswith("("):
    print("ERROR")
    sys.exit(1)
ok, value = ast.literal_eval("(" + ("True" if text.startswith("(true") else "False") + text[text.index(","):])
if not ok:
    print("ERROR: " + value, file=sys.stderr)
    print("ERROR")
    sys.exit(1)
try:
    value = json.loads(value) if value else ""
except ValueError:
    pass
print(value if isinstance(value, str) else json.dumps(value, ensure_ascii=False, separators=(",", ":")))'
}
gs() { in_session "gsettings $*" 2>/dev/null; }
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
P="${UI}.get('pill')"
# One field of the pill's state, as JSON
# One field of the pill's state, as JSON ('"idle"', true, 5)
st() { ev "JSON.stringify(${P}.state()$1)"; }
# The middle of the chip the pill shows, as fractions of the screen, once it has stopped moving into its place: read
# while the plate still slides, or before it is laid out, the point is off the chip (or NaN)
chip_point() {
    local last="" now tries=30
    while ((tries-- > 0)); do
        now="$(ev "(() => { const c = [...${P}.clock.actor.shown].find(a => a._pillAction); if (!c) return ''; const [x, y] = c.get_transformed_position(); const [w, h] = c.get_transformed_size(); return ((x + w / 2) / global.stage.width).toFixed(4) + ',' + ((y + h / 2) / global.stage.height).toFixed(4); })()")"
        [[ "${now}" =~ ^[0-9.]+,[0-9.]+$ && "${now}" == "${last}" ]] && break
        last="${now}"
        sleep 0.2
    done
    echo "${now}"
}
activity_is() { [[ "$(st .activity)" == "\"$1\"" || "$(st .activity)" == "$1" ]]; }
keys() { python3 "${HERE}/keys.py" "$@" >/dev/null 2>&1; }
shell_click() { user_bus python3 "${HERE}/atspi.py" gnome-shell click "$@" >/dev/null 2>&1; }
# The middle of an actor, as keys.py's fractions of the screen
where() {
    ev "(() => { const a = $1; const [x, y] = a.get_transformed_position(); const [w, h] = a.get_transformed_size();
        return ((x + w / 2) / global.stage.width).toFixed(4) + ',' + ((y + h / 2) / global.stage.height).toFixed(4); })()"
}
PILL_ACTOR="Main.panel.statusArea.dateMenu"
DEV="{id: 'vmlabphone', name: 'Sam phone', launchable: true, ready: true, muteRinger: () => { globalThis.vmlabMuted = true; return Promise.resolve(); }}"
listeners() { ev "(() => { let n = 0; for (const l of ${UI}.get('connect')._listeners) { $1; n++; } return n; })()"; }
since="$(date '+%Y-%m-%d %H:%M:%S')"

if ! has_session; then
    bash "$(dirname "$0")/10-account.sh" >/dev/null
fi
if ! wait_for 300 has_session; then
    not_ok "no session for ${TEST_USER}"
    finish
fi
for key in live privacy-dots glow quiet quiet-night digest digest-times; do gs reset "${PILL}" "${key}"; done

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
pill_ready() { [[ "$(ev "typeof ${P}?.state")" == function ]]; }
if ! wait_for 240 has_session || ! wait_for 120 pill_ready; then
    not_ok "the pill module did not start in the session"
    finish
fi
sleep 8
ev "Main.overview.hide(); 'hidden'" >/dev/null
# The account's own units write their logs here
install -d -m 0755 -o "${TEST_USER}" -g "${TEST_USER}" "${WORK}"
cp "${HERE}/mpris.py" "${HERE}/sharecast.py" "${WORK}/"
FORCED="$(ev "global.force_animations")"
# Whatever happens, the suites after this one find GNOME's animations and reduced motion as they were, and nothing
# staged by step 16 is left for the next start
STAGED_HERE=""
cleanup() {
    ev "global.force_animations = ${FORCED:-false}; 'restored'" >/dev/null 2>&1
    gs reset org.gnome.desktop.a11y.interface reduced-motion
    if [[ -n "${STAGED_HERE}" ]]; then
        rpm-ostree cleanup --pending >/dev/null 2>&1
        unshare -m sh -c 'mount -o remount,rw /sysroot && ostree refs --repo=/sysroot/ostree/repo --delete vmlab/pill' >/dev/null 2>&1
        systemctl start uupd.timer 2>/dev/null
    fi
}
trap cleanup EXIT

# --- 1-2: the module, and idle is GNOME's clock ---------------------------------------------------------------------
check "1 the pill module runs" test "$(ev "${UI}.has('pill')")" == true
# Seven of the pill's own, Later's two and What's new's (the notify group), the digest's lists (finish), the meetings
# (time), saving data (restart) and the second zone (bar)
same "1 the schema has its fifteen keys" \
    "$(in_session "gsettings list-keys ${PILL}" | sort | paste -sd ' ')" \
    "digest digest-through digest-times digest-wait glow later later-times live meetings privacy-dots quiet quiet-night saving-data second-zone whats-new"
same "2 idle" "$(st .activity)" '"idle"'
same "2 the date menu's only child is the pill, GNOME's clock box in it and shown" \
    "$(ev "(() => { const d = ${PILL_ACTOR}; const box = d._clockDisplay.get_parent(); return d.get_n_children() + ' ' + d.get_first_child().style_class + ' ' + (box.get_parent() === d.get_first_child()) + ' ' + box.visible; })()")" \
    "1 amethystora-pill true true"
same "2 its text is GNOME's clock" "$(st .text)" "\"$(ev "${PILL_ACTOR}._clockDisplay.text")\""
keys "click:$(where "${PILL_ACTOR}")"
check "2 a click opens the calendar" test "$(ev "${PILL_ACTOR}.menu.isOpen")" == true
keys esc

# --- 3-4a: now playing, its card, and the motion -------------------------------------------------------------------
MPRIS_LOG="/run/user/$(id -u "${TEST_USER}")/vmlab-mpris.log"
rm -f "${MPRIS_LOG}"
user_bus systemd-run --user --collect --unit=vmlab-mpris python3 "${WORK}/mpris.py" >/dev/null
playing() { activity_is media; }
check "3 a player playing makes the pill now playing within 5 s" wait_for 5 playing
same "3 its text" "$(st .text)" '"Night Drive · Lumen"'
keys "click:$(where "${PILL_ACTOR}")"
card_media() { [[ "$(st .card)" == '"media"' ]]; }
check "3 a click opens its card" wait_for 3 card_media
for button in Back Pause Next; do
    check "3 the card has ${button}" user_bus python3 "${HERE}/atspi.py" gnome-shell has "${button}"
done
shell_click Pause
paused_log() { grep -qx PlayPause "${MPRIS_LOG}"; }
check "3 Pause reached the player" wait_for 5 paused_log
is_paused() { [[ "$(st .paused)" == true ]]; }
check "3 the pill stays, paused" wait_for 5 is_paused
same "3 ... with its text" "$(st .text)" '"Night Drive · Lumen"'
keys esc
shell_click Play >/dev/null 2>&1 || in_session "gdbus call --session --dest org.mpris.MediaPlayer2.vmlab --object-path /org/mpris/MediaPlayer2 --method org.mpris.MediaPlayer2.Player.Play" >/dev/null

ev "global.force_animations = true; 'forced'" >/dev/null
gs set "${A11Y}" reduced-motion no-preference
user_bus systemctl --user stop vmlab-mpris >/dev/null 2>&1
idle() { activity_is idle; }
wait_for 5 idle
# Watched from inside the shell every 10 ms (a poll through gdbus would miss a morph of a fifth of a second): when now
# playing appears, when the plate's pieces and the level bars move, and how often the pill is laid out
ev "(() => { const G = imports.gi.GLib; const p = ${P}; const t0 = G.get_monotonic_time(); const r0 = p.clock.relayouts;
    const s = globalThis.vmlabProbe = {mediaAt: -1, morphFrom: -1, morphTo: -1, barsFrom: -1, barsTo: -1, relayouts: -1};
    G.timeout_add(G.PRIORITY_DEFAULT, 10, () => { const ms = Math.round((G.get_monotonic_time() - t0) / 1000); const now = p.state();
        if (now.activity === 'media' && s.mediaAt < 0) s.mediaAt = ms;
        if (now.motion.morphing) { if (s.morphFrom < 0) s.morphFrom = ms; s.morphTo = ms; }
        if (now.motion.bars) { if (s.barsFrom < 0) s.barsFrom = ms; s.barsTo = ms; }
        if (s.mediaAt >= 0 && ms - s.mediaAt > 1500 && s.relayouts < 0) s.relayouts = p.clock.relayouts - r0;
        return ms < 15000; });
    return 'probing'; })()" >/dev/null
user_bus systemd-run --user --collect --unit=vmlab-mpris python3 "${WORK}/mpris.py" >/dev/null
sleep 16
probe="$(ev "JSON.stringify(globalThis.vmlabProbe)")"
pv() { jq -r ".$1" <<<"${probe}"; }
if (($(pv mediaAt) >= 0 && $(pv morphFrom) >= 0 && $(pv morphFrom) - $(pv mediaAt) < 1000)); then
    ok "4 the plate animates when the player starts"
else
    not_ok "4 the plate animates when the player starts" "${probe}"
fi
if (($(pv morphFrom) >= 0 && $(pv morphTo) - $(pv morphFrom) < 1000)); then ok "4 ...and rests within a second"; else
    not_ok "4 ...and rests within a second" "${probe}"; fi
rose="$(pv relayouts)"
# GNOME 51 lays the bar out once more for the same change (seen on the image before rounds 5 and 6 too)
most=2
[[ "$(gnome-shell --version | grep -oE '[0-9]+' | head -n 1)" -ge 51 ]] && most=3
if ((rose >= 0 && rose <= most)); then ok "4 the pill was laid out ${rose} times for the change"; else not_ok "4 the pill was laid out ${rose} times" "at most ${most}"; fi
if (($(pv barsFrom) >= 0)); then ok "4 the level bars move when the track starts"; else not_ok "4 the level bars move when the track starts" "${probe}"; fi
# Four beats of 0.9 s, at GNOME's speed (the image's slow-down factor shortens them), then still
if (($(pv barsFrom) >= 0 && $(pv barsTo) - $(pv barsFrom) < 6000)); then ok "4 ...and stand still after four beats"; else
    not_ok "4 ...and stand still after four beats" "${probe}"; fi
keys "move:$(where "${PILL_ACTOR}")"
bars() { [[ "$(st .motion.bars)" == true ]]; }
check "4 ...and move again while the pointer is on the pill" wait_for 3 bars
keys move:0.5,0.6

check "4a ame desktop comfort reduce-motion on" in_session "ame desktop comfort reduce-motion on"
user_bus systemctl --user stop vmlab-mpris >/dev/null 2>&1
wait_for 5 idle
user_bus systemd-run --user --collect --unit=vmlab-mpris python3 "${WORK}/mpris.py" >/dev/null
# Asked as fast as Eval answers for the first seconds of the change, when the plate would travel
morphing() { [[ "$(st .motion.morphing)" == true ]]; }
quick_poll() { local i; for i in $(seq 20); do "$1" && return 0; sleep 0.05; done; return 1; }
if quick_poll morphing; then not_ok "4a with reduce motion the plate does not travel"; else ok "4a with reduce motion the plate does not travel"; fi
same "4a ...the bars stand still" "$(st .motion.bars)" false
# Measured in the shell: from the change to the new content fully shown, every 10 ms
ev "(() => { const G = imports.gi.GLib; const p = ${P}; globalThis.vmlabShown = -1; const t0 = G.get_monotonic_time(); p.timer.start(30);
    const content = p.timer.pill(); G.timeout_add(G.PRIORITY_DEFAULT, 10, () => {
        const ms = (G.get_monotonic_time() - t0) / 1000;
        if (p.clock.actor.shown === content && content.opacity === 255) { globalThis.vmlabShown = Math.round(ms); return false; }
        if (ms > 2000) { globalThis.vmlabShown = Math.round(ms); return false; } return true; }); return 'measuring'; })()" >/dev/null
sleep 2.5
took="$(ev "globalThis.vmlabShown")"
if [[ "${took}" =~ ^[0-9]+$ ]] && ((took <= 300)); then ok "4a ...and the new content is fully shown in ${took} ms"; else
    not_ok "4a the new content within 300 ms" "${took} ms"; fi
ev "${P}.timer.cancel(); 'cancelled'" >/dev/null
in_session "ame desktop comfort reduce-motion off" >/dev/null
ev "global.force_animations = ${FORCED:-false}; 'restored'" >/dev/null
wait_for 5 playing

# --- 5-7: the timer ------------------------------------------------------------------------------------------------
keys super+v
cal_open() { [[ "$(ev "${PILL_ACTOR}.menu.isOpen")" == true ]]; }
wait_for 3 cal_open || ev "${PILL_ACTOR}.menu.open(); 'opened'" >/dev/null
check "5 the calendar is open" wait_for 3 cal_open
check "5 the calendar has the quick timer" user_bus python3 "${HERE}/atspi.py" gnome-shell has "Start a 5 minute timer"
# A pointer click on the button, as a person makes one
FIVE="(() => { let b = null; const walk = a => { if (!b && a.accessible_name === 'Start a 5 minute timer') b = a; a.get_children().forEach(walk); }; walk(${PILL_ACTOR}.menu.actor); return b; })()"
# The timer is the last section of GNOME's scrolled column, under Getting set on a new account: brought into view
# first, as a person scrolls to it, or the click lands on what the column's end hides it under
ev "(async () => { const b = ${FIVE}; let s = b?.get_parent(); while (s && !(s instanceof imports.gi.St.ScrollView)) s = s.get_parent();
    if (s) (await import('resource:///org/gnome/shell/misc/animationUtils.js')).ensureActorVisibleInScrollView(s, b); })(); 'shown'" >/dev/null
sleep 1
FIVE_AT="$(where "${FIVE}")"
keys "click:${FIVE_AT}"
timer_on() { activity_is timer; }
if wait_for 5 timer_on; then ok "5 the timer starts in the pill"; else not_ok "5 the timer starts in the pill" "clicked at ${FIVE_AT}"; fi
check "5 ...over the music" test "$(st .others | grep -c media)" -ge 1
[[ "$(st .text)" =~ ^\"0[45]:[0-5][0-9]\"$ ]] && ok "5 its text is the time left" || not_ok "5 its text" "$(st .text)"
left1="$(st .timer.left)"
sleep 3
left2="$(st .timer.left)"
if ((left1 - left2 >= 2 && left1 - left2 <= 4)); then ok "5 it counts down"; else not_ok "5 it counts down" "${left1} then ${left2}"; fi
keys "click:$(where "${PILL_ACTOR}")"
for button in "+1 min" Pause Cancel; do
    check "5 the timer's card has ${button}" user_bus python3 "${HERE}/atspi.py" gnome-shell has "${button}"
done
before="$(st .timer.left)"
shell_click "+1 min"
sleep 0.5
added=$(($(st .timer.left) - before))
# The timer runs on while the click is found and lands, a few seconds in the lab
if ((added >= 55 && added <= 62)); then ok "5 +1 min adds a minute"; else not_ok "5 +1 min" "added ${added}"; fi
shell_click Cancel
back_to_media() { activity_is media; }
check "5 Cancel gives the pill back to the music" wait_for 5 back_to_media

ev "${P}.timer.start(60); 'started'" >/dev/null
rang() { [[ "$(ev "Main.messageTray.getSources().some(s => s.notifications.some(n => n.title === 'The timer is done' && n.urgency === 3))")" == true ]]; }
check "6 the timer rings within 65 s: an urgent notification" wait_for 65 rang
done_word() { [[ "$(st .text)" == '"Timer done"' ]]; }
check "6 the pill says Timer done" wait_for 5 done_word
check "6 then the music again" wait_for 10 back_to_media

ev "${P}.timer.start(300); 'started'" >/dev/null
loginctl lock-session "$(loginctl show-user "${TEST_USER}" -p Display --value)"
sleep 10
loginctl unlock-session "$(loginctl show-user "${TEST_USER}" -p Display --value)"
wait_for 30 pill_ready
running() { [[ "$(st .timer.running)" == true ]]; }
check "7 the timer survives the lock" wait_for 10 running
left="$(st .timer.left)"
if ((left >= 270 && left <= 292)); then ok "7 ...with about 290 s left"; else not_ok "7 time left after the lock" "${left}"; fi
ev "${P}.timer.cancel(); 'cancelled'" >/dev/null

# --- 8-9: recording and sharing --------------------------------------------------------------------------------------
ev "global.force_animations = true; 'forced'" >/dev/null
# GNOME's own recorder, as its screenshot UI starts it (the screencast service answers the shell alone): the whole
# screen, recorded
ev "(async () => { const ui = Main.screenshotUI; await ui.open(1); if (!ui._screencastSupported) { globalThis.vmlabRec = 'unsupported'; ui.close(); return; }
    ui._screenButton.checked = true; await ui._onCaptureButtonClicked(); globalThis.vmlabRec = 'started'; })().catch(e => { globalThis.vmlabRec = 'failed: ' + e.message; }); 'asked'" >/dev/null
recording() { activity_is recording; }
sleep 2
if [[ "$(ev "globalThis.vmlabRec ?? 'nothing'")" == unsupported ]]; then
    skip "8 GNOME's recorder is not supported here (no PipeWire or GStreamer in the lab's session)"
elif wait_for 8 recording; then
    ok "8 a recording takes the pill"
    [[ "$(st .text)" =~ ^\"0?0:0[0-9].*Stop\"$ ]] && ok "8 its time and Stop" || not_ok "8 its text" "$(st .text)"
    same "8 the screen's dot" "$(st .dots.screen)" true
    same "8 a glow on every monitor" "$(st .glow)" "$(ev "Main.layoutManager.monitors.length")"
    # GNOME's drag and drop picks with PickMode.ALL, in the bar too: the glow must not stand in the way of a drop
    same "8 a drop reaches what is under the glow" \
        "$(ev "(() => { const glow = Main.uiGroup.get_children().find(a => a.name === 'amethystora-pill-glow'); const m = Main.layoutManager.primaryMonitor; return [[m.x + m.width / 2, m.y + 5], [m.x + m.width / 2, m.y + m.height / 2]].some(([x, y]) => glow.contains(global.stage.get_actor_at_pos(imports.gi.Clutter.PickMode.ALL, x, y))); })()")" false
    same "8 GNOME's recording and sharing buttons hidden" \
        "$(ev "Main.panel.statusArea.screenRecording.container.visible + ' ' + Main.panel.statusArea.screenSharing.container.visible")" "false false"
    same "8 quiet, for the recording" "$(st .quiet.reason)" '"recording"'
    pulse="$(st .motion.pulsing)"
    same "8 the dot and the glow pulse at first" "${pulse}" true
    sleep 8
    same "8 ...and are still after their beats" "$(st .motion.pulsing)" false
    chip="$(chip_point)"
    keys "click:${chip}"
    stopped() { ! activity_is recording && [[ "$(st .glow)" == 0 ]]; }
    check "8 Stop ends it and the glow" wait_for 5 stopped
    same "8 no glow is left in the shell" "$(ev "Main.uiGroup.get_children().some(a => a.name === 'amethystora-pill-glow')")" false
else
    not_ok "8 a recording takes the pill" "$(st .activity) $(ev "globalThis.vmlabRec ?? 'nothing'")"
fi
ev "global.force_animations = ${FORCED:-false}; 'restored'" >/dev/null

user_bus systemd-run --user --collect --unit=vmlab-share --property=StandardOutput=file:"${WORK}/share.log" \
    python3 "${WORK}/sharecast.py" 20 >/dev/null
sleep 3
if grep -q started "${WORK}/share.log" 2>/dev/null; then
    same "9 sharing: the screen's dot" "$(st .dots.screen)" true
    check "9 sharing: a glow" test "$(st .glow)" -ge 1
    same "9 sharing: quiet, for sharing" "$(st .quiet.reason)" '"sharing"'
    [[ "$(st .text)" == '"Quiet while sharing'* || "$(st .others)" == *quiet* ]] && ok "9 the pill says quiet while sharing" ||
        not_ok "9 quiet in the pill" "$(st .text)"
    ev "${P}.privacy._button.menu.open(); 'open'" >/dev/null
    check "9 the list's Screen row says Shared" user_bus python3 "${HERE}/atspi.py" gnome-shell has Shared
    shell_click "Stop sharing the screen"
    share_closed() { grep -q closed "${WORK}/share.log"; }
    check "9 its Stop ends the share" wait_for 5 share_closed
else
    skip "9 Mutter refused the screen cast: $(cat "${WORK}/share.log" 2>/dev/null)"
fi
user_bus systemctl --user stop vmlab-share >/dev/null 2>&1

# --- 10-13: the microphone, the camera, the location, and dots off --------------------------------------------------
in_session "pactl load-module module-null-sink sink_name=vmlab-mic media.class=Audio/Source/Virtual" >/dev/null 2>&1
user_bus systemd-run --user --collect --unit=vmlab-rec pw-record --target vmlab-mic /tmp/vmlab.wav >/dev/null
mic() { [[ "$(st .dots.microphone)" == true && "$(st .dots.visible)" == true ]]; }
if wait_for 5 mic; then
    ok "10 the microphone's dot"
    same "10 GNOME's microphone icon hidden" "$(ev "Main.panel.statusArea.quickSettings._volumeInput.visible")" false
    ev "${P}.privacy._button.menu.open(); 'open'" >/dev/null
    who="$(ev "${P}.privacy._who('microphone')")"
    # pw-record is a link to pw-cat, which is the name it gives PipeWire
    [[ "${who}" == *pw-cat* ]] && ok "10 the list names the recording app" || not_ok "10 the list names the recording app" "${who}"
    ev "${P}.privacy._button.menu.close(); 'closed'" >/dev/null
    check "13 ame desktop pill dots off" in_session "ame desktop pill dots off"
    dots_off() { [[ "$(st .dots.visible)" == false && "$(ev "Main.panel.statusArea.quickSettings._volumeInput.visible")" == true ]]; }
    check "13 the dots go and GNOME's microphone icon is back" wait_for 5 dots_off
    in_session "ame desktop pill dots on" >/dev/null
    check "13 on again hides it" wait_for 5 mic
    user_bus systemctl --user stop vmlab-rec >/dev/null 2>&1
    no_mic() { [[ "$(st .dots.microphone)" == false ]]; }
    check "10 the dot goes when the recording stops" wait_for 10 no_mic
else
    not_ok "10 the microphone's dot" "$(st .dots)"
fi
user_bus systemctl --user stop vmlab-rec >/dev/null 2>&1

if modprobe v4l2loopback devices=1 video_nr=9 card_label=vmlab exclusive_caps=1 2>/dev/null && command -v ffmpeg >/dev/null &&
    command -v gst-launch-1.0 >/dev/null; then
    ffmpeg -nostdin -loglevel quiet -re -f lavfi -i testsrc -f v4l2 /dev/video9 &
    FEED=$!
    camera_node() { in_session "pw-dump" | jq -e '.[] | select(.info.props["media.role"] == "Camera")' >/dev/null; }
    if wait_for 30 camera_node; then
        serial="$(in_session "pw-dump" | jq -r '[.[] | select(.info.props["media.role"] == "Camera")][0].info.props["object.serial"]')"
        user_bus systemd-run --user --collect --unit=vmlab-cam gst-launch-1.0 pipewiresrc target-object="${serial}" ! fakesink >/dev/null
        camera() { [[ "$(st .dots.camera)" == true ]]; }
        check "11 the camera's dot" wait_for 15 camera
        ev "${P}.privacy._button.menu.open(); 'open'" >/dev/null
        check "11 the list names gst-launch-1.0" wait_for 5 user_bus python3 "${HERE}/atspi.py" gnome-shell has gst-launch
        ev "${P}.privacy._button.menu.close(); 'closed'" >/dev/null
        user_bus systemctl --user stop vmlab-cam >/dev/null 2>&1
        no_camera() { [[ "$(st .dots.camera)" == false ]]; }
        check "11 the camera's dot goes" wait_for 10 no_camera
    else
        skip "11 no camera node in PipeWire after 30 s"
    fi
    kill "${FEED}" 2>/dev/null
    modprobe -r v4l2loopback 2>/dev/null
else
    skip "11 no v4l2loopback, ffmpeg or GStreamer here"
fi

if gs set org.gnome.system.location enabled true && in_session "gdbus call --system --dest org.freedesktop.GeoClue2 --object-path /org/freedesktop/GeoClue2/Manager --method org.freedesktop.GeoClue2.Manager.GetClient" >"${WORK}/geoclue" 2>&1; then
    client="$(grep -o "/org/freedesktop/GeoClue2/Client/[0-9]*" "${WORK}/geoclue")"
    in_session "gdbus call --system --dest org.freedesktop.GeoClue2 --object-path ${client} --method org.freedesktop.DBus.Properties.Set org.freedesktop.GeoClue2.Client DesktopId \"<'org.gnome.Maps'>\"; gdbus call --system --dest org.freedesktop.GeoClue2 --object-path ${client} --method org.freedesktop.GeoClue2.Client.Start" >/dev/null 2>&1 &
    shell_click Allow
    location() { [[ "$(st .dots.location)" == true ]]; }
    if wait_for 10 location; then ok "12 the location's dot"; else skip "12 GeoClue did not report a client in use"; fi
else
    skip "12 GeoClue does not answer here"
fi
gs reset org.gnome.system.location enabled

# --- 14-15: a link and a call from a stand-in phone -----------------------------------------------------------------
pgrep -u "${TEST_USER}" -x firefox >/dev/null && pkill -u "${TEST_USER}" -x firefox
listeners "l.url(${DEV}, 'https://recipes.example.org/lemon-tart')" >/dev/null
link() { activity_is link; }
check "14 a link takes the pill" wait_for 3 link
check "14 its address is shown" test "$(st .text | grep -c 'recipes.example.org/lemon-tart')" == 1
sleep 10
if pgrep -u "${TEST_USER}" -x firefox >/dev/null; then not_ok "14 no browser starts by itself"; else ok "14 no browser starts by itself"; fi
body="$(ev "(() => { const a = ${P}.clock.actor; const [x, y] = a.get_transformed_position(); return ((x + 30) / global.stage.width).toFixed(4) + ',' + ((y + a.height / 2) / global.stage.height).toFixed(4); })()")"
keys "click:${body}"
card_link() { [[ "$(st .card)" == '"link"' ]]; }
check "14 a click on its body opens the card" wait_for 3 card_link
# A pointer click on the card's Copy, found by its name in the card
COPY="(() => { let b = null; const walk = a => { if (!b && a.accessible_name === 'Copy') b = a; a.get_children().forEach(walk); }; walk(${P}.clock._card.box); return b; })()"
keys "click:$(where "${COPY}")"
clip="$(ev "new Promise(r => imports.gi.St.Clipboard.get_default().get_text(imports.gi.St.ClipboardType.CLIPBOARD, (c, t) => { globalThis.vmlabClip = t; r(); })); 'asked'" >/dev/null; sleep 1; ev "globalThis.vmlabClip")"
same "14 Copy puts the address on the clipboard" "${clip}" "https://recipes.example.org/lemon-tart"
check "14 ...and the link stays" link
# The card's Not now, by the pointer as Copy: the pill's own Not now chip comes first by name, has no action, and a
# click on it while the card is open only closes the card
NOT_NOW="(() => { let b = null; const walk = a => { if (!b && a.accessible_name === 'Not now') b = a; a.get_children().forEach(walk); }; walk(${P}.clock._card.box); return b; })()"
keys "click:$(where "${NOT_NOW}")"
sleep 1.5
banner_before="$(ev "Main.messageTray._notificationState")"
gone_to_list() { [[ "$(ev "Main.messageTray.getSources().some(s => s.notifications.some(n => n.title === 'Sam phone sent a link' && n.body === 'https://recipes.example.org/lemon-tart'))")" == true ]] && ! link; }
check "14 Not now: gone from the pill, kept in the list" wait_for 3 gone_to_list
same "14 ...without a banner" "${banner_before}" 0
for bad in "'javascript:alert(1)'" "'https://exa mple.org'"; do
    listeners "l.url(${DEV}, ${bad})" >/dev/null
    sleep 0.5
    if link; then not_ok "14 ${bad} is refused"; else ok "14 ${bad} is refused"; fi
done
listeners "l.url(${DEV}, 'https://аррӏе.com/')" >/dev/null
ascii_host() { [[ "$(st .text)" == '"xn--80ak6aa92e.com"' ]]; }
check "14 a name in another alphabet is shown in its ASCII form" wait_for 3 ascii_host
ev "${P}.phone.notNow(${P}.phone._link); 'kept'" >/dev/null
# An override character in the phone's name and in the address: neither reaches what the pill shows or says
listeners "l.url({...${DEV}, name: 'Sam‮ live'}, 'https://example.org/a‮b')" >/dev/null
sleep 0.5
shown_words="$(ev "(() => { const l = ${P}.phone._link; const card = l.card(); const out = [l.text, l.accessible]; const walk = a => { if (a.text) out.push(a.text); a.get_children().forEach(walk); }; walk(card); card.destroy(); return out.join('|'); })()")"
if [[ -n "${shown_words}" && "${shown_words}" != *$'‮'* && "${shown_words}" == *"Sam"* ]]; then
    ok "14 an override character never reaches the pill, its card or its name"
else
    not_ok "14 an override character reached the pill" "${shown_words}"
fi
ev "${P}.phone.notNow(${P}.phone._link); 'kept'" >/dev/null

ev "globalThis.vmlabMuted = false; 'reset'" >/dev/null
# The card opened in 14 is still open: Not now ended the link and the card went on to the next activity (the music).
# While it is open, its menu holds the pointer, and a click on the pill (the card's source) only closes it, as GNOME's
# menus do: closed first, with Escape as a person would
card_closed() { [[ "$(st .card)" == null ]]; }
card_closed || keys esc
wait_for 3 card_closed || ev "${P}.clock._closeCard(); 'closed'" >/dev/null
listeners "l.call(globalThis.vmlabDev = ${DEV}, 'ringing', '+15550100', 'Sam')" >/dev/null
calling() { activity_is call; }
check "15 a call takes the pill, above everything" wait_for 3 calling
same "15 its text" "$(st .text)" '"Sam is calling"'
chip="$(chip_point)"
card_before="$(st .card)"
keys "click:${chip}"
muted() { [[ "$(ev "globalThis.vmlabMuted")" == true ]]; }
if wait_for 3 muted; then ok "15 Mute ringer reaches the phone"; else
    not_ok "15 Mute ringer reaches the phone" "clicked at ${chip}, the card before: ${card_before}" \
        "$(ev "JSON.stringify({chips: [...${P}.clock.actor.shown].filter(a => a._pillAction).map(a => a.text), card: !!${P}.clock._card, top: ${P}.state().activity})")"
fi
listeners "l.call(globalThis.vmlabDev, 'ended', '', '')" >/dev/null
not_calling() { ! activity_is call; }
check "15 the call ends" wait_for 3 not_calling

# --- 16: an update ready, staged as an update is ----------------------------------------------------------------------
user_bus systemctl --user stop vmlab-mpris >/dev/null 2>&1
REPO=/sysroot/ostree/repo
rw() { unshare -m sh -c 'mount -o remount,rw /sysroot && "$@"' sh "$@"; }
ORIGINAL="$(bootc status --format=json | jq -r '.status.booted.ostree.checksum')"
# The update: the booted commit with one file added on top, layered by ostree itself (--tree), so that nothing of the
# repository's own objects is checked out, linked or touched (a hard-link checkout once changed objects that suite 47
# then found changed outside an update)
TREE="${WORK}/vmlab-pill-tree"
systemctl stop uupd.timer 2>/dev/null
rpm-ostree cleanup --pending >/dev/null 2>&1
STAGED_HERE=1
if [[ -f /usr/share/amethystora/sealed ]]; then
    skip "16 a sealed image stages no ostree update"
elif rm -rf "${TREE}" && mkdir -p "${TREE}/usr/share" && echo vmlab >"${TREE}/usr/share/vmlab-pill" &&
    rw ostree commit --repo="${REPO}" --parent="${ORIGINAL}" -b vmlab/pill --bootable \
        --tree=ref="${ORIGINAL}" --tree=dir="${TREE}" --selinux-policy-from-base \
        --add-metadata-string=version=vmlab-pill >/dev/null &&
    timeout 1500 ostree admin deploy --stage vmlab/pill >/dev/null 2>&1; then
    check "16 ostree staged it" test -f /run/ostree/staged-deployment
    update() { activity_is update; }
    check "16 the pill says an update is ready within 10 s" wait_for 10 update
    # The pill says Ready · restart until the soft reboot's helper, asked through pkexec, answers
    quick_word() { [[ "$(st .text)" == '"Ready · quick restart"' ]]; }
    if wait_for 20 quick_word; then ok "16 ...a quick one"; else
        not_ok "16 ...a quick one" "found: $(st .text)" "the helper: $(/usr/libexec/amethystora-soft-reboot status 2>&1)"
    fi
    keys "click:$(where "${PILL_ACTOR}")"
    shell_click "Restart now"
    updates_open() { [[ "$(ev "global.get_window_actors().some(a => a.meta_window.get_wm_class() === 'org.amethystora.Updates')")" == true ]]; }
    check "16 Restart now opens System Updates" wait_for 15 updates_open
    check "16 ...and restarts nothing" test -f /run/ostree/staged-deployment
    ev "global.get_window_actors().filter(a => a.meta_window.get_wm_class() === 'org.amethystora.Updates').forEach(a => a.meta_window.delete(global.get_current_time())); 'closed'" >/dev/null
    rpm-ostree cleanup --pending >/dev/null 2>&1
    no_update() { ! activity_is update; }
    check "16 the activity goes with the staged update" wait_for 10 no_update
else
    not_ok "16 an update could not be staged"
fi
rw ostree refs --repo="${REPO}" --delete vmlab/pill >/dev/null 2>&1
rm -rf "${TREE}"
systemctl start uupd.timer 2>/dev/null
STAGED_HERE=""

# --- 17-19: the battery, live off, ten disables and enables ------------------------------------------------------------
keys "move:$(where "Main.panel.statusArea.quickSettings._system")"
sleep 1
same "17 no battery here: no time-left card" "$(ev "Main.uiGroup.get_children().some(a => a.has_style_class_name?.('amethystora-pill-tip'))")" false
same "17 at 20% discharging: the offer, and the time left" \
    "$(ev "JSON.stringify(${P}.batteryView({present: true, state: 2, percentage: 20, timeToEmpty: 4320, profile: 'balanced'}))")" \
    '{"offer":true,"hover":"1 h 12 min left"}'
same "17 with power saving on: no offer" \
    "$(ev "${P}.batteryView({present: true, state: 2, percentage: 20, timeToEmpty: 4320, profile: 'power-saver'}).offer")" false
charging="$(ev "JSON.stringify(${P}.batteryView({present: true, state: 1, percentage: 20, timeToFull: 4200, profile: 'balanced'}))")"
[[ "${charging}" == *'"offer":false'*'Full in '* ]] && ok "17 charging: no offer, the time until full" || not_ok "17 charging" "${charging}"
keys move:0.5,0.6

user_bus systemd-run --user --collect --unit=vmlab-mpris python3 "${WORK}/mpris.py" >/dev/null
wait_for 5 playing
check "18 ame desktop pill off" in_session "ame desktop pill off"
gnome_clock() { [[ "$(ev "${PILL_ACTOR}.get_first_child().style_class")" == clock-display-box ]]; }
check "18 the date menu holds GNOME's clock again" wait_for 3 gnome_clock
same "18 idle" "$(st .activity)" '"idle"'
same "18 no timer in the calendar" "$(ev "${PILL_ACTOR}._weatherItem.get_parent().get_children().some(a => a.has_style_class_name('amethystora-pill-timer-section'))")" false
in_session "ame desktop pill on" >/dev/null
check "18 on again: now playing" wait_for 5 playing

# Each step waits for the extension's state: GNOME 51 turns it on and off after the call returns (its manager awaits a
# lock), and an enable takes seconds in the lab
CYCLE="(async () => { try { const uuid = '${UUID}'; const E = Main.extensionManager; const MT = await import('resource:///org/gnome/shell/ui/messageTray.js'); const O = await import('resource:///org/gnome/shell/ui/osdWindow.js');
    const count = () => Main.uiGroup.get_n_children();
    const wait = ms => new Promise(r => setTimeout(r, ms));
    const until = async on => { for (let n = 0; n < 300; n++) { const s = E.lookup(uuid).state; if (on ? s === 1 : s === 2 || s === 6) return; await wait(100); } throw new Error('the extension did not turn ' + (on ? 'on' : 'off')); };
    E.disableExtension(uuid); await until(false); await wait(500); const before = count();
    globalThis.vmlabOriginals = {show: O.OsdWindow.prototype.show, banner: MT.MessageTray.prototype._showNotification};
    for (let i = 0; i < 10; i++) { E.enableExtension(uuid); await until(true); await wait(300); E.disableExtension(uuid); await until(false); await wait(300); }
    const after = count(); const gesture = Main.panel.statusArea.dateMenu._clickGesture.enabled;
    const methods = MT.MessageTray.prototype._showNotification === globalThis.vmlabOriginals.banner && O.OsdWindow.prototype.show === globalThis.vmlabOriginals.show;
    E.enableExtension(uuid);
    globalThis.vmlabCycle = (after - before) + ' ' + gesture + ' ' + methods; } catch (e) { globalThis.vmlabCycle = 'failed: ' + e.message; } })(); 'started'"
ev "${CYCLE}" >/dev/null
cycle_done() { [[ -n "$(ev "globalThis.vmlabCycle ?? ''")" ]]; }
wait_for 180 cycle_done
same "19 ten disables and enables: nothing left in the shell, GNOME's click and methods back" "$(ev "globalThis.vmlabCycle")" "0 true true"
wait_for 30 pill_ready

# --- and nothing threw ---------------------------------------------------------------------------------------------------
user_bus systemctl --user stop vmlab-mpris >/dev/null 2>&1
errors="$(journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --since "${since}" --no-pager -o cat 2>/dev/null |
    grep -E "JS ERROR|Amethystora UI: the pill module" | head -n 20)"
if [[ -z ${errors} ]]; then ok "nothing of the pill threw"; else not_ok "the pill threw" "${errors}"; fi
for key in live privacy-dots glow quiet quiet-night digest digest-times; do gs reset "${PILL}" "${key}"; done
ev "global.force_animations = ${FORCED:-false}; 'restored'" >/dev/null
finish
