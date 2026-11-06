#!/usr/bin/bash
# What a player plays now, with its buttons in the menu: the one playing, else the one paused. Hidden while no player is open. The buttons are
# actions: the host runs this again with PLUGIN_ACTION set, after a click, which is the only time `plugin control` is allowed.
set -euo pipefail

case "${PLUGIN_ACTION:-}" in
    play-pause | next | previous) plugin control media "${PLUGIN_ACTION}" >/dev/null 2>&1 || true ;;
esac

media="$(plugin state media 2>/dev/null || echo null)"
if [[ -z "${media}" || "${media}" == null ]]; then
    echo '{"hidden": true}'
    exit 0
fi
jq -c '
    (.status // "Stopped") as $status |
    (.title // "") as $title |
    (.artist // "") as $artist |
    {
        text: (if $title == "" then "Playing" else ($title | if length > 40 then .[0:39] + "…" else . end) end),
        icon: (if $status == "Playing" then "media-playback-start-symbolic" else "media-playback-pause-symbolic" end),
        click: "play-pause",
        menu: ([if $artist != "" then {label: $artist} else empty end] + [
            {label: (if $status == "Playing" then "Pause" else "Play" end), action: "play-pause"},
            {label: "Next", icon: "media-skip-forward-symbolic", action: "next"},
            {label: "Previous", icon: "media-skip-backward-symbolic", action: "previous"}
        ])
    }' <<<"${media}"
