#!/usr/bin/bash
# Logins (plans/done/PLAN-pam-gaps.md), as people would make them:
#
#   1. Parental Controls' allowed hours: a child's login typed at the text console on a keyboard of the
#      suite's own, and over SSH, refused outside the hours and let in inside them; an administrator skips it
#   2. one-time codes for SSH (ame security ssh-codes): set up and turned on in a terminal, a password with and
#      without a code, a code used twice, a scratch code, a key; fail2ban counting wrong codes from another
#      address and banning it; what the watcher accepts; turned off again
#   3. an Active Directory domain, a Samba domain controller in a container here: discovered, joined with SSSD
#      and with winbind, a domain account logging in over SSH and seen by the watcher, left again, and each
#      time the image's PAM setup kept
#
# The domain controller is set up in the background from the start, since installing it takes minutes.
# vmlab-paths: @sf/usr/libexec/amethystora-domain-logins @sf/usr/libexec/amethystora-ssh-codes @sf/etc/realmd.conf @sf/etc/security/* @sf/usr/libexec/amethystora-if-found @ui/modules/found.js
source "$(dirname "$0")/lib.sh"

KID=vmlabkid
KID_PASSWORD=vmlabkid2026
WORK=/run/vmlab-logins
SECRET="/var/home/${TEST_USER}/.google_authenticator"
DOMAIN=ad.test
DC_IP=10.89.0.10
DC_PASSWORD=Vmlab-Admin-2026
ALICE_PASSWORD=Vmlab-Alice-2026
STARTED="$(date +%s)"
mkdir -p "${WORK}"

# --- The domain controller, in the background --------------------------------------------------------------

cat >"${WORK}/dc.sh" <<EOF
set -euxo pipefail
podman rm -f vmlab-dc
podman network exists vmlab-ad || podman network create --subnet 10.89.0.0/24 vmlab-ad
# The server is the container's own process, which starts once everything it needs is there (/dc-ready): one
# started with podman exec belongs to whoever ran that, and did not outlive this unit
podman run -d --name vmlab-dc --hostname dc1 --network vmlab-ad --ip ${DC_IP} --privileged \
    registry.fedoraproject.org/fedora:44 sh -c 'until [ -e /dc-ready ]; do sleep 1; done
        exec samba --foreground --no-process-group --debug-stdout'
podman exec vmlab-dc dnf -y -q install --setopt=install_weak_deps=False samba-dc samba-client krb5-workstation
podman exec vmlab-dc sh -c 'rm -f /etc/samba/smb.conf && samba-tool domain provision --realm=AD.TEST --domain=AD \
    --server-role=dc --dns-backend=SAMBA_INTERNAL --use-rfc2307 --host-name=dc1 --host-ip=${DC_IP} \
    --adminpass=${DC_PASSWORD} >/dev/null'
# Samba's MIT KDC takes the realm from /etc/krb5.conf, and without it dies and takes samba with it
podman exec vmlab-dc cp /var/lib/samba/private/krb5.conf /etc/krb5.conf
podman exec vmlab-dc samba-tool user create alice ${ALICE_PASSWORD}
podman exec vmlab-dc samba-tool user setexpiry alice --noexpiry
podman exec vmlab-dc touch /dc-ready
touch ${WORK}/dc-ready
EOF
systemctl reset-failed vmlab-dc-setup.service 2>/dev/null
systemd-run --unit=vmlab-dc-setup --collect --quiet -p "StandardOutput=file:${WORK}/dc.log" \
    -p StandardError=inherit /usr/bin/bash "${WORK}/dc.sh"

# --- Helpers -----------------------------------------------------------------------------------------------

# SSH's askpass: the password for a password prompt, the code for a code prompt; each prompt is noted
cat >"${WORK}/askpass" <<'EOF'
#!/usr/bin/bash
printf '%s\n' "$1" >>"${ASK_LOG}"
case "$1" in
    *[Vv]erification*) printf '%s\n' "${ASK_CODE}" ;;
    *) printf '%s\n' "${ASK_PASSWORD}" ;;
