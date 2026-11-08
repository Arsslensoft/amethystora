# La terminal

Amethystora está hecho tanto para quienes disfrutan de la línea de comandos como para quienes nunca la
abren. Cuando la abra, estará lista para usted.

## Abrir una

`Super+Return` abre una terminal, igual que `Ctrl+Alt+T` y `Ctrl+Alt+Return`. La terminal es Ptyxis.
Conoce los contenedores: el menú junto al botón de pestaña nueva abre una pestaña en el anfitrión o
dentro de cualquiera de sus [contenedores de Distrobox y Toolbox](containers.md).

Sus colores siguen el [tema](themes.md), y el prompt también.

## El saludo

Una terminal nueva le saluda con el logotipo de Amethystora, un resumen del equipo, la imagen que está
usando, algunos comandos útiles y un consejo. `ame desktop greeting` lo desactiva, y lo vuelve a
activar.

`fetch` vuelve a mostrar el logotipo y el resumen del sistema cuando quiera. Un destello recorre las
facetas del logotipo antes de que aparezca el resumen; en su lugar verá el logotipo quieto por SSH, en
la consola de texto, en una terminal sin color completo, con `NO_COLOR` definida o cuando
**Reducir animación** esté activado en **Configuración → Accesibilidad**. Todo lo que añada después
de `fetch` se pasa a fastfetch, que es lo que ejecuta.

## Shells

La shell es bash, con el prompt Starship. fish y zsh también están instaladas. El cambio se hace en la
terminal y no en todo el sistema, para que una configuración de shell rota nunca le impida iniciar
sesión:

1. Abra las **Preferencias** de la terminal y edite su perfil.
2. Active **Usar comando personalizado** e introduzca `/usr/bin/fish` o `/usr/bin/zsh`.

## ame

`ame` ejecuta los comandos que vienen con el sistema: pequeños scripts probados para las tareas que,
si no, exigirían una búsqueda en la web, organizados en grupos como `desktop`, `security` y `system`.
Este manual los menciona donde son útiles; para verlos todos:

```bash
ame                           # the groups, and the commands outside them
ame security                  # the commands in one group, with a line about what each does
ame --list --list-submodules  # every command, in every group
ame -n security status        # print what a command would run, without running it
```

`ame pkg` es [amethystora-pkg](containers.md#packages-from-other-distributions). `ujust`, el nombre
anterior de `ame`, ejecuta los mismos comandos, y sus nombres de antes de agruparse, como
`ujust setup-backup`, siguen funcionando.

Los comandos son recetas de `just`, y el propio `just` está a su disposición para sus proyectos: un
`justfile` en cualquier carpeta convierte sus comandos en recetas de la misma forma.

## Herramientas de línea de comandos

Las herramientas de línea de comandos vienen de Homebrew, que instala en su carpeta personal sin
`sudo` y sin tocar el sistema:

```bash
brew install ripgrep
brew search <name>
brew upgrade
```

Vea lo que hay en [formulae.brew.sh](https://formulae.brew.sh). Homebrew se actualiza junto con el
sistema. No lo ejecute nunca con `sudo`: es suyo, no de root.

Ya vienen en la imagen: `just`, `gum`, `glow`, `tmux`, `fastfetch`, `git` y el resto de las
herramientas habituales. Las fuentes incluyen Inter, JetBrains Mono y los símbolos de Nerd Fonts, para
que los prompts y los gestores de archivos de la terminal dibujen sus iconos.
