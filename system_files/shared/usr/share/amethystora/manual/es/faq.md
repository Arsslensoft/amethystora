# Preguntas y respuestas

## ¿Por qué no puedo usar `dnf install`?

Porque el sistema es una imagen, y la imagen es la misma en todos los equipos. Eso es lo que hace que
las actualizaciones sean seguras y que volver atrás sea instantáneo. Las aplicaciones vienen de
Flathub, las herramientas de Homebrew, y todo lo demás de un contenedor;
[Instalar software](software.md) explica qué es cada cosa. En un contenedor Fedora
(`amepkg containers new --template fedora`), `dnf install` funciona exactamente como lo conoce.

## ¿Puedo usar otro navegador?

El navegador es Firefox, exactamente como lo compila Fedora: Amethystora no le añade ningún ajuste ni
extensión. Chrome, Brave y todos los demás navegadores están en Bazaar. Para que uno sea el
predeterminado, abra **Configuración → Aplicaciones → Aplicaciones predeterminadas**; las
[aplicaciones web](web-apps.md#which-browser-runs-them) se abrirán entonces también en él.

## ¿Dónde está Brave?

Amethystora traía Brave antes. Ahora está en Bazaar, como los demás navegadores. Lo que tenía en él
sigue en `~/.config/BraveSoftware`, y Firefox puede traerlo: **Ajustes → General → Importar datos del
navegador**.

## ¿Esto es Fedora?

Por debajo, sí: Amethystora está construido sobre el escritorio atómico de Fedora y GNOME, y sigue las
versiones de Fedora. Lo que añade es el escritorio que lo rodea, el refuerzo de la seguridad, las
herramientas y una imagen que se prueba y se firma antes de llegarle.

## ¿Puedo jugar?

Sí. `ame apps gaming` instala Steam con lo que necesitan sus juegos, y le concede el X11 con el que
dibuja, que se niega a todas las demás aplicaciones. Muchos juegos de Windows funcionan con Proton en
Steam. [Juegos y aplicaciones Android](games.md) explica lo que configura y cómo añadir Heroic o
Lutris.

## ¿Puedo ejecutar aplicaciones Android?

Sí, en Waydroid: `ame apps android` lo configura. [Juegos y aplicaciones Android](games.md#android-apps).

## ¿Puedo ejecutar software de Windows?

A menudo. Instale **Bottles** desde Bazaar, que ejecuta programas de Windows en sus propios prefijos.
Wine necesita X11, así que concédaselo a Bottles en Flatseal. Para lo demás, una máquina virtual con
Windows en la [imagen para desarrolladores](developer.md) sirve para casi todo.

## ¿Cómo ejecuto una AppImage?

Instale **Gear Lever** desde Bazaar. Pone las AppImage en la cuadrícula de aplicaciones y las mantiene
actualizadas.

## ¿Cómo instalo un `.deb` o un `.rpm`?

Ábralo en Archivos. Va a un contenedor creado para su tipo de paquete, tras un análisis de virus y un
examen de su procedencia; si existe la misma aplicación en Flathub, se ofrece primero.
[Un archivo de paquete](software.md#a-package-file).

## ¿Dónde se guarda mi configuración?

En su carpeta personal: `~/.config` y `~/.local` para las aplicaciones y el escritorio,
`~/.config/amethystora` para el tema y el resto de la configuración propia de Amethystora, y
`~/.var/app` para cada Flatpak. Haga una copia de seguridad de su carpeta personal y tendrá una copia
de todo lo que es suyo.

## ¿Por qué siempre seis áreas de trabajo?

Para que un número siempre corresponda al mismo sitio. Con áreas de trabajo que aparecen y
desaparecen, `Super+3` depende de lo que tuviera abierto hace una hora; con seis fijas, el navegador
en la 1 y la terminal en la 2 están siempre justo donde sus dedos esperan encontrarlos.

## ¿Por qué no puedo cambiar archivos en `/usr`?

`/usr` es la imagen, de solo lectura a propósito, y cada actualización lo sustituye por completo. La
configuración de todo el equipo va en `/etc`, donde sus cambios se conservan y se combinan con cada
actualización. La configuración de su cuenta va en su carpeta personal.

## ¿Puedo confiar en la imagen?

Se compila públicamente en GitHub a partir del código fuente de
[arsslensoft/amethystora](https://github.com/arsslensoft/amethystora), se firma con una clave cuya mitad
pública va en cada imagen, y su equipo solo la acepta si esa firma es válida.
