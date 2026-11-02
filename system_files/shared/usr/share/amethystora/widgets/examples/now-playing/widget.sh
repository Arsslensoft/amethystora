#!/usr/bin/bash
# What a player plays now, from the interface every player puts on the session bus (MPRIS), with its buttons in the
# menu: the one playing, else the one paused. Hidden while no player is open. "./widget.sh play-pause", "next" and
# "previous" are what the menu runs; they ask the same player again first. Titles are the player's, shown as text.
set -euo pipefail

MPRIS=/org/mpris/MediaPlayer2
PLAYER=org.mpris.MediaPlayer2.Player

# The players on the bus, by their names
players() {
    busctl --user call --json=short org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus ListNames 2>/dev/null |
        jq -r '.data[0][] | select(startswith("org.mpris.MediaPlayer2."))'
}

property() {
    busctl --user get-property --json=short "$1" "${MPRIS}" "${PLAYER}" "$2" 2>/dev/null
}

# The player playing, else the first one paused
chosen=""
paused=""
while read -r name; do
    [[ -n "${name}" ]] || continue
    case "$(property "${name}" PlaybackStatus | jq -r '.data // empty')" in
        Playing) chosen="${name}"; break ;;
        Paused) [[ -n "${paused}" ]] || paused="${name}" ;;
    esac
done < <(players)
chosen="${chosen:-${paused}}"

case "${1:-}" in
    play-pause | next | previous)
        [[ -n "${chosen}" ]] || exit 0
        method=PlayPause
        [[ "$1" == next ]] && method=Next
        [[ "$1" == previous ]] && method=Previous
        busctl --user call "${chosen}" "${MPRIS}" "${PLAYER}" "${method}" >/dev/null 2>&1 || true
        exit 0
        ;;
esac

if [[ -z "${chosen}" ]]; then
    echo '{"hidden": true}'
    exit 0
fi

status="$(property "${chosen}" PlaybackStatus | jq -r '.data // "Stopped"')"
property "${chosen}" Metadata | jq -c --arg status "${status}" '
    (.data["xesam:title"].data // "") as $title |
    ((.data["xesam:artist"].data // []) | join(", ")) as $artist |
    {
        text: (if $title == "" then "Playing" else ($title | if length > 40 then .[0:39] + "…" else . end) end),
        icon: (if $status == "Playing" then "media-playback-start-symbolic" else "media-playback-pause-symbolic" end),
        click: "./widget.sh play-pause",
        menu: ([if $artist != "" then {label: $artist} else empty end] + [
            {label: (if $status == "Playing" then "Pause" else "Play" end), icon: (if $status == "Playing" then "media-playback-pause-symbolic" else "media-playback-start-symbolic" end), run: "./widget.sh play-pause"},
            {label: "Next", icon: "media-skip-forward-symbolic", run: "./widget.sh next"},
            {label: "Previous", icon: "media-skip-backward-symbolic", run: "./widget.sh previous"}
        ])
    }' || echo '{"hidden": true}'
