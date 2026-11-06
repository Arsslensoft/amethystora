#!/usr/bin/bash
# The hostile plugin of tests/vm/suites/72-plugins.sh: it tries what a sandbox must stop, one action for each kind, and
# prints each attempt as a row of a card: "blocked" when it failed as it had to, "LEAK" when it worked. The suite sets
# @HOME@ and @UID@ to the test account's before it packs the plugin. It is never installed on a real machine.
set -u
HOME_REAL=@HOME@
UID_REAL=@UID@
rows=()
# probe LABEL COMMAND...: the command, for five seconds at most; success is a leak
probe() {
    local label="$1" value=blocked
    shift
    timeout 5 "$@" >/dev/null 2>&1 && value=LEAK
    rows+=("{\"type\":\"row\",\"label\":\"${label}\",\"value\":\"${value}\"}")
}
# syscall NUMBER ARGS...: whether the call worked (python's ctypes; a refusal is -1)
syscall() {
    python3 -c 'import ctypes, sys
libc = ctypes.CDLL(None, use_errno=True)
args = [int(a, 0) for a in sys.argv[1:]]
sys.exit(0 if libc.syscall(*args) >= 0 else 1)' "$@"
}
card() {
    local joined
    joined="$(
        IFS=,
        echo "${rows[*]}"
    )"
    printf '{"text":"%s","card":[%s]}\n' "$1" "${joined}"
}

case "${PLUGIN_ACTION:-}" in
    fs)
        probe read-ssh cat "${HOME_REAL}/.ssh/id_test"
        probe list-home ls "${HOME_REAL}"
        probe write-home touch "${HOME_REAL}/leaked"
        probe write-home-by-shell sh -c "echo x > ${HOME_REAL}/leaked2"
        probe read-shadow cat /etc/shadow
        probe write-usr touch /usr/leaked
        probe write-etc touch /etc/leaked
        probe write-own-folder touch /plugin/leaked
        probe read-the-store ls /var/lib/amethystora/plugins/accounts
        probe read-another-home ls /var/home
        probe setuid sudo -n true
        probe pkexec pkexec true
        card fs
        ;;
    bus)
        probe session-bus test -S "/run/user/${UID_REAL}/bus"
        probe session-bus-list busctl --user list
        probe system-bus test -S /run/dbus/system_bus_socket
        probe runtime-dir ls "/run/user/${UID_REAL}"
        probe x11 test -e /tmp/.X11-unix/X0
        probe wayland test -S "/run/user/${UID_REAL}/wayland-0"
        probe gpu ls /dev/dri/card0
        probe passwd-has-root grep -q "^root:" /etc/passwd
        card bus
        ;;
    net)
        probe raw-tcp bash -c 'exec 3<>/dev/tcp/1.1.1.1/443'
        probe localhost-service bash -c 'exec 3<>/dev/tcp/127.0.0.1/8099'
        probe udp-dns python3 -c 'import socket; s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM); s.settimeout(3); s.sendto(b"\x00" * 12, ("8.8.8.8", 53)); s.recvfrom(512)'
        probe name-lookup getent hosts example.org
        probe raw-socket python3 -c 'import socket; socket.socket(socket.AF_INET, socket.SOCK_RAW, 1)'
        probe netlink ip addr add 10.9.9.9/24 dev lo
        card net
        ;;
    proc)
        probe ptrace syscall 101 16 1 0 0
        probe mount syscall 165 0 0 0 0 0
        probe io-uring syscall 425 1 0
        probe bpf syscall 321 0 0 0
        probe keyctl syscall 250 0 0 0 0 0
        probe new-user-namespace unshare -U true
        probe new-user-namespace-by-syscall syscall 272 0x10000000
        probe tiocsti python3 -c 'import fcntl, termios; fcntl.ioctl(0, termios.TIOCSTI, b"x")'
        probe other-process pgrep -x gnome-shell
        # Signals to the namespace's init from inside it are dropped and kill says nothing: only an init that went counts
        probe kill-pid-1 sh -c 'kill -9 1 2>/dev/null; sleep 0.5; ! kill -0 1'
        card proc
        ;;
    bomb)
        # A fork bomb is held to 64 tasks. Counted in Python: bash itself exits (254) when a fork finally fails, which is the
        # limit holding, not a result
        value=LEAK
        python3 -c 'import os, sys, time
forks = 0
for _ in range(400):
    try:
        pid = os.fork()
    except OSError:
        break
    if pid == 0:
        time.sleep(30)
        os._exit(0)
    forks += 1
sys.exit(0 if forks <= 64 else 1)' && value=blocked
        rows+=("{\"type\":\"row\",\"label\":\"fork-bomb\",\"value\":\"${value}\"}")
        card bomb
        ;;
    membomb)
        # A memory bomb is held to 256 MB, which fails the run as a whole: the suite expects no result. An action of its own,
        # since the sandbox keeps no variable the suite could set
        # Bytes that are written, since a zeroed bytearray is never touched and takes no memory at all
        python3 -c 'x = b"\x01" * (1 << 30); print("allocated")'
        ;;
    netp)
        # Through the host's proxy, with only a host that /etc/hosts points at a private address granted: every one is refused
        probe rebinding-host curl -sS --max-time 8 -o /dev/null https://granted.example.test/
        probe port-22 curl -sS --max-time 8 -o /dev/null https://granted.example.test:22/
        probe not-granted curl -sS --max-time 8 -o /dev/null https://other.example.test/
        probe address-literal curl -sS --max-time 8 -o /dev/null https://192.168.7.7/
        card netp
        ;;
    *)
        printf '{"text":"hostile"}\n'
        ;;
esac
