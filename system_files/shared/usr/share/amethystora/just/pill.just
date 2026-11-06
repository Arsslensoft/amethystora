# vim: set ft=make :

set default-list

# Make a script live in the pill, with no folder and no setup: it is there until the script ends, even if the script forgets to end it. Say what it shows with --text, --progress 0..1, --until EPOCH or +SECONDS, --icon NAME and --private (hidden while your screen is shared); a plugin or a call always comes first
[positional-arguments]
start NAME *OPTIONS:
    @/usr/libexec/amethystora-pill start "$@"

# Change what a pill shows (only what you name changes)
[positional-arguments]
update NAME *OPTIONS:
    @/usr/libexec/amethystora-pill update "$@"

# Take a pill away
[positional-arguments]
end NAME:
    @/usr/libexec/amethystora-pill end "$@"
