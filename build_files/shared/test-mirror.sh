#!/usr/bin/bash
# How do I look and sound? (R7-44): what /usr/libexec/amethystora-mirror says under the meter, that nothing in it can write
# or send what it sees and hears, and that it exits on SIGTERM rather than leave the camera on. Run by 20-tests.sh; HELPER
# names another copy.
set -euo pipefail

HELPER="${HELPER:-/usr/libexec/amethystora-mirror}"

fail() {
    echo "test-mirror: $*"
    exit 1
}

python3 - "${HELPER}" <<'EOF' || fail "its words are not the spec's"
import importlib.machinery, importlib.util, sys
loader = importlib.machinery.SourceFileLoader('mirror', sys.argv[1])
spec = importlib.util.spec_from_loader('mirror', loader)
mirror = importlib.util.module_from_spec(spec)
loader.exec_module(mirror)
assert mirror.advice('Built-in', False, False) == 'Built-in.'
assert 'lower your sound' in mirror.advice('Built-in', False, True)
assert 'lowers your headset' in mirror.advice('Sony', True, True)
assert 'No microphone' in mirror.advice(None, False, False)
assert mirror.fraction(0.0) == 1.0 and mirror.fraction(-200.0) == 0.0 and 0 < mirror.fraction(-30.0) < 1
assert mirror.MOST_S <= 300
# The camera that is shown is one that sees in colour and is real: not an infrared sensor, not a virtual camera
assert mirror.colour_caps(['video/x-raw, format=(string)YUY2, width=(int)640'])
assert mirror.colour_caps(['image/jpeg, width=(int)1280', 'video/x-raw, format=(string)GRAY8, width=(int)340'])
assert mirror.colour_caps(['video/x-raw, format=(string){ NV12, YUY2 }, width=(int)640'])
assert mirror.colour_caps([])
assert not mirror.colour_caps(['video/x-raw, format=(string)GRAY8, width=(int)340'])
assert not mirror.colour_caps(['video/x-raw, format=(string){ GRAY8, GRAY16_LE }, width=(int)340'])
assert mirror.virtual_camera('v4l2 loopback') and mirror.virtual_camera('v4l2loopback') and not mirror.virtual_camera('uvcvideo')
assert not mirror.virtual_camera(None)
EOF

# It ends on SIGTERM, which is how the shell ends it (a lock, a share, the card): the pipelines are put down first, and the
# program leaves. The real signal handler is run, over stand-ins for the windows and the pipelines
python3 - "${HELPER}" <<'EOF' || fail "it does not put the camera down and leave on SIGTERM"
import signal, subprocess, sys
CHILD = r'''
import importlib.machinery, importlib.util, sys
from gi.repository import GLib
loader = importlib.machinery.SourceFileLoader('mirror', sys.argv[1])
mirror = importlib.util.module_from_spec(importlib.util.spec_from_loader('mirror', loader))
loader.exec_module(mirror)
class Pipeline:
    def set_state(self, state):
        print('pipeline', state, flush=True)
class Gst:
    class State:
        NULL = 'NULL'
class App:
    def __init__(self):
        self.loop = GLib.MainLoop()
    def run(self, args):
        print('ready', flush=True)
        self.loop.run()
        return 0
    def quit(self):
        self.loop.quit()
one = mirror.Mirror.__new__(mirror.Mirror)
one.Gtk = one.Gdk = None
one.GLib, one.Gst = GLib, Gst
one.pipelines = [Pipeline(), Pipeline()]
one.app = App()
mirror.end_on_signals(GLib, one)
sys.exit(one.app.run([]))
'''
child = subprocess.Popen([sys.executable, '-c', CHILD, sys.argv[1]], stdout=subprocess.PIPE, text=True)
assert child.stdout.readline().strip() == 'ready'
child.send_signal(signal.SIGTERM)
out = child.stdout.read()
assert child.wait(timeout=5) == 0, 'it did not leave'
assert out.count('pipeline NULL') == 2, out
EOF

# No element that writes or sends, in the pipelines or anywhere in the program; the window is told apart by its app id
for word in filesink multifilesink splitmuxsink tcpserversink udpsink souphttpclientsink rtmpsink shout2send \
    filesrc socket urllib requests "open(" tempfile; do
    grep -qF "${word}" "${HELPER}" && fail "the helper names ${word}"
done
grep -q "^APP_ID = 'org.amethystora.Mirror'$" "${HELPER}" || fail "another app id"
# The three-minute limit is armed before any device is opened, and no core dump holds a frame or the sound
python3 - "${HELPER}" <<'EOF' || fail "the limit is not armed before the camera"
import sys
text = open(sys.argv[1], encoding='utf-8').read()
assert text.index('timeout_add_seconds(MOST_S, self.quit)') < text.index('idle_add(self.begin)')
assert 'timeout_add_seconds(MOST_S' not in text[text.index('def begin'):]
assert 'RLIMIT_CORE, (0, 0)' in text
EOF
echo "test-mirror: ok"
