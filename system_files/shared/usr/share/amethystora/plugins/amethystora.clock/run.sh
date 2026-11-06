#!/usr/bin/bash
# The time with the day under it, as GNOME's clock is set to show it (24 or 12 hours, which `plugin state clock` says), on the desktop.
set -euo pipefail

format="$(plugin state clock 2>/dev/null | jq -r '.format // "24h"' 2>/dev/null || echo 24h)"
if [[ "${format}" == 12h ]]; then
    time="$(date '+%-I:%M %p')"
else
    time="$(date '+%H:%M')"
fi
jq -nc --arg text "${time}"$'\n'"$(date '+%A %-d %B')" '{$text, icon: "preferences-system-time-symbolic"}'
