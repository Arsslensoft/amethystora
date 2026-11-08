# Thèmes

Un thème est une palette, et tout est peint à partir d’elle. Changez de thème et tout le bureau change
ensemble : le style clair ou sombre et la couleur d’accentuation de GNOME, le terminal, l’invite du
shell, la barre supérieure, le dock, le fond d’écran, VSCodium, et ce manuel. Un thème clair donne au
dock une légère teinte sombre, pour que ses icônes ressortent encore sur un fond d’écran pâle.

## Changer de thème

| Touche | Rôle |
| --- | --- |
| `Super+Ctrl+Shift+Space` | Choisir un thème |
| `Super+Ctrl+D` | Basculer entre les versions claire et sombre du thème |
| `Super+Ctrl+Space` | Fond d’écran suivant du thème actuel |

La même chose depuis un terminal :

```bash
ame desktop theme                   # pick one from a list
amethystora-theme list              # the themes, with the current one marked
amethystora-theme set "Tokyo Night" # apply one by name
amethystora-theme toggle            # light or dark
amethystora-theme current           # which one is on
```

## Les thèmes

| Thème | Nom à utiliser | Style |
| --- | --- | --- |
| Amethystora | `amethystora` | Sombre, par défaut |
| Amethystora Light | `amethystora-light` | Clair |
| Catppuccin Mocha | `catppuccin` | Sombre |
| Catppuccin Latte | `catppuccin-latte` | Clair |
| Everforest | `everforest` | Sombre |
| Gruvbox | `gruvbox` | Sombre |
| Matte Black | `matte-black` | Sombre |
| Nord | `nord` | Sombre |
| Rosé Pine Dawn | `rose-pine-dawn` | Clair |
| Tokyo Night | `tokyo-night` | Sombre |

Les deux thèmes Amethystora apportent aussi le thème GTK Amethystora, celui aux trois voyants colorés
dans le coin de chaque fenêtre. Les autres gardent l’apparence propre à GNOME, dans leur couleur
d’accentuation. `Super+Ctrl+D` fait basculer un thème vers son partenaire (Catppuccin Mocha et Latte,
Amethystora et Amethystora Light) ; un thème sans partenaire bascule vers Amethystora dans l’autre
mode.
