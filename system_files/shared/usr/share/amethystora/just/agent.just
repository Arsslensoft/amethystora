# vim: set ft=make :

# No default-list here: `ame agent`, alone, opens the agent as amethystora-agent does (`ame --list agent` lists the group)

# Open the AI agent in a terminal, as Super+Ctrl+Shift+A does (`ame agent`, alone, runs this)
[default]
open:
    @amethystora-agent

# Open the AI agent with a first question
[positional-arguments]
ask +QUESTION:
    @amethystora-agent ask -- "$@"

# Find out why something broke, changing nothing: a process number, a unit, or what went wrong in your words (without one, it asks)
[positional-arguments]
diagnose *WHAT:
    @amethystora-agent diagnose "$@"

# Each AI agent, whether it is installed, and its name
list:
    @amethystora-agent list

# Whether the agentic features are on for your account, and which agent
status:
    @amethystora-agent status

# Search the manual and the image's reference as the agent does (-k N for N passages)
[positional-arguments]
search +ARGS:
    @amethystora-agent search "$@"

# Read a whole passage a search found, by its id
[positional-arguments]
read ID:
    @amethystora-agent read "$1"

# This machine's state as the agent reads it, by topic: overview, health, settings, security, software, updates, backups, disks, units, crashes... (FILTER: only what matches)
[positional-arguments]
state TOPIC *FILTER:
    @amethystora-agent state "$@"

# Turn the AI agent on or off for your account, off until you turn it on: its menu entry, Super+Ctrl+Shift+A, and the skill that tells it about this system (ACTION: on or off; without it, it flips)
[positional-arguments]
toggle ACTION="":
    #!/usr/bin/bash
    case "${1:-}" in
        on | off) amethystora-agent "${1:-}" ;;
        "")
            if amethystora-agent status >/dev/null; then
                amethystora-agent off
            else
                amethystora-agent on
            fi
            ;;
        *)
            echo "usage: ame agent toggle [on|off]"
            exit 64
            ;;
    esac

# Let the AI agent's tools make changes: settings, apps, and commands it hands you, each asked for every time (ACTION: on or off; without it, it flips). Needs the agentic features on
[positional-arguments]
tools ACTION="":
    #!/usr/bin/bash
    case "${1:-}" in
        on | off) amethystora-agent tools "$1" ;;
        "")
            if amethystora-agent tools status >/dev/null; then
                amethystora-agent tools off
            else
                amethystora-agent tools on
            fi
            ;;
        *)
            echo "usage: ame agent tools [on|off]"
            exit 64
            ;;
    esac

# Install the AI agent, where it keeps itself up to date: claude, opencode, codex, agy, copilot or cursor-agent (default: the chosen one)
[positional-arguments]
install AGENT="":
    @amethystora-agent install ${1:+"$1"}

# Choose the AI agent that the menu, Super+Ctrl+Shift+A and Ask the agent open: claude, opencode, codex, agy, copilot or cursor-agent
[positional-arguments]
use AGENT:
    @amethystora-agent use "$1"

# Lock the AI agent, for every account or for one (an administrator's): act lets it know and change things, know only know, off nothing, default puts one account back to everyone's. Without a level, says what it is
[positional-arguments]
lock *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    usage() { echo "usage: ame agent lock [ACCOUNT] act|know|off, or ame agent lock ACCOUNT default"; exit 64; }
    case "$#" in
        0) exec /usr/libexec/amethystora-agent-tools policy ;;
        1) [[ "$1" =~ ^(act|know|off)$ ]] || usage; set -- allow "$1" ;;
        2) [[ "$2" =~ ^(act|know|off|default)$ ]] || usage; set -- allow "$2" "$1" ;;
        *) usage ;;
    esac
    echo "$(gettext "This is an administrator's setting, and asks for an administrator's password.")"
    sudo /usr/libexec/amethystora-agent-tools conf "$@"

# How the AI agent's root commands are allowed, for every account (an administrator's): session (one password, then until the agent ends, the screen locks or an hour passes), minutes (five minutes), each (every time) or terminal (sudo in a terminal). Without one, says which it is
[positional-arguments]
root MODE="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    case "${1:-}" in
        "") exec /usr/libexec/amethystora-agent-tools policy ;;
        session | minutes | each | terminal) ;;
        *) echo "usage: ame agent root session|minutes|each|terminal"; exit 64 ;;
    esac
    echo "$(gettext "This is an administrator's setting, and asks for an administrator's password.")"
    sudo /usr/libexec/amethystora-agent-tools conf root "$1"

# What the AI agent's tools changed, newest first, each with its id (ID: one change's result)
[positional-arguments]
history ID="":
    @amethystora-agent history ${1:+"$1"}

# Undo one change the AI agent's tools made, by its id from ame agent history, as the live pill's Undo does
[positional-arguments]
undo ID:
    @amethystora-agent undo "$1"

# End the AI agent's tools for now, as the live pill's Stop does: the agent can change nothing more until it is started again
stop:
    @amethystora-agent stop

