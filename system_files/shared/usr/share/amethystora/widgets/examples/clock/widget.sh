#!/usr/bin/bash
# The time in big type, with the day under it, as GNOME's clock is set to show it (24 or 12 hours). "interval" is 0
# in widget.json: this keeps running and prints a line when the minute turns, waking at most every ten seconds so
# that it is right again soon after the computer wakes from sleep. It prints nothing else and reads no file.
set -euo pipefail

shown=""
while true; do
    if [[ "$(gsettings get org.gnome.desktop.interface clock-format 2>/dev/null)" == "'12h'" ]]; then
        time="$(date '+%-I:%M %p')"
    else
        time="$(date '+%H:%M')"
    fi
    line="$(jq -nc --arg text "${time}"$'\n'"$(date '+%A %-d %B')" '{$text, icon: "preferences-system-time-symbolic"}')"
    if [[ "${line}" != "${shown}" ]]; then
        echo "${line}"
        shown="${line}"
    fi
    left=$((60 - 10#$(date +%S)))
    sleep $((left < 10 ? left : 10))
done
