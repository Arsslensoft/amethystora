# Fondos de pantalla

Cada tema trae tres fondos de pantalla de lugares imaginarios dibujados en sus propios colores, y al
cambiar a un tema se pone el primero en el escritorio: una ciudad de neón bajo la lluvia para Tokyo
Night, una aurora sobre un lago de montaña para Nord, pinos entre la niebla para Everforest, una
puesta de sol en el desierto para Gruvbox, cumbres bajo una luna pastel para Catppuccin, un eclipse
para Matte Black, etcétera. `Super+Ctrl+Space` los va alternando. Los dos temas Amethystora empiezan
con las facetas de una piedra tallada, y tienen también el campo de cristales y un cielo nocturno de
humo violeta; la Configuración de GNOME los ofrece en Apariencia.

Para añadir los suyos, ponga imágenes (JPG, PNG, WebP o SVG) en una carpeta con el nombre del tema.
Van detrás de las imágenes propias del tema:

```bash
mkdir -p ~/.config/amethystora/backgrounds/amethystora
cp ~/Pictures/mountains.jpg ~/.config/amethystora/backgrounds/amethystora/
```

`amethystora-theme bg list` muestra lo que puede alternar el tema actual, y
`amethystora-theme bg set <picture>` pone uno directamente.

## Transiciones

Un tema o un fondo nuevo no aparece sin más. La pantalla se queda quieta un momento mientras todo
cambia por debajo, y luego el escritorio nuevo se abre en un círculo desde el puntero, con un
resplandor del color de acento del tema en el borde. Funciona igual se haga el cambio como se haga:
con las teclas de arriba, desde la Configuración de GNOME o con el botón Estilo oscuro de la
configuración rápida.

```bash
ame desktop transition         # pick one from a list
ame desktop transition wave    # or name it
```

| Transición | Aspecto |
| --- | --- |
| `grow` | Un círculo se abre desde el puntero. La predeterminada |
| `outer` | El escritorio nuevo se cierra sobre el puntero desde los bordes |
| `wipe` | Un borde suave barre la pantalla en diagonal |
| `wave` | Lo mismo, con un borde ondulado |
| `fade` | Un fundido simple |
| `random` | Una de las cuatro primeras, con cualquier ángulo |
| `none` | Sin animación: el fondo se funde como lo funde GNOME |

Una transición dura 1,2 segundos. Para cambiarlo, indique la duración en milisegundos:

```bash
gsettings set org.gnome.shell.extensions.amethystora-transitions duration 800
```

No hay transiciones mientras las animaciones estén desactivadas en Configuración, en Accesibilidad.
Vienen de la extensión Amethystora Transitions, que se puede desactivar en el Gestor de extensiones.
