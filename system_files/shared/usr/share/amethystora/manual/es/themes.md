# Temas

Un tema es una paleta, y todo se pinta a partir de ella. Al cambiar de tema, todo el escritorio cambia
a la vez: el estilo claro u oscuro y el color de acento de GNOME, la terminal, el prompt de la shell,
la barra superior, el dock, el fondo de pantalla, VSCodium y este manual. Un tema claro da al dock un
ligero tinte oscuro, para que sus iconos sigan destacando sobre un fondo pálido.

## Cambiar de tema

| Tecla | Qué hace |
| --- | --- |
| `Super+Ctrl+Shift+Space` | Elegir un tema |
| `Super+Ctrl+D` | Alternar entre la versión clara y la oscura del tema |
| `Super+Ctrl+Space` | Siguiente fondo de pantalla del tema actual |

Lo mismo desde una terminal:

```bash
ame desktop theme                   # pick one from a list
amethystora-theme list              # the themes, with the current one marked
amethystora-theme set "Tokyo Night" # apply one by name
amethystora-theme toggle            # light or dark
amethystora-theme current           # which one is on
```

## Los temas

| Tema | Nombre que usar | Estilo |
| --- | --- | --- |
| Amethystora | `amethystora` | Oscuro, el predeterminado |
| Amethystora Light | `amethystora-light` | Claro |
| Catppuccin Mocha | `catppuccin` | Oscuro |
| Catppuccin Latte | `catppuccin-latte` | Claro |
| Everforest | `everforest` | Oscuro |
| Gruvbox | `gruvbox` | Oscuro |
| Matte Black | `matte-black` | Oscuro |
| Nord | `nord` | Oscuro |
| Rosé Pine Dawn | `rose-pine-dawn` | Claro |
| Tokyo Night | `tokyo-night` | Oscuro |

Los dos temas Amethystora traen además el tema GTK de Amethystora, el de las tres luces de colores en
la esquina de cada ventana. Los demás conservan el aspecto propio de GNOME con su color de acento.
`Super+Ctrl+D` cambia un tema por su pareja (Catppuccin Mocha y Latte, Amethystora y Amethystora
Light); un tema sin pareja cambia a Amethystora en el otro modo.