esac
EOF
chmod 0755 "${WORK}/askpass"
# ssh_once LOGIN PASSWORD [CODE]: one password login over SSH, which prints the account's name; SSH_FROM runs ssh
# elsewhere (another network namespace)
SSH_FROM=()
SSH_TO=127.0.0.1
ssh_once() {
    : >"${WORK}/asked"
    # The login by a configuration file: OpenSSH 10.6 (Fedora 45) refuses a domain's AD\alice given with -l or -o
    printf 'User "%s"\n' "$1" >"${WORK}/ssh.conf"
    "${SSH_FROM[@]}" env ASK_PASSWORD="$2" ASK_CODE="${3:-}" ASK_LOG="${WORK}/asked" SSH_ASKPASS="${WORK}/askpass" \
        SSH_ASKPASS_REQUIRE=force DISPLAY=:0 setsid -w ssh -F "${WORK}/ssh.conf" -o StrictHostKeyChecking=no \
        -o UserKnownHostsFile=/dev/null -o PubkeyAuthentication=no -o LogLevel=ERROR -o ConnectTimeout=20 \
        -o NumberOfPasswordPrompts="${PROMPTS:-1}" "${SSH_TO}" id -un </dev/null 2>>"${WORK}/ssh.err"
}
# ssh_in LOGIN PASSWORD [CODE]: the same, tried again when sshd turned it away before asking anything, as it
# does for a while after failed logins from one address (PerSourcePenalties)
ssh_in() {
    local try out
    for try in 1 2 3 4 5 6; do
        out="$(ssh_once "$@")" && { echo "${out}"; return 0; }
        [[ -s "${WORK}/asked" ]] && return 1
        sleep 15
    done
    return 1
}
# refused LOGIN PASSWORD [CODE]: whether SSH asked and turned the login away
refused() { ! ssh_in "$@" >/dev/null && [[ -s "${WORK}/asked" ]]; }
# key_login: the account's key, without a password or a code
key_login() {
    local try
    for try in 1 2 3 4 5 6; do
        runuser -u "${TEST_USER}" -- ssh -F none -i "/var/home/${TEST_USER}/.ssh/vmlab-key" -o BatchMode=yes \
            -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR "${TEST_USER}@127.0.0.1" true &&
            return 0
        sleep 15
    done
    return 1
}
# The code a phone app would show now for the account's codes, as RFC 6238 computes it
totp() {
    python3 - "${SECRET}" <<'EOF'
import base64, hashlib, hmac, struct, sys, time
secret = open(sys.argv[1]).readline().strip()
key = base64.b32decode(secret + '=' * (-len(secret) % 8))
digest = hmac.new(key, struct.pack('>Q', int(time.time()) // 30), hashlib.sha1).digest()
offset = digest[-1] & 0x0F
print(f'{(struct.unpack(">I", digest[offset:offset + 4])[0] & 0x7FFFFFFF) % 1000000:06d}')
EOF
}
# type_lines LINE...: each LINE and Enter typed on a keyboard of the suite's own (uinput), into whichever
# console is in front: lower-case letters and digits
type_lines() {
    python3 - "$@" <<'EOF'
import fcntl, os, struct, sys, time
keys = {c: k for k, c in enumerate('1234567890', 2)}
for row, first in (('qwertyuiop', 16), ('asdfghjkl', 30), ('zxcvbnm', 44)):
    keys.update({c: k for k, c in enumerate(row, first)})
keys['\n'] = 28
fd = os.open('/dev/uinput', os.O_WRONLY | os.O_NONBLOCK)
fcntl.ioctl(fd, 0x40045564, 1)                       # UI_SET_EVBIT EV_KEY
for key in set(keys.values()):
    fcntl.ioctl(fd, 0x40045565, key)                 # UI_SET_KEYBIT
fcntl.ioctl(fd, 0x405c5503, struct.pack('HHHH80sI', 6, 1, 1, 1, b'vmlab keyboard', 0))   # UI_DEV_SETUP
fcntl.ioctl(fd, 0x5501)                              # UI_DEV_CREATE
time.sleep(1)
for line in sys.argv[1:]:
    for char in line + '\n':
        for value in (1, 0):
            os.write(fd, struct.pack('llHHi', 0, 0, 1, keys[char], value) + struct.pack('llHHi', 0, 0, 0, 0, 0))
            time.sleep(0.03)
    time.sleep(1.5)
fcntl.ioctl(fd, 0x5502)                              # UI_DEV_DESTROY
EOF
}
# The audit lines PAM wrote since a time (seconds), one a line
pam_audit_since() { journalctl --since "@$1" _TRANSPORT=audit -o cat --no-pager | grep "op=PAM:"; }
# The SELinux refusals since a time (seconds), D-Bus's among them, less the lab's own
denials_since() { journalctl --since "@$1" _TRANSPORT=audit -o cat --no-pager | grep -E "avc: +denied" | grep -E "$2"; }
# What the security watcher finds changed in /etc of what logins rest on, after a run of its own
drift_now() {
    wait_for 600 sh -c '[ "$(systemctl show -p ActiveState --value amethystora-security-watch.service)" != activating ]'
    systemctl start amethystora-security-watch.service
    jq -c '[.drift[]? | select(.path | test("^/etc/(pam\\.d|ssh|authselect|security)/"))]' \
        /var/lib/amethystora/security/status.json
}
# A check of the security report, by id, as JSON
report() { /usr/libexec/amethystora-security-status --json 2>/dev/null | jq -c --arg id "$1" '.checks[] | select(.id == $id)'; }

SSHD_WAS="$(systemctl is-active sshd.service)"
# The console in front when the suite began, from the kernel itself: fgconsole needs a terminal, which the lab's
# helper has none of, and its fallback, 1, is GDM's own console, so switching back to it left the account's
# session (on the next console) inactive for every suite after this one
CONSOLE_WAS="$(sed -n 's/^tty//p' /sys/class/tty/tty0/active 2>/dev/null)"
[[ "${CONSOLE_WAS}" =~ ^[0-9]+$ ]] || CONSOLE_WAS="$(fgconsole 2>/dev/null || echo 1)"
SSSD_WAS="$(systemctl is-enabled sssd.service 2>/dev/null)"
ODDJOBD_WAS="$(systemctl is-enabled oddjobd.service 2>/dev/null)"
cleanup() {
    [[ -n "$(realm list --name-only 2>/dev/null)" ]] && realm leave >/dev/null 2>&1
    systemctl stop vmlab-dc-setup.service 2>/dev/null
    podman rm -f vmlab-dc >/dev/null 2>&1
    podman network rm vmlab-ad >/dev/null 2>&1
    podman rmi registry.fedoraproject.org/fedora:44 >/dev/null 2>&1
    [[ "$(/usr/libexec/amethystora-ssh-codes status)" == on ]] && /usr/libexec/amethystora-ssh-codes off
    fail2ban-client set sshd unbanip 192.168.77.2 >/dev/null 2>&1
    ip netns del vmlab-ssh 2>/dev/null
    firewall-cmd --zone=amethystora --remove-service=ssh >/dev/null 2>&1
    [[ "${SSHD_WAS}" == active ]] || systemctl stop sshd.service
    [[ "${SSSD_WAS}" == enabled ]] && systemctl enable sssd.service >/dev/null 2>&1
    [[ "${ODDJOBD_WAS}" == enabled ]] || systemctl disable --now oddjobd.service >/dev/null 2>&1
    cmp -s /usr/etc/samba/smb.conf /etc/samba/smb.conf || cat /usr/etc/samba/smb.conf >/etc/samba/smb.conf
    loginctl terminate-user "${KID}" 2>/dev/null
    userdel -r "${KID}" 2>/dev/null
    rm -rf "/var/home/alice@${DOMAIN}" /var/home/alice@AD
    rm -f "${SECRET}" "/var/home/${TEST_USER}/.ssh/vmlab-key" "/var/home/${TEST_USER}/.ssh/vmlab-key.pub"
    [[ -f "${WORK}/authorized_keys" ]] && cat "${WORK}/authorized_keys" >"/var/home/${TEST_USER}/.ssh/authorized_keys"
    [[ -f "${WORK}/authorized_keys" ]] || rm -f "/var/home/${TEST_USER}/.ssh/authorized_keys"
    faillock --user "${TEST_USER}" --reset
    chvt "${CONSOLE_WAS}" 2>/dev/null
}
trap cleanup EXIT

systemctl start sshd.service
check "the SSH server runs, and fail2ban with it" wait_for 60 systemctl is-active --quiet fail2ban.service
check "the image's policy module for logins is loaded" sh -c 'semodule -l | grep -qx amethystora-pam'

# --- 1. Parental Controls' allowed hours ------------------------------------------------------------------

getent passwd "${KID}" >/dev/null || useradd -m -c "Child account" "${KID}"
echo "${KID}:${KID_PASSWORD}" | chpasswd
LIMITS=com.endlessm.ParentalControls.SessionLimits
KID_OBJECT="$(busctl call org.freedesktop.Accounts /org/freedesktop/Accounts org.freedesktop.Accounts \
    FindUserByName s "${KID}" | awk -F'"' '{print $2}')"
# hours now|later: the child's allowed hours around this moment, or an hour starting two hours from now
hours() {
    local now start end
    now=$(($(date +%s) % 86400))
    if [[ "$1" == now ]]; then
        start=$((now > 600 ? now - 600 : 0))
        end=$((now + 3600 > 86400 ? 86400 : now + 3600))
    else
        start=$(((now + 7200) % 86400))
        ((start > 82800)) && start=3600
        end=$((start + 3600))
    fi
    busctl set-property org.freedesktop.Accounts "${KID_OBJECT}" "${LIMITS}" DailySchedule "(uu)" "${start}" "${end}" &&
        busctl set-property org.freedesktop.Accounts "${KID_OBJECT}" "${LIMITS}" LimitType u 1
}
console_session() { loginctl list-sessions --no-legend | awk -v u="${KID}" '$3 == u && $7 == "tty3" {print $1}'; }
has_console_session() { [[ -n "$(console_session)" ]]; }
# login's password prompt is up once it has turned the echo off: what is typed before that, it throws away
password_asked() { stty -F /dev/tty3 -a 2>/dev/null | grep -qw -- -echo; }
console_in_front() { [[ "$(cat /sys/class/tty/tty0/active)" == tty3 ]]; }
# The first switch away from GNOME's console in a boot is slow, and what is typed before it lands nowhere:
# the name again until login asks for the password
console_login() {
    local try
    systemctl restart getty@tty3.service
    chvt 3
    wait_for 20 console_in_front
    sleep 3
    for try in 1 2 3; do
        type_lines "${KID}"
        wait_for 15 password_asked && break
    done
    type_lines "${KID_PASSWORD}"
    wait_for 20 has_console_session
}

if hours later; then
    t="$(date +%s)"
    out="$(ssh_in "${KID}" "${KID_PASSWORD}")"
    if [[ -z "${out}" && -s "${WORK}/asked" ]] && journalctl --since "@${t}" -u sshd -o cat | grep -q "Access denied for user ${KID} by PAM account configuration"; then
        ok "outside the allowed hours, SSH refuses the child"
    else
        not_ok "outside the allowed hours, the child logged in over SSH" "${out}" "$(pam_audit_since "${t}" | tail -n 3)" \
            "$(denials_since "${t}" accountsd)"
    fi
    t="$(date +%s)"
    console_login
    if [[ -z "$(console_session)" ]] && pam_audit_since "${t}" | grep 'exe="/usr/bin/login"' | grep "acct=\"${KID}\"" |
        grep -q 'op=PAM:accounting.*res=failed'; then
        ok "outside the allowed hours, the text console refuses the child"
    else
        not_ok "outside the allowed hours, the text console did not refuse the child" "$(loginctl list-sessions --no-legend)" \
            "$(pam_audit_since "${t}" | tail -n 3)" "$(denials_since "${t}" accountsd)"
    fi
    loginctl terminate-user "${KID}" 2>/dev/null

    hours now
    t="$(date +%s)"
    out="$(ssh_in "${KID}" "${KID_PASSWORD}")"
    if [[ "${out}" == "${KID}" ]]; then ok "inside the allowed hours, SSH lets the child in"; else not_ok "inside the allowed hours, SSH refused the child" "$(tail -n 3 "${WORK}/ssh.err")"; fi
    if console_login; then
        limit="$(systemctl show "session-$(console_session).scope" -p RuntimeMaxUSec --value)"
        if [[ -n "${limit}" && "${limit}" != infinity ]]; then
            ok "inside the allowed hours, the text console lets the child in, until the hours end (${limit})"
        else
            not_ok "the child's console session has no end: RuntimeMaxUSec=${limit}"
        fi
    else
        not_ok "inside the allowed hours, the text console did not let the child in" "$(pam_audit_since "${t}" | tail -n 3)"
    fi
    loginctl terminate-user "${KID}" 2>/dev/null
    chvt "${CONSOLE_WAS}"
    if pam_audit_since "${t}" | grep "acct=\"${KID}\"" | grep 'op=PAM:accounting' | grep -q pam_malcontent; then
        ok "pam_malcontent decides the child's logins"
    else
        not_ok "pam_malcontent took no part in the child's logins" "$(pam_audit_since "${t}" | grep "acct=\"${KID}\"" | tail -n 3)"
    fi
    t="$(date +%s)"
    out="$(ssh_in "${TEST_USER}" "${TEST_USER}")"
    if [[ "${out}" == "${TEST_USER}" ]] && ! pam_audit_since "${t}" | grep "acct=\"${TEST_USER}\"" |
        grep 'op=PAM:accounting' | grep -q pam_malcontent; then
        ok "an administrator logs in over SSH without pam_malcontent"
    else
        not_ok "an administrator's SSH login: ${out}" "$(pam_audit_since "${t}" | tail -n 3)"
    fi
    hours later
else
    not_ok "could not set the child's allowed hours through accountsservice"
fi
d="$(denials_since "${STARTED}" accountsd)"
if [[ -z "${d}" ]]; then ok "SELinux refused no login accountsservice"; else not_ok "SELinux refused logins accountsservice" "${d}"; fi

# --- 2. One-time codes for SSH ---------------------------------------------------------------------------

rm -f "${SECRET}"
out="$(in_terminal 120 'Enter code from app' $'-1\r' -- runuser -u "${TEST_USER}" -- bash -lc 'cd; ame security ssh-codes setup')"
if [[ $? -eq 0 && -f "${SECRET}" && "$(stat -c %C "${SECRET}")" == *:auth_home_t:* ]]; then
    ok "ame security ssh-codes setup makes the account's codes, labelled for sshd"
else
    not_ok "ame security ssh-codes setup" "$(tail -n 8 <<<"${out}")" "$(ls -Z "${SECRET}" 2>&1)"
fi
out="$(in_terminal 120 'Turn one-time codes for SSH on\?' y '\[sudo\] password for' "${TEST_USER}"$'\r' \
    -- runuser -u "${TEST_USER}" -- bash -lc 'cd; ame security ssh-codes on')"
settings="$(sshd -T 2>/dev/null | awk '{ $1 = tolower($1) } 1' | grep -E '^(kbdinteractiveauthentication|passwordauthentication|authenticationmethods) ' | sort | paste -sd ' ')"
if [[ "$(/usr/libexec/amethystora-ssh-codes status)" == on &&
    "${settings}" == "authenticationmethods publickey keyboard-interactive kbdinteractiveauthentication yes passwordauthentication no" ]]; then
    ok "ame security ssh-codes on"
else
    not_ok "ame security ssh-codes on: ${settings}" "$(tail -n 8 <<<"${out}")"
fi
[[ "$(report ssh-codes | jq -r .state)" == on ]] && ok "the report says one-time codes for SSH are on" ||
    not_ok "the report says: $(report ssh-codes)"

t="$(date +%s)"
refused "${TEST_USER}" "${TEST_USER}" '' && ok "a password without a code is refused" || not_ok "a password without a code logs in"
refused "${TEST_USER}" "${TEST_USER}" 000000 && ok "a wrong code is refused" || not_ok "a wrong code logs in"
refused "${TEST_USER}" "not-${TEST_USER}" "$(totp)" && ok "a wrong password with the right code is refused" ||
    not_ok "a wrong password with the right code logs in"
# The account's codes take three logins in 30 seconds (setup's --rate-limit=3 --rate-time=30), the three
# refused above among them
sleep 31
inode="$(stat -c %i "${SECRET}")"
code="$(totp)"
out="$(ssh_in "${TEST_USER}" "${TEST_USER}" "${code}")"
if [[ "${out}" == "${TEST_USER}" ]]; then ok "the password and the right code log in"; else not_ok "the password and the right code were refused" "$(journalctl --since "@${t}" -u sshd -o cat | grep -iv "pam_unix\|disconnect" | tail -n 6)"; fi
if [[ "$(stat -c %i "${SECRET}")" != "${inode}" && "$(stat -c %C "${SECRET}")" == *:auth_home_t:* ]] &&
    grep -qE '^" DISALLOW_REUSE [0-9]+' "${SECRET}"; then
    ok "sshd rewrote the account's codes, the code just used noted and the label kept"
else
    not_ok "sshd did not rewrite the account's codes" "$(ls -Z "${SECRET}")" "$(denials_since "${t}" google_authenticator)"
fi
refused "${TEST_USER}" "${TEST_USER}" "${code}" && ok "a code is refused the second time" || not_ok "a code logged in twice"
sleep 31
scratch="$(grep -m1 -E '^[0-9]{8}$' "${SECRET}")"
if [[ "$(ssh_in "${TEST_USER}" "${TEST_USER}" "${scratch}")" == "${TEST_USER}" ]] && ! grep -qx "${scratch}" "${SECRET}" &&
    refused "${TEST_USER}" "${TEST_USER}" "${scratch}"; then
    ok "a scratch code logs in once"
else
    not_ok "a scratch code did not log in exactly once"
fi
refused "${KID}" "${KID_PASSWORD}" '' && ok "an account without codes cannot log in with a password" ||
    not_ok "an account without codes logged in with a password alone"
ssh_dir="/var/home/${TEST_USER}/.ssh"
[[ -f "${ssh_dir}/authorized_keys" ]] && cp "${ssh_dir}/authorized_keys" "${WORK}/authorized_keys"
runuser -u "${TEST_USER}" -- sh -c "mkdir -p -m 0700 ${ssh_dir} && ssh-keygen -q -t ed25519 -N '' -f ${ssh_dir}/vmlab-key &&
    cat ${ssh_dir}/vmlab-key.pub >>${ssh_dir}/authorized_keys && chmod 0600 ${ssh_dir}/authorized_keys && restorecon -R ${ssh_dir}"
check "a key logs in without a code" key_login
d="$(denials_since "${STARTED}" "scontext=system_u:system_r:sshd")"
if [[ -z "${d}" ]]; then ok "SELinux refused sshd nothing"; else not_ok "SELinux refused sshd" "${d}"; fi

# fail2ban, from an address of another network namespace: this machine's own are never banned
ip netns add vmlab-ssh
ip link add vmlab-ssh0 type veth peer name vmlab-ssh1
nmcli device set vmlab-ssh0 managed no 2>/dev/null
ip link set vmlab-ssh1 netns vmlab-ssh
ip addr add 192.168.77.1/24 dev vmlab-ssh0
ip link set vmlab-ssh0 up
ip netns exec vmlab-ssh sh -c 'ip addr add 192.168.77.2/24 dev vmlab-ssh1 && ip link set vmlab-ssh1 up && ip link set lo up'
firewall-cmd --zone=amethystora --add-service=ssh >/dev/null
failed() { fail2ban-client status sshd | awk -F'\t' '/Total failed/ {print $2}'; }
before="$(failed)"
SSH_FROM=(ip netns exec vmlab-ssh)
SSH_TO=192.168.77.1
for attempt in 1 2 3; do
    PROMPTS=3 ssh_once "${TEST_USER}" "${TEST_USER}" 000000 >/dev/null
    fail2ban-client status sshd | grep -q 192.168.77.2 && break
    sleep 25
done
if fail2ban-client status sshd | grep -q 192.168.77.2; then
    ok "fail2ban counted $(($(failed) - before)) wrong codes from another address and banned it"
else
    not_ok "fail2ban did not ban an address that gave wrong codes: $(($(failed) - before)) counted"
fi
[[ -z "$(ssh_once "${TEST_USER}" "${TEST_USER}" "$(totp)")" && ! -s "${WORK}/asked" ]] &&
    ok "the banned address cannot reach SSH, even with the right code" || not_ok "the banned address still reached SSH"
SSH_FROM=()
SSH_TO=127.0.0.1
fail2ban-client set sshd unbanip 192.168.77.2 >/dev/null
ip netns del vmlab-ssh
firewall-cmd --zone=amethystora --remove-service=ssh >/dev/null

if [[ "$(drift_now)" == "[]" ]]; then ok "the watcher takes codes on for the image's own setting"; else not_ok "the watcher reports codes on" "$(drift_now)"; fi
cp -a /etc/pam.d/sshd "${WORK}/sshd.pam"
echo "auth       optional     pam_permit.so" >>/etc/pam.d/sshd
if [[ "$(drift_now)" == *'"changed","path":"/etc/pam.d/sshd"'* ]]; then ok "the watcher reports a line added by hand to sshd's PAM"; else not_ok "the watcher missed a line added to sshd's PAM" "$(drift_now)"; fi
cat "${WORK}/sshd.pam" >/etc/pam.d/sshd

out="$(in_terminal 120 '\[sudo\] password for' "${TEST_USER}"$'\r' -- runuser -u "${TEST_USER}" -- bash -lc 'cd; ame security ssh-codes off')"
if [[ "$(/usr/libexec/amethystora-ssh-codes status)" == off ]] && cmp -s /etc/pam.d/sshd /usr/etc/pam.d/sshd &&
    [[ ! -e /etc/ssh/sshd_config.d/30-amethystora-ssh-codes.conf ]]; then
    ok "ame security ssh-codes off leaves sshd as the image ships it"
else
    not_ok "ame security ssh-codes off" "$(tail -n 6 <<<"${out}")" "$(diff /usr/etc/pam.d/sshd /etc/pam.d/sshd)"
fi
[[ "$(ssh_in "${TEST_USER}" "${TEST_USER}")" == "${TEST_USER}" ]] && ok "a password alone logs in again" || not_ok "a password alone no longer logs in"

# --- 3. An Active Directory domain -----------------------------------------------------------------------

if ! wait_for 1200 test -e "${WORK}/dc-ready"; then
    skip "the domain controller could not be set up here: $(tail -n 3 "${WORK}/dc.log" | paste -sd ' ')"
    finish
fi
ok "a Samba domain controller was made for ${DOMAIN} ($(($(date +%s) - STARTED)) s from the suite's start)"
BRIDGE="$(podman network inspect vmlab-ad --format '{{.NetworkInterface}}')"
resolvectl dns "${BRIDGE}" "${DC_IP}"
resolvectl domain "${BRIDGE}" "~${DOMAIN}"
# samba-tool wrote the account into the database directly, which says nothing of the server: wait until its
# DNS answers what realmd asks first
dc_answers() { resolvectl query --cache=no -t SRV "_ldap._tcp.${DOMAIN}"; }
wait_for 180 dc_answers || not_ok "the domain controller's DNS does not answer" \
    "$(resolvectl query --cache=no -t SRV "_ldap._tcp.${DOMAIN}" 2>&1)" "$(podman ps -a --format '{{.Names}} {{.Status}}')" \
    "$(podman logs --tail 15 vmlab-dc 2>&1)" "$(resolvectl status "${BRIDGE}" 2>&1 | head -n 8)"
IMAGE_FEATURES="$(grep -vE '^[[:space:]]*(#|$)' /usr/etc/authselect/authselect.conf | tail -n +2 | paste -sd ' ')"

discovered="$(realm discover "${DOMAIN}" 2>&1)"
missing="$(sed -n 's/^ *required-package: //p' <<<"${discovered}" | xargs -r rpm -q | grep 'not installed')"
if grep -q "realm-name: AD.TEST" <<<"${discovered}" && [[ -z "${missing}" ]]; then
    ok "realm finds the domain, and every package it needs is in the image"
else
    not_ok "realm discover" "${discovered}" "${missing}"
fi

# join sssd|winbind: as Settings' Enterprise Login or `realm join` would, with the domain administrator's password
join() {
    local out
    if out="$(printf '%s\n' "${DC_PASSWORD}" | realm join --client-software="$1" -U Administrator "${DOMAIN}" 2>&1)" &&
        [[ "$(grep -vE '^[[:space:]]*(#|$)' /etc/authselect/authselect.conf | paste -sd ' ')" == "$1 ${IMAGE_FEATURES} with-mkhomedir" ]] &&
        /usr/libexec/amethystora-domain-logins check; then
        ok "joined with $1, the image's authselect features kept: $(grep -vE '^[[:space:]]*(#|$)' /etc/authselect/authselect.conf | paste -sd ' ')"
    else
        not_ok "the join with $1" "$(tail -n 8 <<<"${out}")" "$(grep -v '^#' /etc/authselect/authselect.conf | paste -sd ' ')"
        return 1
    fi
}
# leave sssd|winbind: and /etc as the image ships it again
leave() {
    local out
    out="$(realm leave "${DOMAIN}" 2>&1)"
    if diff -r /usr/etc/authselect /etc/authselect >/dev/null && cmp -s /usr/etc/security/pam_winbind.conf /etc/security/pam_winbind.conf &&
        ! /usr/libexec/amethystora-domain-logins check; then
        ok "left the $1 domain: authselect and pam_winbind as the image ships them"
    else
        not_ok "leaving the $1 domain" "$(tail -n 4 <<<"${out}")" "$(diff -r /usr/etc/authselect /etc/authselect | head -n 8)" \
            "$(diff /usr/etc/security/pam_winbind.conf /etc/security/pam_winbind.conf)"
    fi
    if [[ "$(drift_now)" == "[]" && -z "$(report domain-login)" ]]; then ok "after leaving $1, the watcher and the report see the image's settings"; else not_ok "after leaving $1" "$(drift_now)" "$(report domain-login)"; fi
}
# The domain's account, as each client names it
domain_login() {
    local login="$1" home="$2" out
    out="$(ssh_in "${login}" "${ALICE_PASSWORD}")"
    if [[ "${out}" == "${login}" && "$(stat -c %C "${home}" 2>/dev/null)" == *:user_home_dir_t:* ]]; then
        ok "${login} logs in over SSH, and gets a home folder in /var/home"
    else
        not_ok "${login} over SSH: ${out}" "$(tail -n 3 "${WORK}/ssh.err")" "$(ls -dZ "${home}" 2>&1)"
    fi
    if [[ "$(drift_now)" == "[]" ]] && report domain-login | grep -q "${DOMAIN}"; then
        ok "the watcher takes the domain's setup for the image's, and the report says who can log in"
    else
        not_ok "the watcher or the report on a domain" "$(drift_now)" "$(report domain-login)"
    fi
}

if join sssd; then
    domain_login "alice@${DOMAIN}" "/var/home/alice@${DOMAIN}"
    refused "${KID}" "${KID_PASSWORD}" && ok "the child is still refused outside the allowed hours" ||
        not_ok "joined to a domain, the child logged in outside the allowed hours"
    # What the watcher and the audit rules read, for an account /etc/passwd does not have. A finding carries
    # its run's start, so the time is taken with no run under way.
    drift_now >/dev/null
    t="$(date +%s)"
    runuser -u "alice@${DOMAIN}" -- sh -c 'mkdir -p ~/.local/bin && printf "#!/bin/sh\nexec /usr/bin/sudo \"\$@\"\n" >~/.local/bin/sudo && chmod 0755 ~/.local/bin/sudo'
    drift_now >/dev/null
    if jq -e -s --argjson t "${t}" 'any(.[]; .time >= $t and .key == "user-paths" and (.text | contains("alice@ad.test/.local/bin/sudo")))' \
        /var/log/amethystora-security-events.jsonl >/dev/null; then
        ok "the watcher reads the domain account's home"
    else
        not_ok "the watcher did not see a sudo in the domain account's home"
    fi
    rm -f "/var/home/alice@${DOMAIN}/.local/bin/sudo"
    /usr/libexec/amethystora-audit-home-rules >/dev/null 2>&1
    if grep -qs "/var/home/alice@${DOMAIN}/" /etc/audit/rules.d/61-amethystora-home.rules /run/amethystora/sensor/watch.list; then
        ok "the audit watches cover the domain account's home"
    else
        not_ok "no audit watch in the domain account's home"
    fi
    leave sssd
fi
if join winbind; then
    domain_login 'AD\alice' /var/home/alice@AD
    leave winbind
fi
finish
