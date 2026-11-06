#!/usr/bin/bash
# Home Assistant's front door sensor, through its cloud address. The token is a secret: it lives in the Vault, and reaches this script on a pipe.
# Its settings come on a pipe (descriptor 10), never in the environment: `ame plugin set ID NAME` gives one (a secret goes into the Vault).
# What it prints is one line of JSON, built by jq so that any text in it is escaped. The network is only the hosts its manifest names.
set -euo pipefail
settings='{}'
read -r -t 1 -u 10 settings 2>/dev/null || true
setting() { jq -r --arg k "$1" '.[$k] // empty' <<<"${settings}"; }

instance="$(setting instance)"
token="$(setting token)"
if [[ ! "${instance}" =~ ^[a-z0-9]+\.ui\.nabu\.casa$ || -z "${token}" ]]; then
    jq -nc '{hidden: true, text: "Needs its address and a token"}'
    exit 0
fi
state="$(curl -fsS --max-time 15 --config <(printf 'header = "Authorization: Bearer %s"\n' "${token}") "https://${instance}/api/states/binary_sensor.front_door" | jq -r '.state')" || { jq -nc '{hidden: true}'; exit 0; }
if [[ "${state}" == on ]]; then
    jq -nc '{text: "Front door open", icon: "changes-allow-symbolic", class: "warning", card: [{type: "switch", label: "Door open", on: true}]}'
else
    jq -nc '{hidden: true, text: "Front door closed", icon: "changes-prevent-symbolic", card: [{type: "switch", label: "Door open", on: false}]}'
fi
