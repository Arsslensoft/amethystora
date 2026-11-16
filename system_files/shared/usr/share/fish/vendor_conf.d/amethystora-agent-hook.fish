# The shells follow the folder for the AI agent (ame agent hook), as /usr/share/amethystora/agent-hook/hook.sh gives bash and
# zsh: claude's and codex's own variables point at the folder's profile, the prompt says it and the sandbox, and each agent's
# command typed here goes through amethystora-agent-tools agent-run when the folder has a profile or a sandbox rule. Off,
# nothing here runs but the test.
set -l __config $HOME/.config
set -q XDG_CONFIG_HOME; and test -n "$XDG_CONFIG_HOME"; and set __config $XDG_CONFIG_HOME
if status is-interactive; and test -e $__config/amethystora/agent-hook
    function __amethystora_agent_follow --on-variable PWD
        # Our own set and erase lines, each value quoted
        /usr/libexec/amethystora-agent-tools profile env --shell fish --dir "$PWD" 2>/dev/null | source
    end

    function __amethystora_agent_run
        set -l name $argv[1]
        set -e argv[1]
        if set -q AMETHYSTORA_AGENT_SANDBOX_RULE; or set -q AMETHYSTORA_AGENT_PROFILE_PIN; or begin
                set -q AMETHYSTORA_AGENT_PROFILE; and test "$AMETHYSTORA_AGENT_PROFILE" != default
            end
            /usr/libexec/amethystora-agent-tools agent-run --agent $name -- $name $argv
        else
            command $name $argv
        end
    end
    for __agent in claude codex opencode agy copilot cursor-agent
        function $__agent --inherit-variable __agent
            __amethystora_agent_run $__agent $argv
        end
    end

    # This shell on one profile wherever it goes (agent-profile NAME), back to the folder's (agent-profile off), or which (alone)
    function agent-profile
        switch "$argv[1]"
            case ''
                set -q AMETHYSTORA_AGENT_PROFILE; and echo $AMETHYSTORA_AGENT_PROFILE; or echo default
                return
            case off
                set -e AMETHYSTORA_AGENT_PROFILE_PIN
            case '*'
                if test "$argv[1]" != default; and not /usr/libexec/amethystora-agent-tools profile root $argv[1] >/dev/null 2>&1
                    echo "there is no profile $argv[1] (ame agent profile list)" >&2
                    return 1
                end
                set -gx AMETHYSTORA_AGENT_PROFILE_PIN $argv[1]
        end
        __amethystora_agent_follow
    end

    __amethystora_agent_follow
end
