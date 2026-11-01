#!/usr/bin/bash
# Today's date and the week's number (ISO 8601's, as GNOME's calendar counts them). "interval" is 0 in widget.json:
# this keeps running and prints a line when the day turns, waking every ten minutes to see whether it has.
set -euo pipefail

shown=""
while true; do
    text="$(date '+%A')"$'\n'"$(date '+%-d %B %Y')"$'\n'"Week $(date '+%-V')"
    line="$(jq -nc --arg text "${text}" '{$text, icon: "x-office-calendar-symbolic"}')"
    if [[ "${line}" != "${shown}" ]]; then
        echo "${line}"
        shown="${line}"
    fi
    sleep 600
done
