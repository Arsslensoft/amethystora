# Acerca de

Amethystora lo crea y lo mantiene Arsslen Idadi ([@iarsslen](https://github.com/iarsslen)). Es
software libre bajo la Apache License 2.0, desarrollado de forma abierta en
[github.com/arsslensoft/amethystora](https://github.com/arsslensoft/amethystora). Allí son bienvenidas las
incidencias, las ideas y las pull requests.

Amethystora se basa en [Universal Blue](https://universal-blue.org), copyright de los colaboradores de Universal Blue, con licencia Apache License 2.0.

## Este manual

Las páginas son archivos Markdown en `/usr/share/amethystora/manual`, uno por página, y la página de
atajos de teclado es `/usr/share/amethystora/keybindings.md`, el mismo archivo que muestra
`ame desktop keybindings`. Vienen con la imagen, así que el manual siempre describe el sistema que
está usando, y funciona sin conexión de red.

```bash
amethystora-manual                 # open it where you left off
amethystora-manual updates         # open a page by its file name
amethystora-manual --list          # the pages there are
```

Dentro del manual: `Ctrl+K` o `/` buscan, `Alt+Left` y `Alt+Right` van hacia atrás y hacia delante,
`Ctrl+=` y `Ctrl+-` cambian el tamaño del texto, y `Ctrl+0` lo restablece.

## A hombros de gigantes

Amethystora está construido sobre [Fedora](https://fedoraproject.org) y [GNOME](https://www.gnome.org),
y sobre el trabajo de muchos otros proyectos: el núcleo Linux y systemd, bootc y rpm-ostree, Flatpak
y Flathub, Homebrew, Distrobox y Podman, y todas las extensiones de GNOME enumeradas en
[Moverse por el escritorio](desktop.md#extensions). Los iconos son
[candy-icons](https://github.com/EliverLara/candy-icons), de Eliver Lara, y el tema GTK de Amethystora
es una versión recoloreada de su Sweet. Los fondos de pantalla de los temas están dibujados para
Amethystora.

Fedora y el logotipo de Fedora son marcas comerciales de Red Hat, Inc. Amethystora no está
proporcionado, respaldado ni avalado por el Proyecto Fedora ni por Red Hat. Fedora está en
[fedoraproject.org](https://fedoraproject.org).

Las licencias de todo lo que hay en la imagen están en `/usr/share/licenses`. Algunas partes no son
software libre y se rigen por las condiciones de sus fabricantes: el firmware de los dispositivos, el
controlador DisplayLink para las bases de conexión y el controlador de NVIDIA en las imágenes NVIDIA.
`/usr/share/licenses/amethystora/NOTICE` las enumera.

## Código fuente

Todos los paquetes de la imagen figuran en `/usr/share/licenses/amethystora/SOURCES`, con su licencia,
el paquete fuente a partir del que se compiló y dónde está ese código fuente. El código fuente de un
paquete de Fedora está en el sistema de compilación de Fedora, en la dirección indicada allí, o a un
comando de distancia:

```bash
dnf download --srpm <package>    # the source of the version Fedora has now
```

Algunos paquetes los compilan otros (RPM Fusion, negativo17, Fedora Copr), que solo publican su
compilación actual de cada uno. Cuando la licencia de uno de esos paquetes exige su código fuente, el
paquete fuente con el que se compiló esta imagen está en la propia imagen, en `/usr/src/amethystora`.

Todo lo demás se compila a partir del [repositorio de Amethystora](https://github.com/arsslensoft/amethystora)
en el commit indicado como `BUILD_ID` en `/usr/lib/os-release`. Durante al menos tres años después de
la última publicación de una imagen, el código fuente completo de todo lo que contiene cuya licencia
exige que su código fuente esté disponible (la GNU GPL y la LGPL, la MPL, la CDDL, la licencia del
códec AAC de Fraunhofer) se proporciona también a quien lo pida mediante el
[gestor de incidencias](https://github.com/arsslensoft/amethystora/issues); la oferta por escrito está en
`/usr/share/licenses/amethystora/NOTICE`.
