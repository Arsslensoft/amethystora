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

# The plugins the image ships (add one with: ame plugin add ID, which needs a click and no administrator)
shipped:
    @/usr/libexec/amethystora-plugins shipped

# Ask the AI agent to make a plugin from your words ("show my GitHub build in the pill"): it appears as a draft that runs with nothing allowed, and you keep it, change it or throw it away
[positional-arguments]
make +REQUEST:
    @/usr/libexec/amethystora-plugins make "$@"

# Show a draft folder as a draft: it is checked, marked as the agent's, and runs with nothing allowed (the agent runs this when it is done; --by me for your own)
[positional-arguments]
draft ID *ARGS:
    @/usr/libexec/amethystora-plugins draft "$@"

# Keep a draft as a plugin: you read what it asks for and allow each part, as for any plugin, and an administrator approves
[positional-arguments]
keep ID:
    @/usr/libexec/amethystora-plugins keep "$@"

# Throw a draft away, with what it kept
[positional-arguments]
discard ID:
    @/usr/libexec/amethystora-plugins discard "$@"

# Ask the AI agent to change a draft
[positional-arguments]
change ID:
    @/usr/libexec/amethystora-plugins change "$@"

# Ask the AI agent to find out why a plugin paused itself and fix it
[positional-arguments]
fix ID:
    @/usr/libexec/amethystora-plugins fix "$@"

# What a plugin asks you to give it (a name, or a secret such as a token), and whether you did (--json for a program)
[positional-arguments]
settings ID *ARGS:
    @/usr/libexec/amethystora-plugins settings "$@"

# Give a plugin one of its settings; what you type is not shown, and a secret goes into the Vault and nowhere in the clear
[positional-arguments]
set ID NAME:
    @/usr/libexec/amethystora-plugins set "$@"

# Open a .amethystora-pill file: you read what it runs and reaches, then keep it or not
[positional-arguments]
open FILE:
    @/usr/libexec/amethystora-plugins open "$@"

# The examples to start from: the image's own plugins, and a few that reach a service (a build's status, a door) for you to give your own address and token
examples:
    @/usr/libexec/amethystora-plugins examples

# Start a draft from an example under a name of your own, to change its files (it runs with nothing allowed until you keep it)
[positional-arguments]
new ID EXAMPLE="hello":
    @/usr/libexec/amethystora-plugins new "$@"

# Run a plugin or a draft once in its sandbox and print what the shell would be given (ACTION: as a click on one of its own buttons)
[positional-arguments]
try ID *ACTION:
    @/usr/libexec/amethystora-plugins try "$@"

# Remove everything the AI agent made, plugins and drafts, with what they kept; asks first
remove-agent-made:
    @/usr/libexec/amethystora-plugins remove-agent-made
