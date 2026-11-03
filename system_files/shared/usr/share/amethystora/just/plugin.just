# vim: set ft=make :

set default-list

# The plugins you added, each with its version and whether it is on (--json for a program)
[positional-arguments]
list *ARGS:
    @/usr/libexec/amethystora-plugins list "$@"

# Show one plugin: its files, what it was allowed and what it asked for, what is blocked, and the sandbox it runs in (--json for a program)
[positional-arguments]
show ID *ARGS:
    @/usr/libexec/amethystora-plugins show "$@"

# Add a plugin from an https git address, a folder or a .amethystora-plugin file, to read what it asks for and to allow each part you choose, or not: nothing of it runs while you read (--digest SHA256 refuses any other files)
[positional-arguments]
add SOURCE *ARGS:
    @/usr/libexec/amethystora-plugins add "$@"

# Bring a plugin you added up to date from its source: what is new in it and in what it asks is shown first, and an administrator approves
[positional-arguments]
update ID SOURCE:
    @/usr/libexec/amethystora-plugins update "$@"

# Remove a plugin, with the files it kept
[positional-arguments]
remove ID:
    @/usr/libexec/amethystora-plugins remove "$@"

# Check a plugin's folder as adding it would, and say what its author should know: nothing of it runs (--json for a program)
[positional-arguments]
check FOLDER *ARGS:
    @/usr/libexec/amethystora-plugins check "$@"

# Compare every file of the plugins you added with what you approved (ID: one plugin; --json is not offered)
[positional-arguments]
verify *ID:
    @/usr/libexec/amethystora-plugins verify "$@"

# What was added, allowed, blocked and removed, and when (ID: one plugin; --json for a program)
[positional-arguments]
history *ARGS:
    @/usr/libexec/amethystora-plugins history "$@"

# Go back to the version of a plugin from before its last update, with no more allowed than you allowed it (an administrator approves)
[positional-arguments]
rollback ID:
    @/usr/libexec/amethystora-plugins rollback "$@"

# Turn a plugin on: it runs in its sandbox, within what you allowed
[positional-arguments]
on ID:
    @/usr/libexec/amethystora-plugins on "$@"

# Turn a plugin off: nothing of it runs
[positional-arguments]
off ID:
    @/usr/libexec/amethystora-plugins off "$@"

# Allow more of what a plugin asked for, one permission at a time; this asks in a terminal and an administrator approves (PERMISSION: only these)
[positional-arguments]
grant ID *PERMISSION:
    @/usr/libexec/amethystora-plugins grant "$@"

# Take permissions away from a plugin (all of them without PERMISSION)
[positional-arguments]
revoke ID *PERMISSION:
    @/usr/libexec/amethystora-plugins revoke "$@"

# Block what a plugin sends, to HOST or to every host: it applies at once and cuts what is open
[positional-arguments]
block ID *HOST:
    @/usr/libexec/amethystora-plugins block "$@"

# Let a blocked plugin send again, to HOST or to every host (an administrator approves)
[positional-arguments]
unblock ID *HOST:
    @/usr/libexec/amethystora-plugins unblock "$@"
