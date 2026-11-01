#!/usr/bin/bash

echo "::group:: ===$(basename "$0")==="

set -eoux pipefail

# System hardening that needs more than the files in system_files (sysctl.d, modprobe.d, bootc kargs.d,
# sshd_config.d). Runs after 07-debrand.sh, which gives the kernel's certificate its Amethystora name.

# Signed updates: images from ghcr.io/iarsslen are only accepted with a cosign signature made with the key
# CI signs them with (cosign.pub, installed as /etc/pki/containers/amethystora.pub by the Containerfile).
# The sigstore attachments are looked up as set in /etc/containers/registries.d/amethystora.yaml.
# Other registries keep Fedora's default, so pulling other container images works as before.
test -s /etc/pki/containers/amethystora.pub
jq '.transports.docker["ghcr.io/iarsslen"] = [{
        "type": "sigstoreSigned",
        "keyPath": "/etc/pki/containers/amethystora.pub",
        "signedIdentity": {"type": "matchRepository"}
    }]' /etc/containers/policy.json >/tmp/policy.json
mv /tmp/policy.json /etc/containers/policy.json
chmod 0644 /etc/containers/policy.json

# Root's sudo PATH: ublue-os/main appends /home/linuxbrew/.linuxbrew/bin to secure_path in /etc/sudoers,
# and brew-setup.service gives that directory to the first user. Anything they put there would run as root
# from `sudo <name>`. Drop it; `sudo /home/linuxbrew/.linuxbrew/bin/<name>` still works.
if grep -qE "^Defaults[[:space:]]+secure_path.*linuxbrew" /etc/sudoers; then
    sed -i -E '/^Defaults[[:space:]]+secure_path/ s|:?/home/linuxbrew/\.linuxbrew/bin||' /etc/sudoers
    grep -qE "^Defaults[[:space:]]+secure_path.*linuxbrew" /etc/sudoers && false
    visudo -c -q -f /etc/sudoers
fi

# /dev/uinput makes a virtual keyboard or mouse, which types into any window, a root terminal included.
# Solaar's udev rules hand it to whoever sits at the machine, and so to every app that may reach all of /dev
# (Steam, the Chromium browsers, Discord, OBS, Zoom), by a static node that a later rule cannot take the
# tag back from. Its one line about uinput goes, marked as changed; its rules for Logitech's receivers stay.
# `ame security virtual-input off` gives uinput back, which Steam Input and OpenTabletDriver need.
SOLAAR_RULES=/usr/lib/udev/rules.d/42-logitech-unify-permissions.rules
if grep -q '^KERNEL=="uinput"' "${SOLAAR_RULES}" 2>/dev/null; then
    sed -i '/^KERNEL=="uinput"/d' "${SOLAAR_RULES}"
    sed -i '1i # Changed by Amethystora (build_files/base/08-hardening.sh): the line giving /dev/uinput to whoever\n# sits at the machine is taken out. `ame security virtual-input off` gives it back.' "${SOLAAR_RULES}"
fi

# ddcutil's own rule gives whoever sits at the machine every i2c bus of the display controllers, which can reach other
# chips of the card than the screens', and its modules-load file loads i2c-dev at every boot. Neither is active as the
# image ships: `ame desktop screen-brightness on` copies the image's one-line rule (/usr/share/amethystora/ddc) into
# /etc/udev/rules.d and loads i2c-dev, for the external screens' brightness in quick settings, and off takes it back.
rm -f /usr/lib/udev/rules.d/60-ddcutil-i2c.rules /usr/lib/modules-load.d/ddcutil.conf

# Firewall: Fedora Workstation's default zone accepts incoming connections on every port from 1025 up.
# The Amethystora zone (/usr/lib/firewalld/zones/amethystora.xml) only lets in what a desktop needs.
# Tailscale traffic is trusted: who may reach this machine over the tailnet is set by the tailnet's ACLs,
# and Taildrop and Tailscale SSH need incoming connections.
firewall-offline-cmd --set-default-zone=amethystora
firewall-offline-cmd --zone=trusted --add-interface=tailscale0

