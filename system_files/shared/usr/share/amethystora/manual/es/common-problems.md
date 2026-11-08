# Problemas frecuentes

## Ha desaparecido un paquete que superpuse

Se superpuso con `rpm-ostree install`, y después se cambió el equipo con `bootc switch`, que construye
el sistema nuevo solo a partir de la imagen. Vuelva a instalarlo y, a partir de ahora, cambie de imagen
o de canal con `rpm-ostree rebase` ([Instalar software](software.md#layering-the-last-resort)).

## Una aplicación no ve una carpeta o un dispositivo, o no llega a abrirse

Las aplicaciones Flatpak solo llegan a lo que se les permite, y en Amethystora eso excluye X11 y los
dispositivos de entrada. Abra **Flatseal**, elija la aplicación y concédale lo que necesita: una
carpeta en **Sistema de archivos**, o **Sistema de ventanas X11** para una aplicación que no habla
Wayland.

## Una extensión falla, o el escritorio se ve mal

Desactive la extensión en el **Gestor de extensiones**, y después cierre la sesión y vuelva a
iniciarla. Si el escritorio sigue fallando, restablezca la configuración de GNOME a los valores
predeterminados de la imagen y vuelva a aplicar su tema:

```bash
dconf reset -f /org/gnome/
amethystora-theme reload
```

Eso restablece toda la configuración de GNOME de esta cuenta, incluidos el dock, sus atajos de teclado
personalizados y su elección de extensiones, así que resérvelo para cuando nada más haya funcionado.

## `Super+Ctrl+Space` no cambia nada

Pasa al siguiente fondo de pantalla del tema actual. Un tema propio con una sola imagen no tiene
adónde pasar, y una notificación lo indica: añada más ([Temas](wallpapers.md)). El selector
de temas es `Super+Ctrl+Shift+Space`.

## La cuenta se bloquea tras varias contraseñas incorrectas

Diez contraseñas incorrectas bloquean la cuenta durante diez minutos. Espere, o desbloquéela desde otra
cuenta de administrador con `sudo faillock --user <name> --reset`.

## Una base de conexión DisplayLink no muestra nada

Con Secure Boot activado, su controlador solo se carga cuando la clave de Amethystora está registrada:
`ame security secure-boot`, y después reinicie ([Hardware](hardware.md#secure-boot)).

## Cambiar la distribución del teclado

Aquí `Super+Space` es la búsqueda, así que la siguiente distribución de teclado está en
`Super+Shift+Space`. Añada distribuciones en **Configuración → Teclado**.

## El disco se está llenando

```bash
ame system clean
```

Elimina los contenedores, las imágenes y los entornos de ejecución de Flatpak que no se usan.
`flatpak uninstall --unused` y `podman system prune` hacen a mano parte de lo mismo.
