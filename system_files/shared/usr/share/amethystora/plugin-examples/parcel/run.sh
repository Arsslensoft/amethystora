#!/usr/bin/bash
# A parcel on its way: where it is and when it is due, counted down by the shell. Replace api.example.org with your carrier's host in manifest.json, and the path below with its tracking address.
# Its settings come on a pipe (descriptor 10), never in the environment: `ame plugin set ID NAME` gives one (a secret goes into the Vault).
# What it prints is one line of JSON, built by jq so that any text in it is escaped. The network is only the hosts its manifest names.
set -euo pipefail
settings='{}'
read -r -t 1 -u 10 settings 2>/dev/null || true
setting() { jq -r --arg k "$1" '.[$k] // empty' <<<"${settings}"; }

number="$(setting number)"
if [[ ! "${number}" =~ ^[A-Za-z0-9-]{4,40}$ ]]; then
    jq -nc '{hidden: true, text: "Needs a tracking number"}'
    exit 0
fi
# The answer is expected as {"status": "...", "eta": 1790000000, "progress": 0.4}: change the filter to what the carrier sends
info="$(curl -fsS --max-time 15 "https://api.example.org/track/${number}")" || { jq -nc '{hidden: true}'; exit 0; }
jq -c '{text: .status, icon: "mail-send-symbolic", until: .eta, progress: .progress, card: [{type: "row", label: "Status", value: .status}]}' <<<"${info}"