# Repeated failed logins from one address: fail2ban watches the journal and has firewalld reject that
# address for a while (/etc/fail2ban/jail.d/10-amethystora.conf, enabled in 17-cleanup.sh).
# Its firewalld action is the image's own, which bans in each Amethystora zone: fail2ban's
# firewallcmd-rich-rules, which it builds on, bans in the default zone alone, and a rule in a zone the
# network's interface is not in never matches anything, so the ban would do nothing at all.
test -f /etc/fail2ban/action.d/firewallcmd-rich-rules.conf
[[ "$(sed -n 's/^banaction[a-z_]* = \([a-z-]*\)\[.*$/\1/p' /etc/fail2ban/jail.d/10-amethystora.conf | sort -u)" == amethystora-zones ]]
F2B_ZONES="$(sed -n 's/^zones = //p' /etc/fail2ban/action.d/amethystora-zones.conf | tr ' ' '\n')"
for zone in amethystora amethystora-home amethystora-away; do
    firewall-offline-cmd --get-zones | tr ' ' '\n' | grep -qx -- "${zone}"
    grep -qx -- "${zone}" <<<"${F2B_ZONES}"
done

# Lock an account for 10 minutes after 10 wrong passwords in a row, against guessing at the login and lock
# screens, sudo and polkit prompts. Settings in /etc/security/faillock.conf.
authselect enable-feature with-faillock

# Fingerprint login, which the installer turns on wherever pam_fprintd is installed. authselect.conf lists
# features in the order they were turned on and the installer's comes after the image's, so it has to be the
# image's last too: then an installed machine's /etc/authselect is the image's own to the byte, and not five
# settings the security report calls changed on this machine.
test -e /usr/lib64/security/pam_fprintd.so
authselect enable-feature with-fingerprint

# The duress finger (`ame security duress`, /usr/libexec/amethystora-duress): the feature
# with-duress-fingerprint, which the owner turns on per machine, adds pam_fprintd's duress option
# (packaging/fprintd) to its line in fingerprint-auth, where GDM and the lock screen ask for a finger, and in
# system-auth, where sudo, polkit and the text consoles do. The vendor profiles the base image made link each
# file to the default profile's; these two become copies with that one change, in local and in the two
# profiles a domain join selects (amethystora-domain-logins). With the feature off, authselect writes what
# it wrote before, to the byte.
for profile in local sssd winbind; do
    for file in fingerprint-auth:allow system-auth:deny; do
        default="/usr/share/authselect/default/${profile}/${file%%:*}"
        vendor="/usr/share/authselect/vendor/${profile}/${file%%:*}"
        [[ -L "${vendor}" && "$(readlink "${vendor}")" == "${default}" ]]
        [[ "$(grep -cE '[[:space:]]pam_fprintd\.so([[:space:]]|$)' "${default}")" -eq 1 ]]
        rm "${vendor}"
        sed -E "s/([[:space:]])pam_fprintd\.so([[:space:]]|$)/\1{if \"with-duress-fingerprint\":pam_fprintd.so duress=${file#*:}|pam_fprintd.so}\2/" \
            "${default}" >"${vendor}"
        [[ "$(grep -c 'with-duress-fingerprint' "${vendor}")" -eq 1 ]]
    done
done

# The duress password (`ame security duress`, /usr/libexec/amethystora-duress): the feature
# with-duress-password, which the owner turns on per machine, adds one pam_exec line after pam_unix in
# system-auth and password-auth, where every password is checked. The helper /usr/libexec/amethystora-duress-pam
# fires the owner's action on an account's duress password and, at the graphical login and the lock screen
# (gdm-password), lets it in; everywhere else it is refused like a wrong password. The line sits before
# faillock's authfail, so a let-in duress login records no failure. It ends in {include if}, as authselect's
# own optional lines do, which leaves the whole line out when the feature is off: a false {if} would leave an
# empty line instead, and every profile written again (a domain joined and left) would differ from the
# image's. The finger step above already made system-auth a real vendor file; password-auth is still a
# symlink here.
DURESS_PAM_LINE='auth [success=done default=ignore] pam_exec.so quiet expose_authtok /usr/libexec/amethystora-duress-pam'
for profile in local sssd winbind; do
    for file in system-auth password-auth; do
        default="/usr/share/authselect/default/${profile}/${file}"
        vendor="/usr/share/authselect/vendor/${profile}/${file}"
        if [[ -L "${vendor}" ]]; then
            [[ "$(readlink "${vendor}")" == "${default}" ]]
            rm "${vendor}"
            cp "${default}" "${vendor}"
        fi
        [[ "$(grep -cE '[[:space:]]pam_unix\.so([[:space:]]|$)' "${vendor}")" -ge 1 ]]
        awk -v line="${DURESS_PAM_LINE} {include if \"with-duress-password\"}" '
            { print }
            !done && /pam_unix\.so/ { print line; done = 1 }' "${vendor}" >"${vendor}.new"
        mv "${vendor}.new" "${vendor}"
        [[ "$(grep -c 'with-duress-password' "${vendor}")" -eq 1 ]]
    done
