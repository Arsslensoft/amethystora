# Instalar software

Aquí no hay `dnf install`, y no lo echará de menos. Cada tipo de software tiene su propio lugar,
ninguno cambia la imagen del sistema, y por eso ninguna aplicación puede estropear una actualización
ni verse estropeada por una.

| Quiere | Use | Cómo |
| --- | --- | --- |
| Una aplicación con ventana | Flatpak, de Flathub | Bazaar, el centro de software |
| Una herramienta de línea de comandos | Homebrew | `brew install <name>` |
| Todo un entorno de usuario Linux: `apt`, `dnf`, `pacman`, un conjunto de herramientas | Un contenedor | `amepkg` ([Contenedores](containers.md)) |
| Un `.deb` o un `.rpm` de un sitio web | Un contenedor creado para él | Ábralo en Archivos ([Un archivo de paquete](containers.md#a-package-file)) |
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
`ame security apps` ([Permisos de las aplicaciones](app-permissions.md)).

## Herramientas de línea de comandos: Homebrew

```bash
brew install ripgrep fd bat
brew search <name>
```

Homebrew instala en su propio prefijo, sin `sudo`, y se actualiza junto con el sistema. La página de
la [terminal](terminal.md#command-line-tools) tiene más información.

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
