# Modificar un tema o crear uno propio

Los temas vienen de solo lectura en `/usr/share/amethystora/themes`. Una carpeta con el mismo nombre en
`~/.config/amethystora/themes` se superpone a ellos, así que solo tiene que escribir los archivos que
quiera cambiar. Una carpeta con un nombre propio es un tema nuevo. Cada archivo es pequeño y opcional,
salvo la paleta:

| Archivo | Contiene |
| --- | --- |
| `colors.toml` | La paleta: `accent`, `foreground`, `background`, `cursor`, `selection_foreground`, `selection_background`, y de `color0` a `color15` |
| `light.mode` | Presente, y vacío, cuando el tema es claro |
| `pair.theme` | El tema al que cambia `Super+Ctrl+D` |
| `accent.theme` | El color de acento de GNOME: blue, teal, green, yellow, orange, red, pink, purple o slate. Sin él, se elige el más parecido |
| `gtk.theme` | Un nombre de tema GTK, en lugar del aspecto propio de GNOME |
| `icons.theme` | Un tema de iconos, en lugar de candy-icons |
| `cursor.theme` | Un tema de cursor |
| `vscode.theme` | El tema de color de VSCodium al que cambiar |
| `tophat.theme` | El color de los indicadores de la barra superior, cuando el acento se ve mal como línea fina |
| `backgrounds/` | Fondos de pantalla |
| `backgrounds.list` | Fondos de pantalla guardados en otro lugar del sistema, una ruta por línea. El primero es con el que empieza el tema |

Un tema sin imágenes propias recibe cuatro dibujadas en sus colores: un resplandor suave, colinas
entre la bruma, un mapa topográfico y cintas ondulantes.

Un tema propio se crea con tres comandos:

```bash
mkdir -p ~/.config/amethystora/themes/sunset
cp /usr/share/amethystora/themes/amethystora/colors.toml ~/.config/amethystora/themes/sunset/
amethystora-theme set sunset
```

Edite los colores, vuelva a ejecutar `amethystora-theme set sunset` y mire el resultado.

## Ir más allá

- **Cómo se escribe una configuración.** La paleta de la terminal, el prompt, los colores de btop y
  los fondos dibujados son plantillas en `/usr/share/amethystora/themed`. Copie una en
  `~/.config/amethystora/themed/` y edite la copia para cambiar cómo la genera cada tema.
- **Ejecutar algo en cada cambio.** Ponga un ejecutable en
  `~/.config/amethystora/hooks/theme-set.d/`. Recibe el nombre del tema en `AMETHYSTORA_THEME` y su
  carpeta en `AMETHYSTORA_THEME_DIR`.
- **Su propio prompt sigue siendo suyo.** Si ha escrito usted mismo `~/.config/starship.toml`, el
  tema no lo toca.
- **No edite nunca `~/.config/amethystora/current/`.** Se reescribe en cada cambio.

## El menú de arranque

El menú de arranque también puede llevar las ilustraciones de Amethystora. Está desactivado de forma
predeterminada, porque el menú de arranque está fuera de la imagen:

```bash
ame desktop boot-menu
```

Vuelva a ejecutarlo para quitar las ilustraciones. Una vez activado, las actualizaciones de la imagen
lo mantienen al día.