done

# The guest login (`ame security guest-login`, /usr/libexec/amethystora-guest; plans/ui-ideas/spec-later.md, R6-36): the
# feature with-guest-login, which the owner turns on per machine, keeps the guest's empty password to the login
# screen. In system-auth (sudo, su, polkit, the text consoles) the account is refused outright; in password-auth
# (GDM, SSH and every other network login) it is refused for every service but gdm-password. The lines go first in
# the auth stack, before pam_unix could take the empty password, each marked {include if} as the duress password's is,
# so that with the feature off authselect writes the same bytes as before.
GUEST_SYSTEM='auth        requisite                                            pam_succeed_if.so quiet user != amethystora-guest'
GUEST_SKIP='auth        [success=1 default=ignore]                           pam_succeed_if.so quiet user != amethystora-guest'
GUEST_GDM='auth        requisite                                            pam_succeed_if.so quiet service = gdm-password'
for profile in local sssd winbind; do
    for file in system-auth password-auth; do
        vendor="/usr/share/authselect/vendor/${profile}/${file}"
        [[ -f "${vendor}" && ! -L "${vendor}" ]]
        [[ "$(grep -cE '^auth[[:space:]]' "${vendor}")" -ge 1 ]]
        if [[ "${file}" == system-auth ]]; then
            lines="${GUEST_SYSTEM} {include if \"with-guest-login\"}"
        else
            # Two lines, apart by awk's own \n: -v reads the escapes in what it is given
            lines="${GUEST_SKIP} {include if \"with-guest-login\"}\\n${GUEST_GDM} {include if \"with-guest-login\"}"
        fi
        awk -v lines="${lines}" '!done && /^auth[[:space:]]/ { print lines; done = 1 } { print }' "${vendor}" >"${vendor}.new"
        mv "${vendor}.new" "${vendor}"
        [[ "$(grep -c 'with-guest-login' "${vendor}")" -eq "$([[ "${file}" == system-auth ]] && echo 1 || echo 2)" ]]
    done
done

# Parental Controls' allowed hours at the text consoles and over SSH. GNOME Shell locks a child's desktop
# when its time runs out and will not unlock it until the time comes round again, but a text console or an
# SSH login never meets that lock. There pam_malcontent refuses the account outside its hours, and hands
# what is left to pam_systemd, whose session is closed when that runs out. GDM's own services are left
# out on purpose: there it would close the desktop, unsaved work and all, where GNOME's lock keeps it.
#
# Only its verdict, "no time left" (auth_err), refuses. Its errors, no system bus or an account
# accountsservice does not know, let the login through, and the leading - skips the line if the module is
# ever missing. Administrators skip it: they can lift their own limits, and the module tells every account
# without any that it has none. Both lines go before the include, because the sssd and winbind profiles
# end the account stack early for every local account (pam_localuser.so is sufficient there), so a line
# after it would stop running once the machine joined a domain.
test -e /usr/lib64/security/pam_malcontent.so
for service in login sshd; do
    awk '!done && /^account[[:space:]]+include[[:space:]]/ {
            print "account    [success=1 default=ignore]                  pam_succeed_if.so quiet user ingroup wheel"
            print "-account   [success=ok auth_err=die default=ignore]    pam_malcontent.so"
            done = 1
        }
        { print }
        END { exit !done }' "/etc/pam.d/${service}" >"/tmp/${service}.pam"
    # Written over rather than moved, so that the file keeps its mode and label
    cat "/tmp/${service}.pam" >"/etc/pam.d/${service}"
    rm "/tmp/${service}.pam"
done
# Fedora's SELinux policy lets neither the console's login nor sshd's per-connection process ask
# accountsservice, so without this the module fails and every child logs in at any hour. The same module
# lets sshd keep ~/.google_authenticator up to date for one-time codes (amethystora-pam.cil says why).
semodule -i /usr/share/selinux/packages/amethystora-pam.cil
POLICY="$(echo /etc/selinux/targeted/policy/policy.*)"
for domain in local_login_t sshd_session_t; do
    [[ -n "$(sesearch -A -s "${domain}" -t accountsd_t -c dbus -p send_msg "${POLICY}")" ]]
