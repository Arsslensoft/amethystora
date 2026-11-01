#!/usr/bin/env bash

source /usr/lib/amethystora/setup-services/libsetup.sh

version-script installer-ssh system 1 || exit 0

set -x

# The live ISO's Anaconda ran `firewall-offline-cmd --enabled --service=ssh` (iso/live/installer.ks no longer lets
# it), which wrote a copy of the Amethystora zone into /etc with ssh added. Nobody chose that, and the report calls
# it drift. Taken away once, and only while sshd is not enabled: whoever turned sshd on may be relying on the
# port. A zone with anything else of the owner's in it keeps that. AMETHYSTORA_FIREWALL_ROOT and AMETHYSTORA_SSHD_ENABLED
# are for 20-tests.sh, which runs this against stand-ins.
ROOT="${AMETHYSTORA_FIREWALL_ROOT:-}"
ZONE="${ROOT}/etc/firewalld/zones/amethystora.xml"
IMAGE="${ROOT}/usr/lib/firewalld/zones/amethystora.xml"
[[ -f "${ZONE}" && -f "${IMAGE}" ]] || exit 0
grep -q '<service name="ssh"/>' "${ZONE}" || exit 0
if [[ -n "${AMETHYSTORA_SSHD_ENABLED:-}" ]]; then
    [[ "${AMETHYSTORA_SSHD_ENABLED}" == no ]] || exit 0
else
    ! systemctl is-enabled --quiet sshd.service || exit 0
fi

# What a zone lets in, one sorted line each, without the words around it
allowed() { grep -E '<(service|port|protocol|rule|source|interface|masquerade|forward-port|icmp-block|source-port)\b' "$1" | sed 's/^ *//' | sort; }
if [[ "$(allowed "${ZONE}" | grep -vxF '<service name="ssh"/>')" == "$(allowed "${IMAGE}")" ]]; then
    rm -f "${ZONE}" "${ZONE}.old"
else
    sed -i '/<service name="ssh"\/>/d' "${ZONE}"
fi
[[ -n "${ROOT}" ]] || ! systemctl is-active --quiet firewalld.service || firewall-cmd --reload
