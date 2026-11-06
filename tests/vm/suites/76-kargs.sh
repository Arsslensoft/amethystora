#!/usr/bin/bash
# vmlab-vm: installer
# The image's kernel arguments at the same restart on a machine rpm-ostree updates (amethystora-kargs,
# plans/done/PLAN-kargs-same-restart.md), on the installer-layout machine, whose /boot keeps a staged deployment through a
# restart. An update of the running system is made here (as suite 89 makes them) whose kargs.d adds vmlab.added=1 and
# drops vsyscall=none, and is staged with ostree alone, with no rpm-ostree and no bootc to apply kargs.d:
# - staged with an argument of the owner's, the path unit gives it the image's at once: the addition, not the removed
#   one, the owner's kept; status says so, and a soft reboot is refused with kernel;
# - staged again with the path unit masked, nothing changes it until the shutdown, where the finalize unit does, and the
#   kernel then starts with the image's arguments and the owner's;
# - the machine is put back on its own image.
# What 20-tests.sh holds against stand-ins is not repeated: the arithmetic, two stagings before one restart, the
# architecture filter, and the catch-up hook (32-kargs-catch-up.sh).
# vmlab-paths: @sf/usr/libexec/amethystora-kargs @sf/usr/lib/systemd/system/amethystora-kargs* @sf/usr/libexec/amethystora-soft-reboot
source "$(dirname "$0")/lib.sh"

STATE=/var/lib/vmlab-tests/76-kargs
mkdir -p "${STATE}"
HELPER=/usr/libexec/amethystora-kargs
KARGS_D=usr/lib/bootc/kargs.d
# Nothing else stages anything while this runs
systemctl stop uupd.timer 2>/dev/null

same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
remember() { printf '%s\n' "$2" >"${STATE}/$1"; }
recall() { cat "${STATE}/$1" 2>/dev/null; }
bootc_json() { bootc status --format=json | jq -r "$1"; }
REPO=/sysroot/ostree/repo
rw() { unshare -m sh -c 'mount -o remount,rw /sysroot && "$@"' sh "$@"; }
# The staged deployment's arguments, one a line, as the restart will start them
staged_args() {
    python3 -c '
from gi.repository import GLib
d = open("/run/ostree/staged-deployment", "rb").read()
v = GLib.Variant.new_from_bytes(GLib.VariantType.new("a{sv}"), GLib.Bytes.new(d), False).unpack()
print("\n".join(v["kargs"]))' 2>/dev/null
}
has() { grep -qxF -- "$2" <<<"$1"; }
synced() { [[ "$("${HELPER}" status --json | jq -r .synced)" == true ]]; }
stage() { timeout 1500 ostree admin deploy --stage vmlab/kargs --karg-append=vmlab.owner=1 2>&1; }
unstage() { ostree admin undeploy 0 >/dev/null 2>&1 || rpm-ostree cleanup --pending >/dev/null 2>&1; }

