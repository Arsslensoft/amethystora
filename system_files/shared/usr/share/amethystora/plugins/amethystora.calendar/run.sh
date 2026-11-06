#!/usr/bin/bash
# Today's date and the week's number (ISO 8601's, as GNOME's calendar counts them), on the desktop. It reads nothing and reaches nothing.
set -euo pipefail

text="$(date '+%A')"$'\n'"$(date '+%-d %B %Y')"$'\n'"Week $(date '+%-V')"
jq -nc --arg text "${text}" '{$text, icon: "x-office-calendar-symbolic"}'
