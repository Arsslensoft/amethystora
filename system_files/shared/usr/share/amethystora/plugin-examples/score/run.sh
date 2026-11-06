#!/usr/bin/bash
# A game's score in the pill while it is on. Replace api.example.org with your scores service's host in manifest.json.
# Its settings come on a pipe (descriptor 10), never in the environment: `ame plugin set ID NAME` gives one (a secret goes into the Vault).
# What it prints is one line of JSON, built by jq so that any text in it is escaped. The network is only the hosts its manifest names.
set -euo pipefail
settings='{}'
read -r -t 1 -u 10 settings 2>/dev/null || true
setting() { jq -r --arg k "$1" '.[$k] // empty' <<<"${settings}"; }

game="$(setting game)"
[[ "${game}" =~ ^[A-Za-z0-9_-]{1,40}$ ]] || { jq -nc '{hidden: true, text: "Needs a game"}'; exit 0; }
score="$(curl -fsS --max-time 15 "https://api.example.org/score/${game}")" || { jq -nc '{hidden: true}'; exit 0; }
# Expected: {"home": "Lions", "away": "Bears", "home_score": 2, "away_score": 1, "minute": 63, "live": true}
jq -c '{hidden: (.live | not), text: "\(.home) \(.home_score)-\(.away_score) \(.away)", icon: "applications-games-symbolic",
        card: [{type: "row", label: "Minute", value: (.minute | tostring)}]}' <<<"${score}"