if [[ "${SUITE_PHASE}" == 1 ]]; then
    check "the helper is there" test -x "${HELPER}"
    check "the path unit and the finalize unit are enabled" systemctl is-enabled --quiet amethystora-kargs.path amethystora-kargs-finalize.service
    settled() { [[ "$(systemctl show -P ActiveState amethystora-signed-updates.service)" != activating ]]; }
    wait_for 1200 settled
    [[ "$(bootc_json '.status.staged')" == null ]] || unstage
    remember original "$(bootc_json '.status.booted.ostree.checksum')"
    remember deployments "$(rpm-ostree status --json | jq '.deployments | length')"
    if ! grep -qw vsyscall=none /proc/cmdline; then
        not_ok "the machine does not start with vsyscall=none, which this suite takes off" "$(cat /proc/cmdline)"
        finish
    fi
    dir="${REPO}/tmp/vmlab-kargs"
    toml="${KARGS_D}/10-amethystora-hardening.toml"
    rw rm -rf "${dir}"
    if ! out="$(rw ostree checkout --repo="${REPO}" -H "$(recall original)" "${dir}" 2>&1 &&
        rw sh -c "cd '${dir}' && sed 's/, \"vsyscall=none\"//' ${toml} >${toml}.new &&
            chcon --reference=${toml} ${toml}.new && mv -f ${toml}.new ${toml} &&
            printf 'kargs = [\"vmlab.added=1\"]\n' >${KARGS_D}/50-vmlab.toml &&
            chcon --reference=${toml} ${KARGS_D}/50-vmlab.toml" 2>&1 &&
        rw ostree commit --repo="${REPO}" --parent="$(recall original)" -b vmlab/kargs -I --consume --bootable \
            --add-metadata-string=version=vmlab-kargs "${dir}" 2>&1)"; then
        not_ok "could not make the update" "$(tail -n 15 <<<"${out}")"
        finish
    fi

    out="$(stage)" || not_ok "staging the update" "${out}"
    if wait_for 120 synced; then ok "the path unit synced the staged update"; else not_ok "nothing synced the staged update" "$("${HELPER}" status 2>&1)"; fi
    args="$(staged_args)"
    if has "${args}" vmlab.added=1 && ! has "${args}" vsyscall=none && has "${args}" vmlab.owner=1 && has "${args}" slab_nomerge; then
        ok "the staged update has the image's addition, not its removal, and the owner's argument"
    else
        not_ok "the staged update's arguments" "${args}"
    fi
    check "the helper logged what it changed" sh -c \
        "journalctl -t amethystora-kargs -b --no-pager -o cat | grep -qF '+vmlab.added=1 -vsyscall=none'"
    same "a soft reboot is refused for it" "$(/usr/libexec/amethystora-soft-reboot status | jq -r .why)" kernel
    before="$(sha256sum /run/ostree/staged-deployment)"
    systemctl start amethystora-kargs.service
    same "a second run changes nothing" "$(sha256sum /run/ostree/staged-deployment)" "${before}"

    # Staged again with the path unit masked: only the finalize unit, at shutdown, is left to act
    unstage
    systemctl mask --runtime --now amethystora-kargs.path >/dev/null 2>&1
    out="$(stage)" || not_ok "staging the update again" "${out}"
    sleep 5
    if has "$(staged_args)" vsyscall=none && ! synced; then
        ok "with the path unit masked, the staged update still has the old arguments"
    else
        not_ok "something synced the update with the path unit masked" "$(staged_args)"
    fi
    remember staged "$(bootc_json '.status.staged.ostree.checksum')"
    reboot_and_continue
fi

if [[ "${SUITE_PHASE}" == 2 ]]; then
    same "the restart started the update" "$(bootc_json '.status.booted.ostree.checksum')" "$(recall staged)"
    cmdline=" $(cat /proc/cmdline) "
    if [[ "${cmdline}" == *" vmlab.added=1 "* && "${cmdline}" != *" vsyscall=none "* && "${cmdline}" == *" vmlab.owner=1 "* ]]; then
        ok "the kernel started with the image's arguments and the owner's, synced at shutdown"
    else
        not_ok "the kernel's arguments after the restart" "${cmdline}"
    fi
    check "the finalize unit logged the change" sh -c \
        "journalctl -b -1 -t amethystora-kargs --no-pager -o cat | grep -qF '+vmlab.added=1 -vsyscall=none'"
    out="$(bootc rollback 2>&1)" || not_ok "bootc rollback to the machine's own image" "${out}"
    reboot_and_continue
fi

if [[ "${SUITE_PHASE}" == 3 ]]; then
    same "back on the machine's own image" "$(bootc_json '.status.booted.ostree.checksum')" "$(recall original)"
    check "with its own arguments" grep -qw vsyscall=none /proc/cmdline
    rpm-ostree cleanup -r >/dev/null 2>&1
    rw ostree refs --repo="${REPO}" --delete vmlab >/dev/null 2>&1
    rw ostree prune --repo="${REPO}" --refs-only >/dev/null 2>&1
    if [[ "$(rpm-ostree status --json | jq '.deployments | length')" == "$(recall deployments)" ]]; then
        ok "the machine is left with its deployments as they were"
    else
        not_ok "the machine is not as it was" "$(rpm-ostree status 2>&1 | head -n 20)"
    fi
    rm -rf "${STATE}"
fi
finish
