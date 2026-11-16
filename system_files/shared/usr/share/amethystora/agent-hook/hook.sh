# shellcheck shell=bash
# The shells follow the folder for the AI agent (ame agent hook, plans/PLAN-agent-accounts.md and PLAN-agent-sandbox.md):
# each time the folder changes, claude's and codex's own variables point at the folder's profile, the prompt says the
# profile and whether the folder is sandboxed, and each agent's command typed here goes through amethystora-agent-tools
# agent-run when the folder has a profile or a sandbox rule. A convenience, not a lock: `command claude` goes round it.
# Off, nothing here runs but the test below. Sourced by /etc/profile.d/92-amethystora-agent-hook.sh (bash) and by
# /etc/zsh/zshrc (zsh); fish has hook.fish.
[[ $- == *i* && -e ${XDG_CONFIG_HOME:-${HOME}/.config}/amethystora/agent-hook ]] || return 0

__amethystora_agent_dir=
__amethystora_agent_follow() {
    [[ ${PWD} != "${__amethystora_agent_dir}" ]] || return 0
    __amethystora_agent_dir="${PWD}"
    # What it prints is our own export and unset lines, each value quoted
    eval "$(/usr/libexec/amethystora-agent-tools profile env --shell bash --dir "${PWD}" 2>/dev/null)"
}

# An agent typed here: through the folder's profile and sandbox when it has either, as it is otherwise
__amethystora_agent_run() {
    local name="$1"
    shift
    if [[ -n ${AMETHYSTORA_AGENT_SANDBOX_RULE:-}${AMETHYSTORA_AGENT_PROFILE_PIN:-} || ${AMETHYSTORA_AGENT_PROFILE:-default} != default ]]; then
        /usr/libexec/amethystora-agent-tools agent-run --agent "${name}" -- "${name}" "$@"
    else
        command "${name}" "$@"
    fi
}
claude() { __amethystora_agent_run claude "$@"; }
codex() { __amethystora_agent_run codex "$@"; }
opencode() { __amethystora_agent_run opencode "$@"; }
agy() { __amethystora_agent_run agy "$@"; }
copilot() { __amethystora_agent_run copilot "$@"; }
cursor-agent() { __amethystora_agent_run cursor-agent "$@"; }

# This shell on one profile wherever it goes (agent-profile NAME), back to the folder's (agent-profile off), or which (alone)
agent-profile() {
    case "${1:-}" in
        "") echo "${AMETHYSTORA_AGENT_PROFILE:-default}" ;;
        off) unset AMETHYSTORA_AGENT_PROFILE_PIN ;;
        *)
            if [[ $1 != default ]] && ! /usr/libexec/amethystora-agent-tools profile root "$1" >/dev/null 2>&1; then
                echo "there is no profile $1 (ame agent profile list)" >&2
                return 1
            fi
            export AMETHYSTORA_AGENT_PROFILE_PIN="$1"
            ;;
    esac
    __amethystora_agent_dir=
    __amethystora_agent_follow
}

if [[ -n ${BASH_VERSION:-} ]]; then
    # First, so that the prompt (starship) sees the folder's profile
    PROMPT_COMMAND=(__amethystora_agent_follow "${PROMPT_COMMAND[@]}")
elif [[ -n ${ZSH_VERSION:-} ]]; then
    autoload -Uz add-zsh-hook
    add-zsh-hook chpwd __amethystora_agent_follow
fi
__amethystora_agent_follow
