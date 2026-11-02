#!/usr/bin/bash
# The machine as it first boots: everything started, Secure Boot and lockdown on, the TPM there, the image's
# own kernel modules loading under Secure Boot (make-disk.sh enrolls its certificates, as an owner would),
# and nothing crashed.
# vmlab-paths: @sf/usr/lib/systemd/* @sf/etc/systemd/* @sf/usr/lib/bootc/* @sf/usr/lib/modprobe.d/* @sf/usr/lib/sysctl.d/* apps/sensor/* @sf/usr/libexec/amethystora-start-times
source "$(dirname "$0")/lib.sh"

state="$(systemctl is-system-running --wait 2>/dev/null)"
if [[ "${state}" == running ]]; then
    ok "every unit started"
else
    failed="$(systemctl --failed --no-legend --plain | awk '{print $1}')"
    not_ok "the system is ${state}: $(paste -sd ' ' <<<"${failed}")" \
        "$(for unit in ${failed}; do journalctl -b -u "${unit}" -p warning --no-pager -o cat | tail -n 3; done)"
fi
check "Secure Boot is on" sh -c 'mokutil --sb-state | grep -q "SecureBoot enabled"'
check "the kernel is locked down" grep -qE '\[(integrity|confidentiality)\]' /sys/kernel/security/lockdown
check "a TPM 2.0 is there" test -c /dev/tpmrm0
check "the booted deployment is the image under test" sh -c 'rpm-ostree status --booted --json | jq -e ".deployments[0].\"container-image-reference\" | test(\"amethystora\")"'
check "the image's own kernel module loads under Secure Boot (v4l2loopback)" modprobe v4l2loopback
# The audit rules are written at boot by a unit whose sandbox, when systemd cannot set it up, skips it quietly
check "the audit rules were written at boot" systemctl is-active --quiet amethystora-audit-rules.service
# Identity is watched by an audit rule, or by the file-watch sensor (plans/done/PLAN-audit-cost.md),
# in which case the plan gives it the key and auditctl shows only the -a never,task filter.
check "identity is watched (audit rule or the sensor)" sh -c 'auditctl -l | grep -q -- "-k identity\$" ||
    { grep -qx "identity sensor" /run/amethystora/sensor/plan && systemctl is-active --quiet amethystora-sensor.service; }'
check "no per-syscall audit rule loaded when the sensor covers identity" sh -c '! grep -qx "identity sensor" /run/amethystora/sensor/plan ||
    { auditctl -l | grep -qE -- "(never,task|task,never)" && ! auditctl -l | grep -qE -- "^-[wa] .*(-k |key=)"; }'
# kargs.d put it there; the system setup's step for machines rpm-ostree updates ran, found it and did nothing
check "the console is not asked its size at boot" grep -qwF systemd.tty.term.console=dumb /proc/cmdline
check "the system setup ran its console step" wait_for 60 jq -e '.version.system."console-query" == "1"' \
    /root/.local/share/amethystora/setup_versioning.json
dumps="$(coredumpctl list --no-legend --no-pager 2>/dev/null)"
if [[ -z "${dumps}" ]]; then ok "nothing crashed"; else not_ok "something crashed" "${dumps}"; fi
finish
