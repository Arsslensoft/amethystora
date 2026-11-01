# A command this computer lacks: where it can come from, while `ame desktop missing-command` is on (read when fish
# starts; bash's is /etc/profile.d/zz-amethystora-missing-command.sh). It only suggests: nothing is installed.
set -l amethystora_config $HOME/.config
set -q XDG_CONFIG_HOME; and set amethystora_config $XDG_CONFIG_HOME
if status is-interactive; and test -e $amethystora_config/amethystora/missing-command
    function fish_command_not_found
        /usr/libexec/amethystora-missing-command $argv[1] >&2; and return 127
        __fish_default_command_not_found_handler $argv
    end
end
