#!/usr/bin/bash
# A 25-minute focus timer in the pill: its card, its tile in quick settings and "pomodoro" in search start and stop it. The time it ends at is a file in $PLUGIN_STATE, and the countdown is
# counted by the shell (`until`), so nothing runs between the host's ticks; one tick sees that the time is over and says so, once.
set -euo pipefail

MINUTES=25
END="${PLUGIN_STATE}/end"

case "${PLUGIN_ACTION:-}" in
    start) echo $(($(date +%s) + MINUTES * 60)) >"${END}" ;;
    stop) rm -f "${END}" ;;
esac

left=0
[[ -s ${END} ]] && left=$(($(<"${END}") - $(date +%s)))
if ((left <= 0)); then
    if [[ -e ${END} ]]; then
        rm -f "${END}"
        plugin notify "Time for a break" "That was ${MINUTES} minutes of focus." >/dev/null 2>&1 || true
    fi
    # Nothing in the pill while it is idle ("hidden"); the tile and the search answer stay, since they were asked for
    echo '{"hidden": true, "text": "Start a focus timer", "icon": "alarm-symbolic", "click": "start", "card": [{"type": "switch", "label": "Focus timer", "on": false, "action": "start"}]}'
    exit 0
fi
total=$((MINUTES * 60))
jq -nc --argjson until "$(<"${END}")" --argjson progress "$(awk -v l="${left}" -v t="${total}" 'BEGIN { printf "%.3f", 1 - l / t }')" '{
    text: "Focus", icon: "alarm-symbolic", class: "accent", until: $until, progress: $progress,
    card: [{type: "text", text: "Twenty-five minutes of focus."}, {type: "switch", label: "Focus timer", on: true, action: "stop"}]
}'
