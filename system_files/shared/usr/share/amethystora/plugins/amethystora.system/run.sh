#!/usr/bin/bash
# How busy the processor is, how much memory is in use and how full the disk is, every five seconds. The processor is the share of the time
# it was busy since the last run, worked out from /proc/stat and the numbers the last run kept in $PLUGIN_STATE.
set -euo pipefail

STATE="${PLUGIN_STATE}/cpu"

read -r _ user nice system idle iowait irq softirq steal _ </proc/stat
total=$((user + nice + system + idle + iowait + irq + softirq + steal))
waiting=$((idle + iowait))
cpu=0
if [[ -s ${STATE} ]]; then
    read -r last_total last_waiting <"${STATE}" || true
    span=$((total - ${last_total:-0}))
    ((span > 0)) && cpu=$(((100 * (span - (waiting - ${last_waiting:-0}))) / span))
fi
echo "${total} ${waiting}" >"${STATE}"
((cpu < 0)) && cpu=0

memory="$(awk '/^MemTotal:/ { t = $2 } /^MemAvailable:/ { a = $2 } END { if (t) printf "%d %.1f", (t - a) * 100 / t, t / 1048576 }' /proc/meminfo)"
read -r used size <<<"${memory}"
disk="$(plugin state disk 2>/dev/null | jq -r '.used_percent // "?"' 2>/dev/null || echo '?')"

class=""
((cpu >= 90 || used >= 90)) && class=warning
[[ "${disk}" =~ ^[0-9]+$ ]] && ((disk >= 95)) && class=warning
jq -nc --arg cpu "${cpu}" --arg used "${used}" --arg size "${size}" --arg disk "${disk}" --arg class "${class}" '{
    text: "\($cpu)% · \($used)%",
    icon: "utilities-system-monitor-symbolic",
    class: $class,
    menu: [
        {label: "Processor \($cpu)% busy"},
        {label: "Memory \($used)% of \($size) GB in use"},
        {label: "Disk \($disk)% full"}
    ]
}'
