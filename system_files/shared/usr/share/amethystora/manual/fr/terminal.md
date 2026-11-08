# Le terminal

Amethystora est fait pour les personnes qui aiment la ligne de commande autant que pour celles qui ne
l’ouvrent jamais. Quand vous l’ouvrez, il est prêt.

## En ouvrir un

`Super+Return` ouvre un terminal, tout comme `Ctrl+Alt+T` et `Ctrl+Alt+Return`. Le terminal est
Ptyxis. Il connaît les conteneurs : le menu à côté du bouton de nouvel onglet ouvre un onglet sur
l’hôte ou à l’intérieur de n’importe lequel de vos
[conteneurs Distrobox et Toolbox](containers.md).

Ses couleurs suivent le [thème](themes.md), tout comme l’invite.

## L’accueil

Un nouveau terminal vous salue avec le logo d’Amethystora, un résumé de la machine, l’image que vous
utilisez, quelques commandes utiles et une astuce. `ame desktop greeting` le désactive, puis le
réactive.

`fetch` affiche de nouveau le logo et le résumé du système quand vous le souhaitez. Un reflet traverse
les facettes du logo avant que le résumé n’apparaisse ; vous obtenez le logo fixe en SSH, dans la
console texte, dans un terminal sans couleurs complètes, avec `NO_COLOR` défini, ou quand les
animations sont désactivées dans **Paramètres → Accessibilité**. Tout ce que vous ajoutez après `fetch` est
transmis à fastfetch, qu’il exécute.

## Shells

Le shell est bash, avec l’invite Starship. fish et zsh sont installés aussi. Le changement se fait dans
le terminal plutôt qu’à l’échelle du système, pour qu’une configuration de shell cassée ne puisse
jamais vous empêcher de vous connecter :

1. Ouvrez les **Préférences** du terminal et modifiez votre profil.
2. Activez **Utiliser une commande personnalisée** et saisissez `/usr/bin/fish` ou `/usr/bin/zsh`.

## ame

`ame` exécute les commandes fournies avec le système : de petits scripts testés pour les tâches qui
demanderaient sinon une recherche sur le Web, regroupés par thème comme `desktop`, `security` et
`system`. Ce manuel les mentionne là où elles sont utiles ; pour les voir toutes :

```bash
ame                           # the groups, and the commands outside them
ame security                  # the commands in one group, with a line about what each does
ame --list --list-submodules  # every command, in every group
ame -n security status        # print what a command would run, without running it
```

`ame pkg` est [amethystora-pkg](containers.md#packages-from-other-distributions). `ujust`, l’ancien nom
d’`ame`, exécute les mêmes commandes, et leurs noms d’avant le regroupement, comme
`ujust setup-backup`, fonctionnent toujours.

Les commandes sont des recettes `just`, et `just` lui-même est à votre disposition pour vos propres
projets : un `justfile` dans n’importe quel dossier transforme ses commandes en recettes de la même
façon.

## Outils en ligne de commande

Les outils en ligne de commande viennent de Homebrew, qui s’installe dans votre dossier personnel sans
`sudo` et sans toucher au système :

```bash
brew install ripgrep
brew search <name>
brew upgrade
```

Parcourez ce qui existe sur [formulae.brew.sh](https://formulae.brew.sh). Homebrew est mis à jour avec
le système. Ne l’exécutez jamais avec `sudo` : il est à vous, pas à root.

Déjà dans l’image : `just`, `gum`, `glow`, `tmux`, `fastfetch`, `git` et le reste des outils
habituels. Les polices comprennent Inter, JetBrains Mono et les symboles Nerd Fonts, pour que les
invites et les gestionnaires de fichiers en terminal affichent leurs icônes.