done
sesearch -T -s sshd_session_t -t user_home_dir_t -c file "${POLICY}" >/tmp/transitions
grep -qx "type_transition sshd_session_t user_home_dir_t:file auth_home_t;" /tmp/transitions
rm /tmp/transitions
# ...which are the domains the two programs run in
[[ "$(matchpathcon -n /usr/bin/login)" == *:login_exec_t:* ]]
[[ "$(matchpathcon -n /usr/libexec/openssh/sshd-session)" == *:sshd_session_exec_t:* ]]

# Audit: the rule set has to be the last word, and it has to survive a full disk.
#
# augenrules concatenates /etc/audit/rules.d/*.rules in name order and loads the result. The file the
# audit package ships, audit.rules, is audit's no-audit set: -D, then "-a task,never", which turns syscall
# auditing off for every process started after it, -w watches included, so not one of the image's keys
# would ever record anything. In its place goes audit's own base configuration (-D and the buffers), under
# a name that sorts first: -D, "delete every rule loaded so far", anywhere later would quietly erase the
# watches the boot-time generator writes into 60- and 61-amethystora.
rm -f /etc/audit/rules.d/audit.rules
install -m 0640 /usr/share/audit-rules/10-base-config.rules /etc/audit/rules.d/10-base.rules
# Nothing may sort after the file that closes the rule set with -e 2. C collation, because that is what
# augenrules sorts with in a service that has no locale of its own.
LAST_RULES="$(find /etc/audit/rules.d -name '*.rules' -printf '%f\n' | LC_ALL=C sort | tail -n1)"
[[ "${LAST_RULES}" == "99-amethystora-finalize.rules" ]]
# A watch needs its path to exist when the rules load; the generator leaves out one that does not, which
# then goes unwatched. Quadlets and resolved's drop-ins (encrypted DNS writes one) are the watched
# directories nothing is guaranteed to have created.
install -d -m 0755 /etc/containers/systemd /etc/systemd/resolved.conf.d

# How much log is kept and what happens when the disk fills. Deliberately not the answer the hardening
# guides give: space_left_action = halt, and the rest of that family, turn a full disk into a laptop
# that stops in the middle of what somebody was doing, or will not boot at all. A gap in the audit log
# is the lesser loss by a wide margin. So: rotate, keep about 250MB of history, and say so in the
# journal rather than stopping the machine.
sed -i -E \
    -e 's|^max_log_file[[:space:]]*=.*|max_log_file = 32|' \
    -e 's|^num_logs[[:space:]]*=.*|num_logs = 8|' \
    -e 's|^max_log_file_action[[:space:]]*=.*|max_log_file_action = ROTATE|' \
    -e 's|^space_left_action[[:space:]]*=.*|space_left_action = SYSLOG|' \
    -e 's|^admin_space_left_action[[:space:]]*=.*|admin_space_left_action = SYSLOG|' \
    -e 's|^disk_full_action[[:space:]]*=.*|disk_full_action = ROTATE|' \
    -e 's|^disk_error_action[[:space:]]*=.*|disk_error_action = SYSLOG|' \
    /etc/audit/auditd.conf
grep -q "^max_log_file = 32$" /etc/audit/auditd.conf
grep -qiE "^(space_left_action|admin_space_left_action|disk_full_action|disk_error_action) = (halt|single|suspend)$" \
    /etc/audit/auditd.conf && false

# Secure Boot certificates, readable by everyone. akmods keeps /etc/pki/akmods/certs at 0750
# root:akmods because the private keys go beside them, and that leaves a normal user unable to see the
# public certificates are there at all: every "is this key enrolled" check run without sudo concluded
# the files did not exist. They are public by definition, so a copy goes where anyone can read it,
# and /usr/libexec/amethystora-mok-status and `ame security secure-boot` work from the copy.
# After 07-debrand.sh, which gives the kernel's certificate its Amethystora name.
SB_CERTS=/usr/share/amethystora/secure-boot
install -Dpm0644 /etc/pki/akmods/certs/amethystora-modules.der "${SB_CERTS}/amethystora-modules.der"
if [[ -f /etc/pki/akmods/certs/akmods-amethystora.der ]]; then
    install -Dpm0644 /etc/pki/akmods/certs/akmods-amethystora.der "${SB_CERTS}/akmods-amethystora.der"
else
    echo "::warning::no akmods-amethystora.der, the kernel's certificate is left out of ${SB_CERTS}"
fi

