#!/usr/bin/bash
# Do Not Disturb, shown only while notifications are silenced, with a switch that brings them back. The click is an action ("toggle"): the host
# runs this again with PLUGIN_ACTION set, and only then may it change the setting.
set -euo pipefail

on() {
    [[ "$(plugin state do-not-disturb 2>/dev/null | jq -r '.on // false' 2>/dev/null)" == true ]]
}

if [[ "${PLUGIN_ACTION:-}" == toggle ]]; then
    if on; then plugin control do-not-disturb off; else plugin control do-not-disturb on; fi
fi

if on; then
    echo '{"icon": "notifications-disabled-symbolic", "class": "warning", "menu": [{"label": "Bring notifications back", "action": "toggle"}]}'
else
    echo '{"hidden": true}'
fi
