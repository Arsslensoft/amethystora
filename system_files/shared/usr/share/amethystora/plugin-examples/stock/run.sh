#!/usr/bin/bash
# A price in the top bar and in search. Replace quotes.example.org with your quotes service's host in manifest.json.
# Its settings come on a pipe (descriptor 10), never in the environment: `ame plugin set ID NAME` gives one (a secret goes into the Vault).
# What it prints is one line of JSON, built by jq so that any text in it is escaped. The network is only the hosts its manifest names.
set -euo pipefail
settings='{}'
read -r -t 1 -u 10 settings 2>/dev/null || true
setting() { jq -r --arg k "$1" '.[$k] // empty' <<<"${settings}"; }

symbol="$(setting symbol)"
[[ "${symbol}" =~ ^[A-Za-z0-9.-]{1,12}$ ]] || { jq -nc '{text: "Needs a symbol"}'; exit 0; }
quote="$(curl -fsS --max-time 15 "https://quotes.example.org/quote/${symbol}")" || { jq -nc '{hidden: true}'; exit 0; }
# Expected: {"price": 123.45, "change": -0.8}
jq -c --arg symbol "${symbol^^}" '{text: "\($symbol) \(.price)", icon: "emblem-money-symbolic", class: (if .change < 0 then "critical" else "good" end),
    menu: [{label: "Change today: \(.change)%"}]}' <<<"${quote}"