# Kernel lockdown. In integrity mode the kernel refuses the operations that let root rewrite the kernel
# it is running: loading a module without a signature it trusts, opening /dev/mem, kexec of an unsigned
# image, and BPF that writes into a running program's memory. A root compromise then ends at the next
# reboot instead of moving into the kernel, where nothing running on the machine can find it. BPF that
# only reads, as bpftrace and eBPF security sensors do, is refused by confidentiality mode alone.
#
# Fedora's x86 kernel locks itself down in integrity mode whenever it starts with Secure Boot
# (CONFIG_LOCK_DOWN_IN_EFI_SECURE_BOOT, which 20-tests.sh checks), whatever the arguments say. So this
# argument only decides it on a machine with Secure Boot off, and there it has a real cost: modules
# signed with the machine owner key rather than Fedora's are only trusted once Secure Boot is on and the
# key is enrolled, so such a machine loses the modules built with the image (evdi, for DisplayLink
# docks). Run `ame security secure-boot` and turn Secure Boot on, or, on a machine where that is not
# possible, `ame security lockdown off`, which takes the argument off that machine for good.
# Hibernation is also refused under lockdown; this image does not set it up (swap is zram, and no
# resume= argument is set), so nothing here depends on it.
#
# Not applied to the NVIDIA images at all: their driver is an akmods build signed with the same machine
# owner key, and a machine that booted without a graphics driver would have no way to read this comment.
# With Secure Boot on they are locked down all the same, by the kernel itself.
if [[ "${IMAGE_NAME}" =~ nvidia ]]; then
    echo "NVIDIA image: leaving kernel lockdown off, the driver is a machine-owner-key module"
else
    tee /usr/lib/bootc/kargs.d/11-amethystora-lockdown.toml >/dev/null <<'KARGS'
# Kernel lockdown, set in build_files/base/08-hardening.sh. See the comment there before changing it.
kargs = ["lockdown=integrity"]
KARGS
    grep -q '"lockdown=integrity"' /usr/lib/bootc/kargs.d/11-amethystora-lockdown.toml
fi

# The image has no cron, so nothing runs what packages leave in /etc/cron.daily. Root's only, as the
# other cron directories are expected to be (Lynis FILE-7524).
if [[ -d /etc/cron.daily ]]; then
    chmod 0700 /etc/cron.daily
fi

# Setuid root on programs a desktop never needs, each a target for an attack that overwrites a setuid
# binary in the page cache (Copy Fail, Dirty Frag). sudo does what they do for an administrator: changing
# one's own finger information or password ageing (chfn, chage), group passwords (gpasswd, newgrp),
# Kerberos su (ksu) and mounting WebDAV as a user (mount.davfs; GNOME's Files reaches WebDAV through gvfs).
for program in chfn chage gpasswd newgrp ksu mount.davfs; do
    if [[ -u "/usr/bin/${program}" ]]; then
        chmod u-s "/usr/bin/${program}"
    fi
done

# SSH over vsock: systemd's generator offers sshd to the host of a virtual machine through AF_VSOCK, which
# the firewall does not see. The generator masked in /etc; `ame security ssh-settings off` unmasks it.
mkdir -p /etc/systemd/system-generators
ln -sfn /dev/null /etc/systemd/system-generators/systemd-ssh-generator

# Bluetooth: a random address that changes, and visible and pairable for 30 seconds once asked rather than
# until turned off. BlueZ reads no drop-ins, so its own main.conf, which `ame security bluetooth-privacy
# off` puts back as BlueZ ships it.
sed -i -e 's/^#DiscoverableTimeout = 0$/DiscoverableTimeout = 30/' -e 's/^#PairableTimeout = 0$/PairableTimeout = 30/' \
    -e 's/^#Privacy = off$/Privacy = network/' /etc/bluetooth/main.conf
grep -qx 'DiscoverableTimeout = 30' /etc/bluetooth/main.conf
grep -qx 'PairableTimeout = 30' /etc/bluetooth/main.conf
grep -qx 'Privacy = network' /etc/bluetooth/main.conf

# Lynis knows a system by the ID in /etc/os-release and reports every ID it has no case for as an
# exception, at the top of each audit. This one is Fedora underneath, so it gets Fedora's case.
sed -i 's/^\([[:space:]]*\)"fedora")$/\1"fedora" | "amethystora")/' /usr/share/lynis/include/osdetection
grep -q '"fedora" | "amethystora")' /usr/share/lynis/include/osdetection

echo "::endgroup::"