# Super+Shift+A asks the AI agent about the text selected in any window, while the agentic features are on (ACTION: on or off; without it, it flips)
[positional-arguments]
selection ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.agent
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" ask-selection)" == *"'<"* ]]; then state=off; else state=on; fi
    fi
    case "${state}" in
        on)
            gsettings reset "${schema}" ask-selection
            echo "$(gettext "Ask about a selection: on. Select text in any window and press Super+Shift+A.")"
            ;;
        off)
            gsettings set "${schema}" ask-selection "[]"
            echo "$(gettext "Ask about a selection: off.")"
            ;;
        *)
            echo "usage: ame agent selection on|off"
            exit 64
            ;;
    esac

# The live pill lists each change the AI agent's tools make while it runs, each with Undo, and Stop (ACTION: on or off; without it, it flips)
[positional-arguments]
pill ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.agent
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" pill)" == true ]]; then state=off; else state=on; fi
    fi
    case "${state}" in
        on)
            gsettings set "${schema}" pill true
            echo "$(gettext "The agent's changes in the live pill: on.")"
            ;;
        off)
            gsettings set "${schema}" pill false
            echo "$(gettext "The agent's changes in the live pill: off.")"
            ;;
        *)
            echo "usage: ame agent pill on|off"
            exit 64
            ;;
    esac

# Super+/ opens a field in the live pill's card, and the AI agent answers there with the tools that only read, while the agentic features are on (ACTION: on or off; without it, it flips)
[positional-arguments]
ask-here ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.agent
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" ask-here)" == *"'<"* ]]; then state=off; else state=on; fi
    fi
    case "${state}" in
        on)
            gsettings reset "${schema}" ask-here
            echo "$(gettext "Ask the agent in the pill: on. Press Super+/.")"
            ;;
        off)
            gsettings set "${schema}" ask-here "[]"
            echo "$(gettext "Ask the agent in the pill: off.")"
            ;;
        *)
            echo "usage: ame agent ask-here on|off"
            exit 64
            ;;
    esac

# The AI agent's tools ask before they run a command for you as a card in the live pill, with the exact command, Allow and Not now; off, they ask in a terminal (ACTION: on or off; without it, it flips)
[positional-arguments]
ask-questions ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.agent
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" ask-questions)" == true ]]; then state=off; else state=on; fi
    fi
    case "${state}" in
        on)
            gsettings set "${schema}" ask-questions true
            echo "$(gettext "The agent's questions in the pill: on.")"
            ;;
        off)
            gsettings set "${schema}" ask-questions false
            echo "$(gettext "The agent's questions in the pill: off. They ask in a terminal.")"
            ;;
        *)
            echo "usage: ame agent ask-questions on|off"
            exit 64
            ;;
    esac

# The live pill says in a few words which of the AI agent's tools is reading, with a tick when it is done: only the tool's name, never what it read (ACTION: on or off; without it, it flips)
[positional-arguments]
doing ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.agent
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" doing)" == true ]]; then state=off; else state=on; fi
    fi
    case "${state}" in
        on)
            gsettings set "${schema}" doing true
            echo "$(gettext "What the agent is reading, in the pill: on.")"
            ;;
        off)
            gsettings set "${schema}" doing false
            echo "$(gettext "What the agent is reading, in the pill: off.")"
            ;;
        *)
            echo "usage: ame agent doing on|off"
            exit 64
            ;;
    esac

# Ask the AI agent about a picture or a text file: drop it on the pill's Ask the agent, or copy a picture and press Super+Shift+A. It shows what will be sent, and to whom, before anything is (ACTION: on or off; without it, it flips)
[positional-arguments]
ask-about ACTION="":
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.agent
    state="${1:-}"
    if [[ -z "${state}" ]]; then
        if [[ "$(gsettings get "${schema}" ask-about)" == true ]]; then state=off; else state=on; fi
    fi
    case "${state}" in
        on)
            echo "$(gettext "A picture or text file you ask about goes to the maker of your agent, with what is on it. It shows you that first, every time, and never while your screen is shared.")"
            gsettings set "${schema}" ask-about true
            echo "$(gettext "Ask about a picture or a file: on.")"
            ;;
        off)
            gsettings set "${schema}" ask-about false
            echo "$(gettext "Ask about a picture or a file: off.")"
            ;;
        *)
            echo "usage: ame agent ask-about on|off"
            exit 64
            ;;
    esac

# Little routines: "when I join the office Wi-Fi, open Slack and mute", one readable file each, sealed in the Vault (VERB: list, show ID, check FILE, add FILE, pause ID, resume ID, remove ID, run ID, undo ID, on, off)
[positional-arguments]
routines VERB="list" *ARGS:
    #!/usr/bin/bash
    set -euo pipefail
    # shellcheck source=/dev/null
    source gettext.sh
    schema=org.gnome.shell.extensions.amethystora-ui.agent
    case "${1:-list}" in
        list | show | check | add | pause | resume | remove | run | undo) exec amethystora-agent routines "$@" ;;
        on)
            gsettings set "${schema}" routines true
            echo "$(gettext "Routines: on.")"
            ;;
        off)
            gsettings set "${schema}" routines false
            echo "$(gettext "Routines: off. None runs.")"
            ;;
        *)
            echo "usage: ame agent routines list | show ID | check FILE | add FILE | pause|resume|remove|run|undo ID | on|off"
            exit 64
            ;;
    esac
