# Actualizaciones

Las actualizaciones son automáticas y silenciosas. El sistema, sus aplicaciones Flatpak y sus
herramientas de Homebrew se comprueban en segundo plano y se descargan mientras trabaja. El sistema
nuevo se prepara junto al que está en uso y lo sustituye en el próximo reinicio, de una sola vez.
Nunca queda nada actualizado a medias, así que una actualización no puede dejar el equipo en un estado
en el que no arranque.

> **El único hábito que conviene mantener:** apague o reinicie el equipo cuando termine la jornada.
> Es entonces cuando se aplica una actualización preparada.

## Actualizar ahora

Abra **Actualizaciones del sistema** desde la cuadrícula de aplicaciones (o **Check for updates** en
**System**, en el menú de Amethystora) y pulse **Actualizar ahora**. Actualiza de una vez el sistema,
sus aplicaciones Flatpak y sus herramientas de Homebrew, y muestra cada uno a medida que ocurre, sin
pedir contraseña. Es la misma actualización que hacen las automáticas, así que si ya hay una en
marcha, la ventana la sigue en lugar de iniciar otra. Puede cerrar la ventana en cualquier momento; la
actualización continúa.

Un sistema nuevo sigue sustituyendo al actual en el próximo reinicio. Cuando hay uno listo,
Actualizaciones del sistema lo indica y le ofrece reiniciar. También muestra la versión que está
usando, la que espera al reinicio y la que se guarda para [volver a la versión anterior](#rolling-back),
y desactiva o vuelve a activar las actualizaciones automáticas.

Si tiene [contenedores](software.md#keeping-them-up-to-date) de `amepkg`, **Actualizar ahora** los
actualiza justo después de lo demás, en **Contenedores**. Uno que no se actualice lo indica ahí, y la
actualización del sistema sigue contando como terminada.

En una terminal, lo mismo de una vez:

```bash
ame update
```

| Comando | Qué hace |
| --- | --- |
| `ame update` | Actualizarlo todo ahora |
| `amepkg upgrade-all` | Actualizar ahora los contenedores de otras distribuciones |
| `ame system auto-updates` | Desactivar las actualizaciones automáticas, o volver a activarlas |
| `ame changelog` | Qué ha cambiado en los paquetes desde la imagen que está usando |
| `rpm-ostree status` | El sistema en uso, la actualización preparada y el sistema al que volver |
| `fwupdmgr get-updates` | Actualizaciones de firmware para este equipo, de su fabricante |

Las notas de la versión de cada compilación estable están en
[GitHub](https://github.com/arsslensoft/amethystora/releases).

## Firmware

El firmware del equipo y de sus dispositivos (el firmware UEFI, las bases de conexión, los SSD, los
teclados, el lector de huellas) se actualiza aparte del sistema, porque una actualización de firmware
puede necesitar el equipo enchufado y un reinicio. Cada día, fwupd busca firmware nuevo en LVFS, donde
lo publican los fabricantes, y la sección **Firmware** de **Actualizaciones** lista cada dispositivo
que tiene alguno, de qué versión a qué versión. **Actualizar el firmware…** lo instala en una
terminal, donde fwupd indica lo que va a hacer y pregunta antes de reiniciar el equipo.
**Comprobar ahora** vuelve a consultar LVFS de inmediato. En una terminal:

```bash
fwupdmgr get-updates      # what there is
fwupdmgr update           # install it
fwupdmgr security         # how well the firmware protects this machine
```

Las actualizaciones del propio firmware UEFI las escribe el firmware en el próximo reinicio: deje el
equipo enchufado hasta que haya vuelto a arrancar. El informe de seguridad muestra la calificación
que da fwupd a las protecciones del firmware, de HSI:0 a HSI:5; lo alto que puede llegar un equipo
depende sobre todo de su fabricante.

## Volver a la versión anterior

El sistema anterior siempre se conserva. Si una actualización trae un problema:

- **En el menú de arranque,** elija la segunda entrada, que es el sistema anterior a la actualización.
  No se cambia nada, y el siguiente reinicio vuelve al más reciente. Al iniciar sesión, una
  notificación indica que está usando la versión anterior; al pulsarla se abre Actualizaciones del
  sistema, que le ofrece **Conservar esta versión**, que la convierte en la que arranca el equipo y
  pide su contraseña, o **Reiniciar en la más reciente**.
- **Para quedarse en el sistema anterior** desde una terminal, conviértalo en el predeterminado y
  reinicie:

```bash
sudo bootc rollback
systemctl reboot
```

Sus archivos y su configuración son los mismos en ambos. La próxima actualización, automática o no,
trae de vuelta la versión más reciente, así que, para quedarse donde está hasta que salga una
solución, desactive las actualizaciones automáticas en Actualizaciones del sistema, o
[quédese en la compilación](#holding-on-to-one-build) que está usando. Después
[informe del problema](troubleshooting.md#reporting-a-problem), y vuelva a seguir el canal cuando
salga una imagen corregida.

## Canales

| Etiqueta | Qué obtiene |
| --- | --- |
| `stable` | Se compila cada semana, con el núcleo retenido en el de Fedora CoreOS, que va un poco por detrás del de Fedora para que las regresiones del núcleo se detecten antes. Para todo el mundo. |
| `stable-daily` | Lo mismo, recompilado cada vez que Amethystora cambia en lugar de cada semana |
| `latest` | Lo más reciente de Fedora y del núcleo, recompilado cada vez que Amethystora cambia |
| `beta` | Lo que viene. Espere algunas asperezas. |

## Cambiar de canal y de imagen

Para pasar a otro canal o a otra imagen, haga el cambio y reinicie:

```bash
sudo bootc switch --enforce-container-sigpolicy ghcr.io/arsslensoft/amethystora:latest
```

Sustituya `amethystora` por `amethystora-dx` para el [modo desarrollador](developer.md), y `latest`
por el canal que quiera.

Si tiene [paquetes superpuestos](software.md#layering-the-last-resort), cambie con `rpm-ostree` en su
lugar, porque `bootc switch` construye el sistema nuevo solo a partir de la imagen y los descartaría:

```bash
sudo rpm-ostree rebase ostree-image-signed:docker://ghcr.io/arsslensoft/amethystora:latest
```

## Quedarse en una compilación

Cada compilación tiene además una etiqueta propia, como `stable-44.20260922`, a partir de la versión
que figura en las notas de la versión. Cambiar a ella fija el equipo en esa compilación, lo que
resulta útil mientras espera una solución. Un equipo fijado en una sola compilación no recibe
actualizaciones; `ame security status` se lo recuerda, e imprime el comando para volver a seguir el
canal.

## Actualizaciones firmadas

Una actualización solo se acepta si lleva la firma de Amethystora, comprobada con la clave de
`/etc/pki/containers/amethystora.pub`. Una imagen manipulada, o una de cualquier otro, se rechaza en
lugar de instalarse. Un servicio comprueba en cada arranque que la comprobación de firmas sigue
activada, y la vuelve a activar si un cambio de imagen la desactivó.
