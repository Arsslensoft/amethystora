#!/usr/bin/bash
# amethystora-notify-users against stand-ins for logind, runuser, id and notify-send, for 20-tests.sh: what it prints
# for a notice with buttons is one "<account> <key>" line for each account it asked, and the key is one of the
# buttons offered. notify-send runs as the account asked, and the pipe its answer comes through can be written to by
# any program of that account (through /proc/PID/fd/1): an answer with a second line, or one that is no button's key,
# must not reach the caller, which acts on every line as that account's. Runs as root, as the helper does.

set -euo pipefail

NOTIFY="${1:-/usr/libexec/amethystora-notify-users}"
T="$(realpath "$(mktemp -d)")"
UID_A=4590
trap 'rm -rf "${T}" "/run/user/${UID_A}"' EXIT
fail() { echo "test-notify-users: $*" >&2; exit 1; }

command -v python3 >/dev/null || { echo "test-notify-users: no python3 to make a socket with, skipped"; exit 0; }
mkdir -p "${T}/bin" "/run/user/${UID_A}"
# What notify-send finds is a socket named bus, as a session's is
python3 -c 'import socket, sys; socket.socket(socket.AF_UNIX).bind(sys.argv[1])' "/run/user/${UID_A}/bus"

cat >"${T}/bin/loginctl" <<'STANDIN'
#!/usr/bin/bash
case "$1" in
    show-user) echo 1 ;;
    show-session) echo user ;;
esac
STANDIN
cat >"${T}/bin/id" <<STANDIN
#!/usr/bin/bash
case "\$*" in
    "-nu ${UID_A}") echo alice ;;
    "-nG alice") echo "alice wheel" ;;
    *) exec /usr/bin/id "\$@" ;;
esac
STANDIN
# runuser -u USER -- COMMAND...: the command, as nobody else, since this runs as root already
cat >"${T}/bin/runuser" <<'STANDIN'
#!/usr/bin/bash
shift 3
exec "$@"
STANDIN
# What the account's own program put into the pipe: ${T}/said, exactly
cat >"${T}/bin/notify-send" <<STANDIN
#!/usr/bin/bash
cat "${T}/said"
STANDIN
chmod 0755 "${T}"/bin/*

ask() { # WHAT THE ACCOUNT'S PROGRAM WROTE, as printf takes it
    printf "$1" >"${T}/said"
    PATH="${T}/bin:${PATH}" "${NOTIFY}" --action 'fix=Fix it' --action 'never=Never' "A device does not work" "details" 2>/dev/null
}

[[ "$(ask 'fix\n')" == "alice fix" ]] || fail "a button's key was not passed on: $(ask 'fix\n')"
[[ "$(ask 'never\n')" == "alice never" ]] || fail "the other button's key was not passed on"
[[ "$(ask '\n')" == "alice " ]] || fail "a notice closed without an answer: $(ask '\n')"
got="$(ask 'fix\nbob fix\nroot fix\n')"
[[ "${got}" == "alice fix" ]] || fail "a second line, which a caller reads as another account's answer, got through: ${got}"
got="$(ask 'bob fix\n')"
[[ "${got}" == "alice " ]] || fail "words that are no button's key got through: ${got}"
got="$(ask 'default\n')"
[[ "${got}" == "alice " ]] || fail "an answer that is no button's key got through: ${got}"

echo "test-notify-users: every check passed"
