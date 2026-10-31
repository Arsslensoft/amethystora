# The prompt, as /etc/profile.d/90-amethystora-starship.sh gives bash and /etc/zsh/zshrc gives zsh
if status is-interactive; and type -q starship
    # What `starship init fish` prints, written by the build for this starship (05-override-install.sh)
    if test -r /usr/share/amethystora/starship/init.fish
        source /usr/share/amethystora/starship/init.fish
    else
        starship init fish | source
    end
end
