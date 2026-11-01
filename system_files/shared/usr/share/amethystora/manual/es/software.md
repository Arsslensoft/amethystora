# Instalar software

Aquí no hay `dnf install`, y no lo echará de menos. Cada tipo de software tiene su propio lugar,
ninguno cambia la imagen del sistema, y por eso ninguna aplicación puede estropear una actualización
ni verse estropeada por una.

| Quiere | Use | Cómo |
| --- | --- | --- |
| Una aplicación con ventana | Flatpak, de Flathub | Bazaar, el centro de software |
| Una herramienta de línea de comandos | Homebrew | `brew install <name>` |
| Todo un entorno de usuario Linux: `apt`, `dnf`, `pacman`, un conjunto de herramientas | Un contenedor | `amepkg` ([Contenedores](#containers)) |
| Un `.deb` o un `.rpm` de un sitio web | Un contenedor creado para él | Ábralo en Archivos ([Un archivo de paquete](#a-package-file)) |
| Un sitio web como aplicación | Una aplicación web | `amethystora-webapp install` ([Aplicaciones web](web-apps.md)) |
| Algo que tiene que estar en el propio anfitrión | Superposición | `rpm-ostree install`, como último recurso |

## Aplicaciones: Bazaar y Flathub

Abra **Bazaar** e instale desde [Flathub](https://flathub.org), donde se publican casi todas las
aplicaciones de escritorio para Linux. Las aplicaciones Flatpak se actualizan solas junto con el
sistema, y quitar una no deja nada atrás. Desde una terminal:

```bash
flatpak install flathub org.gimp.GIMP
flatpak list --app
flatpak uninstall --delete-data org.gimp.GIMP
```

Hay dos herramientas más para los Flatpak:

- **Flatseal** muestra y cambia a qué puede acceder cada aplicación: carpetas, dispositivos, la red.
- **Warehouse** gestiona las aplicaciones instaladas y limpia lo que dejan atrás.

### Lo que no pueden hacer las aplicaciones Flatpak

A todo Flatpak se le niegan X11, `/dev/input` y el propio servicio de Flatpak, pida lo que pida su
manifiesto. Juntos son la vía por la que una aplicación podría leer lo que escribe en otra. Las
aplicaciones escritas para Wayland no lo notan. Una aplicación que solo habla X11 (Wine, algunos
juegos, algunas compilaciones antiguas de Electron) necesita que se le vuelva a conceder, de una en
una, en Flatseal o con:

```bash
flatpak override --user --socket=x11 <app-id>
```

A qué puede acceder cada aplicación más allá de su entorno aislado, lo que pidió y lo que usted le
concedió, está en la página **Aplicaciones** de la aplicación Seguridad, y en una terminal con
`ame security apps` ([Permisos de las aplicaciones](security.md#app-permissions)).

## Herramientas de línea de comandos: Homebrew

```bash
brew install ripgrep fd bat
brew search <name>
```

Homebrew instala en su propio prefijo, sin `sudo`, y se actualiza junto con el sistema. La página de
la [terminal](terminal.md#command-line-tools) tiene más información.

## Contenedores

Para todo lo que espera una distribución tradicional (un conjunto de compiladores, un `.deb` que
distribuye un fabricante, un paquete que solo está en el Arch User Repository, una herramienta solo
para Ubuntu), cree un contenedor. Comparte su carpeta personal y su pantalla, así que las aplicaciones
de dentro abren ventanas como cualquier otra, y se puede desechar sin dejar rastro. Nada de lo que se
instala en uno toca el sistema.

> **Un contenedor no es un entorno aislado.** Lo que instala en uno se ejecuta como usted, con su
> carpeta personal, su pantalla, su sonido y su bus de sesión. Cuando una aplicación esté en Flathub,
> instálela desde allí: las aplicaciones Flatpak están aisladas, firmadas y se actualizan con el sistema.

### Paquetes de otras distribuciones

`amepkg`, abreviatura de `amethystora-pkg`, crea contenedores a partir de plantillas (Debian, Ubuntu,
Fedora, Arch, Alpine, openSUSE Tumbleweed y openSUSE Leap) e instala en ellos con sus propios gestores
de paquetes:

```bash
amepkg containers new --template debian
amepkg debian install gimp
amepkg debian enter
```

La primera línea descarga la imagen que indica la plantilla, crea el contenedor y lo pone al día.
`install` instala desde los repositorios firmados del propio Debian con `apt`, y pone en la cuadrícula
de aplicaciones cada aplicación que trajo el paquete, marcada «(on debian)»; `remove` las vuelve a
quitar. Antes de instalar una aplicación, `amepkg` busca la misma aplicación en Flathub y la ofrece
primero.

| Comando | Qué hace |
| --- | --- |
| `amepkg containers list` | Los contenedores, sus plantillas, cuándo se actualizó cada uno por última vez, sus aplicaciones |
| `amepkg containers new` | Un contenedor nuevo: pregunta la plantilla y el nombre |
| `amepkg containers rm <name>` | Elimina el contenedor y todo lo instalado en él |
| `amepkg containers reset <name>` | Lo elimina y lo vuelve a crear a partir de su plantilla; su carpeta personal se conserva |
| `amepkg <name> install`, `remove` o `purge <package>` | Instala o quita paquetes |
| `amepkg <name> search <words>`, `show <package>` o `list` | Busca en sus repositorios, o en lo instalado |
| `amepkg <name> update`, `upgrade`, `autoremove` o `clean` | Refresca sus listas de paquetes, lo actualiza, hace limpieza |
| `amepkg <name> enter` o `run <command>` | Una shell dentro, o un solo comando |
| `amepkg <name> start` o `stop` | Lo inicia o lo detiene |
| `amepkg <name> export --app <app>` o `--bin <path>` | Pone una aplicación en la cuadrícula de aplicaciones, o un comando en `~/.local/bin`; `unexport` lo deshace |

Solo lista y toca los contenedores que creó: sus propios contenedores de Distrobox no se tocan. En
Arch, `update` solo refresca las listas de paquetes, y Arch no admite instalar a partir de listas más
recientes que los paquetes ya instalados: el que hay que ejecutar es `upgrade`, que primero las
refresca.

### Un archivo de paquete

Haga doble clic en un `.deb` o un `.rpm` en Archivos, o desde una terminal:

```bash
amepkg install ~/Downloads/some-app.deb
```

Un `.deb` va a un contenedor Debian, un `.rpm` a uno Fedora y un `.pkg.tar.zst` a uno Arch, que se crea
a partir de su plantilla si aún no existe. Primero se analiza el archivo en busca de virus, y no se
instala nada si coincide con malware conocido. Después se muestra lo que el archivo dice de sí mismo:
su nombre y versión, quién lo hizo, si está firmado y si ejecuta scripts propios al instalarse. Un
`.rpm` puede llevar una firma; un `.deb` normalmente no, porque Debian firma sus repositorios y no cada
archivo, así que un `.deb` de un sitio web no lleva ninguna prueba de quién lo hizo. Si la misma
aplicación está en Flathub, o el paquete está en los repositorios del propio contenedor, se ofrecen
primero: ambos están firmados y se mantienen al día, y un archivo de un sitio web no cumple ninguna de
las dos cosas.

Un paquete instalado desde un archivo solo se actualiza con su contenedor si trae consigo el
repositorio de su fabricante, como lo configuran algunos paquetes de fabricantes al instalarse.

### Mantenerlos al día

Cada contenedor se actualiza una vez al día con un temporizador de su propia cuenta
(`amethystora-pkg-upgrade.timer`). Igual que las actualizaciones automáticas del sistema, espera
mientras el equipo funciona con poca batería, está ocupado o le falta memoria. **Actualizar ahora**,
en [Actualizaciones del sistema](updates.md#updating-now), también los actualiza, justo después del
sistema, y muestra cómo ha ido en **Contenedores**: un contenedor que no se actualizó nunca hace que la
actualización del sistema cuente como fallida. En una terminal:

```bash
amepkg upgrade-all
```

El [informe de seguridad](security.md#see-where-you-stand) le avisa cuando un contenedor lleva dos
semanas sin actualizarse, y la página **Contenedores** de la aplicación Seguridad lista lo que hay
instalado en cada uno.

### Plantillas y gestores de paquetes

Una plantilla indica la imagen de la que parte un contenedor, el gestor de paquetes con el que se
maneja y los paquetes con los que empieza cada contenedor nuevo. Un gestor de paquetes son los diez
comandos que `amepkg` ejecuta por él, de `install` a `clean`. Cree los suyos:

```bash
amepkg templates new --name devbox --base quay.io/toolbx/ubuntu-toolbox:24.04 \
    --pkg-manager apt --packages "build-essential git"
amepkg templates list
amepkg managers list
```

Una plantilla puede dar a sus contenedores una carpeta personal propia (`--own-home`), que mantiene su
configuración separada de la suya, y las opciones `--unshare` de distrobox (`--unshare netns,ipc`),
adecuadas para herramientas de línea de comandos; una aplicación con ventana suele necesitar lo que
estas quitan. Una carpeta personal propia no oculta sus archivos: siguen siendo accesibles por su ruta
completa. Las plantillas suyas se guardan en `~/.local/share/amethystora/pkg`, y las integradas no se
pueden cambiar ni quitar. `export` escribe una en un archivo, e `import` la vuelve a leer, aquí o en
otro equipo.
`import` también lee los archivos YAML que Apx escribe para sus pilas y
sus gestores de paquetes.

Las imágenes de las plantillas integradas están fijadas a un digest que va dentro de la imagen del
sistema firmada, así que un contenedor parte exactamente de la imagen con la que se compiló y probó
Amethystora, y las imágenes de Debian, Alpine, openSUSE Tumbleweed y openSUSE Leap se comprueban
además con las firmas de sus editores: la de Leap, con las claves propias de openSUSE, que comprueban
también cualquier imagen de Leap que descargue. La imagen de una plantilla suya se toma tal como la
entrega su registro: `templates list` la marca como no verificada, y crear un contenedor a partir de
ella pide confirmación primero, salvo que esté fijada con `@sha256:…` o venga de un registro cuyas
firmas compruebe este equipo.

### El AUR

El Arch User Repository está desactivado. Actívelo con:

```bash
ame apps aur
amepkg containers new --template arch-aur
```

Eso añade la plantilla `arch-aur`, cuyos contenedores tienen una carpeta personal propia, y el gestor
de paquetes `paru`, que se compila desde el AUR en el contenedor la primera vez. Cualquiera puede subir
paquetes al AUR y nadie revisa lo que hay, así que antes de compilar nada `amepkg` muestra lo que el
AUR dice del paquete: quién lo mantiene, cuántos votos tiene, cuándo se envió y cuándo se actualizó
por última vez, y si está marcado como desactualizado, con una advertencia si tiene menos de 30 días o
ningún voto. Después paru le muestra el PKGBUILD, el script que lo compila: léalo. Lo que tienen los
repositorios propios de Arch se toma primero de ellos. La actualización diaria no toca lo que vino del
AUR, porque cada versión nueva es un script nuevo que leer: `amepkg <name> upgrade`, en una terminal,
también actualiza eso.

### En scripts

Con `--json`, las listas imprimen JSON con campos que no cambian:

- `amepkg containers list --json`: `name`, `container` (su nombre en podman), `template`, `manager`,
  `image`, `status`, `home`, `init`, `unshare`, `exported_apps`, `exported_bins`, `packages`,
  `files`, `aur_packages`, `aur_pending`, `created`, `last_upgrade` (ambos en segundos Unix) y
  `last_upgrade_result`
- `amepkg templates list --json`: `name`, `description`, `base`, `manager`, `packages`, `own_home`,
  `unshare`, `built_in`, `verified` y `pinned`
- `amepkg managers list --json`: `name`, `need_sudo`, `noconfirm`, los diez comandos y `built_in`

`containers new`, `templates new` y `update`, y `managers new` y `update` preguntan en una terminal lo
que no se les haya indicado. Con `--no-prompt`, solo aceptan opciones y fallan si falta algo. Los
scripts, los lanzadores y los servicios lo llaman `amethystora-pkg`, su nombre completo.

### Contenedores a mano

Distrobox y Toolbox funcionan como en cualquier otro sitio:

```bash
distrobox create --name ubuntu --image quay.io/toolbx/ubuntu-toolbox:24.04
distrobox enter ubuntu
```

**DistroShelf** muestra en una ventana todos los contenedores, los suyos y los de `amepkg`, y la
terminal abre una pestaña dentro de cualquiera de ellos desde el menú junto a su botón de pestaña
nueva. Una aplicación instalada en uno se puede poner en la cuadrícula de aplicaciones desde dentro con
`distrobox-export --app <name>`.

## Superposición: el último recurso

`rpm-ostree install` añade un paquete a la imagen en este equipo. Úselo solo para lo que no puede
estar en ningún otro sitio, como un controlador o un servicio del sistema:

```bash
sudo rpm-ostree install <package>
```

Reinicie para usarlo. Cada paquete superpuesto hace más lenta cada actualización, y el paquete solo se
conserva mientras actualice mediante `rpm-ostree`. Dos cosas que conviene saber:

- **Al cambiar de imagen o de canal:** `bootc switch` construye el sistema nuevo solo a partir de la
  imagen y descarta los paquetes superpuestos sin avisar. En un equipo con paquetes superpuestos,
  cambie en su lugar con
  `sudo rpm-ostree rebase ostree-image-signed:docker://ghcr.io/arsslensoft/<image>:<stream>`. Las
  actualizaciones automáticas se encargan de esto por usted.
- **Para deshacerlo:** `sudo rpm-ostree uninstall <package>` quita uno, y `sudo rpm-ostree reset`
  vuelve a la imagen sin cambios.

Añadir un repositorio de `dnf` o ejecutar `dnf install` en el anfitrión no funciona: el sistema no
tiene una base de datos de paquetes en la que se pueda escribir.

## VPN

- **Tailscale** está instalado, y puede usarlo sin `sudo`: `tailscale up` le une a su tailnet. El
  cortafuegos confía en la tailnet, donde se aplican las reglas de acceso de su propia tailnet.
- Las configuraciones de **WireGuard** de cualquier proveedor se pueden importar en
  **Configuración → Red → VPN**.
