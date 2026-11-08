# Themen

Ein Thema ist eine einzige Palette, und alles wird aus ihr gemalt. Wechseln Sie das Thema, und der ganze
Desktop ändert sich gemeinsam: GNOMEs heller oder dunkler Stil und die Akzentfarbe, das Terminal, der
Shell-Prompt, die obere Leiste, das Dock, das Hintergrundbild, VSCodium und dieses Handbuch. Ein helles
Thema gibt dem Dock eine leichte dunkle Tönung, damit seine Symbole auch auf einem blassen
Hintergrundbild hervortreten.

## Wechseln

| Taste | Funktion |
| --- | --- |
| `Super+Ctrl+Shift+Space` | Ein Thema wählen |
| `Super+Ctrl+D` | Zwischen der hellen und der dunklen Version des Themas wechseln |
| `Super+Ctrl+Space` | Nächstes Hintergrundbild des aktuellen Themas |

Dasselbe in einem Terminal:

```bash
ame desktop theme                   # pick one from a list
amethystora-theme list              # the themes, with the current one marked
amethystora-theme set "Tokyo Night" # apply one by name
amethystora-theme toggle            # light or dark
amethystora-theme current           # which one is on
```

## Die Themen

| Thema | Zu verwendender Name | Stil |
| --- | --- | --- |
| Amethystora | `amethystora` | Dunkel, die Vorgabe |
| Amethystora Light | `amethystora-light` | Hell |
| Catppuccin Mocha | `catppuccin` | Dunkel |
| Catppuccin Latte | `catppuccin-latte` | Hell |
| Everforest | `everforest` | Dunkel |
| Gruvbox | `gruvbox` | Dunkel |
| Matte Black | `matte-black` | Dunkel |
| Nord | `nord` | Dunkel |
| Rosé Pine Dawn | `rose-pine-dawn` | Hell |
| Tokyo Night | `tokyo-night` | Dunkel |

Die beiden Amethystora-Themen bringen auch das GTK-Thema von Amethystora mit, das mit den drei farbigen
Lichtern in der Ecke jedes Fensters. Die anderen behalten GNOMEs eigenes Aussehen in ihrer Akzentfarbe.
`Super+Ctrl+D` wechselt ein Thema zu seinem Partner (Catppuccin Mocha und Latte, Amethystora und
Amethystora Light); ein Thema ohne Partner wechselt zu Amethystora im jeweils anderen Modus.
