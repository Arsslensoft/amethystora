#!/usr/bin/bash
# A starting point: how long the computer has been on, and a menu. The host runs this every "interval" seconds (manifest.json) in a sandbox
# and shows the last line it prints: one JSON object, built by jq -c so that any text in it is escaped properly.
set -euo pipefail

up="$(uptime -p | sed -e 's/^up //' -e 's/ days\?/d/' -e 's/ hours\?/h/' -e 's/ minutes\?/m/' -e 's/,//g')"

jq -cn --arg up "${up}" '{
    text: $up,
    icon: "computer-symbolic",
    menu: [
        {label: "On for \($up)"}
    ]
}'
