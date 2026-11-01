# Solución de problemas

La mayoría de los problemas en un sistema basado en imágenes tienen las mismas tres respuestas:
averiguar qué ha pasado, volver atrás si lo causó una actualización e informar de ello para que la
próxima imagen lo corrija.

## Deje que el agente mire primero

**Diagnose a problem**, en el menú de Amethystora (`Super+Alt+Space`), o desde una terminal:

```bash
amethystora-agent diagnose "the second monitor stays black after suspend"
```

Lee los registros y los informes de fallos, no cambia nada y vuelve con la causa y una solución que
considerar. [El agente de IA](ai-agent.md#diagnose-a-problem) tiene más información.

## Los registros

Abra **Registros** desde la cuadrícula de aplicaciones, o ejecute `amethystora-logs`. Se abre en
**Importante**, los errores de todo el equipo desde que arrancó. La barra lateral reparte el resto
entre su sesión, el sistema, el núcleo y el hardware, la seguridad (inicios de sesión, `sudo`, lo que
detectaron las reglas de auditoría) y **Fallos**, cada uno con un recuento de lo nuevo desde el último
arranque cuando importa.

- **Buscar mensajes** examina lo que dicen los mensajes a medida que escribe. El botón `.*` de al lado
  hace que la búsqueda se tome como una expresión regular.
- **Periodo** vuelve al arranque anterior, a cualquier arranque que aún guarde el diario, a la última
  hora, al último día o a la última semana, o a todo. **Nivel** oculta lo que sea menos grave que lo
  que elija.
- **Aplicaciones y servicios** lista todo lo que ha escrito en el diario. Elija una aplicación para
  leer lo que dijo cada vez que se ejecutó, o un servicio del sistema o de su sesión.
- Pulse en una entrada para leerla entera, con todos los campos que guarda el diario. **Solo …** limita
  la vista a su procedencia, y **Alrededor de esta** muestra todo lo registrado en el minuto anterior y
  en el posterior.
- **Seguir** añade las entradas nuevas a medida que se escriben. **Exportar** guarda lo que se muestra
  en un archivo de texto o JSON, y el botón de la terminal abre la misma vista en `journalctl`. El
  comando de la vista también está al pie de la barra lateral.

Los administradores, las cuentas del grupo `wheel`, lo ven todo. Cualquier otra cuenta solo ve su
propia sesión y sus aplicaciones, y las vistas del sistema lo indican.

Lo mismo en una terminal:

| Comando | Muestra |
| --- | --- |
| `journalctl -b -p warning` | Las advertencias y los errores desde este arranque |
| `journalctl -b -1 -p warning` | Lo mismo del arranque anterior, tras un fallo o un cuelgue |
| `journalctl --user -b` | Su propia sesión: el escritorio, sus aplicaciones |
| `systemctl --failed` | Los servicios que no pudieron iniciarse |
| `coredumpctl list` | Los programas que se cerraron inesperadamente |
| `ame system logs-this-boot` | Todo lo de este arranque |
| `ame system local-overrides` | Los archivos de `/etc` que usted, o algo que ejecutó, ha cambiado |

## Una actualización estropeó algo

Elija el sistema anterior en el menú de arranque para confirmar que fue la actualización.
Actualizaciones del sistema le ofrecerá entonces conservarlo, lo que también hace
`sudo bootc rollback`, hasta que salga una imagen corregida. [Actualizaciones](updates.md#rolling-back).

## Problemas frecuentes

### Ha desaparecido un paquete que superpuse

Se superpuso con `rpm-ostree install`, y después se cambió el equipo con `bootc switch`, que construye
el sistema nuevo solo a partir de la imagen. Vuelva a instalarlo y, a partir de ahora, cambie de imagen
o de canal con `rpm-ostree rebase` ([Instalar software](software.md#layering-the-last-resort)).

### Una aplicación no ve una carpeta o un dispositivo, o no llega a abrirse

Las aplicaciones Flatpak solo llegan a lo que se les permite, y en Amethystora eso excluye X11 y los
dispositivos de entrada. Abra **Flatseal**, elija la aplicación y concédale lo que necesita: una
carpeta en **Sistema de archivos**, o **Sistema de ventanas X11** para una aplicación que no habla
Wayland.

### Una extensión falla, o el escritorio se ve mal

Desactive la extensión en el **Gestor de extensiones**, y después cierre la sesión y vuelva a
iniciarla. Si el escritorio sigue fallando, restablezca la configuración de GNOME a los valores
predeterminados de la imagen y vuelva a aplicar su tema:

```bash
dconf reset -f /org/gnome/
amethystora-theme reload
```

Eso restablece toda la configuración de GNOME de esta cuenta, incluidos el dock, sus atajos de teclado
personalizados y su elección de extensiones, así que resérvelo para cuando nada más haya funcionado.

### `Super+Ctrl+Space` no cambia nada

Pasa al siguiente fondo de pantalla del tema actual. Un tema propio con una sola imagen no tiene
adónde pasar, y una notificación lo indica: añada más ([Temas](themes.md#wallpapers)). El selector
de temas es `Super+Ctrl+Shift+Space`.

### La cuenta se bloquea tras varias contraseñas incorrectas

Diez contraseñas incorrectas bloquean la cuenta durante diez minutos. Espere, o desbloquéela desde otra
cuenta de administrador con `sudo faillock --user <name> --reset`.

### Una base de conexión DisplayLink no muestra nada

Con Secure Boot activado, su controlador solo se carga cuando la clave de Amethystora está registrada:
`ame security secure-boot`, y después reinicie ([Hardware](hardware.md#secure-boot)).

### Cambiar la distribución del teclado

Aquí `Super+Space` es la búsqueda, así que la siguiente distribución de teclado está en
`Super+Shift+Space`. Añada distribuciones en **Configuración → Teclado**.

### El disco se está llenando

```bash
ame system clean
```

Elimina los contenedores, las imágenes y los entornos de ejecución de Flatpak que no se usan.
`flatpak uninstall --unused` y `podman system prune` hacen a mano parte de lo mismo.

## Informar de un problema

Informe de los errores en [GitHub](https://github.com/arsslensoft/amethystora/issues), y haga sus
preguntas en las [discusiones](https://github.com/arsslensoft/amethystora/discussions). `ame report`
reúne lo que necesita un informe en un archivo de su carpeta personal (la imagen, sus grupos, los
servicios que fallaron y los errores registrados desde este arranque), se lo muestra todo y después,
si usted lo indica, abre una incidencia nueva con todo rellenado salvo los errores. No se publica nada
hasta que lo envíe; arrastre el archivo si los errores ayudan. Si no, incluya:

- la salida de `rpm-ostree status`, que indica la imagen exacta,
- lo que hizo, lo que esperaba y lo que ocurrió en su lugar,
- las líneas pertinentes de los registros anteriores,
- si la imagen anterior tenía el mismo problema.
