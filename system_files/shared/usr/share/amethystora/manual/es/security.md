# Seguridad

Amethystora está reforzado más allá de los valores predeterminados de Fedora, y lo dice claramente:
todo lo que sigue se puede comprobar, y casi todo se puede cambiar si le estorba.

## Ver en qué punto está

Abra **Seguridad** desde la cuadrícula de aplicaciones, o ejecute:

```bash
ame security status
```

Ambos muestran lo que hacen realmente ahora mismo los ajustes de seguridad, con qué hacer con lo que
esté desactivado, y ninguno cambia nada ni pide contraseña. En la aplicación, lo que requiere su
atención aparece primero, con el comando que lo corrige y un botón que lo ejecuta en una terminal, y
las protecciones descritas más abajo, desactivadas hasta que las quiera, están cada una a un botón de
distancia. Para un examen más a fondo, `ame security audit` hace pasar Lynis por unos cientos de
comprobaciones y explica cada hallazgo.

La página **Aplicaciones** de la aplicación lista a qué puede acceder cada aplicación Flatpak fuera de
su entorno aislado, y qué parte de ello se concedió en este equipo
([Permisos de las aplicaciones](app-permissions.md)).

La página **Contenedores** de la aplicación lista lo que está instalado fuera de Flatpak: los
contenedores que creó `amepkg`, las aplicaciones de su cuadrícula de aplicaciones que vienen de ellos y
lo que se compiló desde el AUR, cada uno con la fecha de su última actualización. El informe le avisa
cuando uno lleva dos semanas sin actualizarse. [Contenedores](containers.md).

Cuando algo requiere su atención, **Preguntar al agente** se lo pasa al [agente de IA](ai-agent.md),
que lo investiga en una terminal y no cambia nada hasta que usted esté de acuerdo.

## Activado desde el principio

- **Las actualizaciones deben estar firmadas.** Solo se instalan imágenes firmadas con la clave de
  Amethystora, así que una imagen manipulada o sustituida se rechaza.
  [Actualizaciones](streams.md#signed-updates).
- **El cortafuegos rechaza las conexiones entrantes.** Se permiten el descubrimiento de impresoras y
  dispositivos, el uso compartido de archivos de Windows, la configuración de IPv6 y GSConnect; su
  tailnet de Tailscale es de confianza. Para dejar entrar algo más, como un servidor de desarrollo al
  que quiera llegar desde su teléfono, use la aplicación **Cortafuegos** o
  `sudo firewall-cmd --add-port=8080/tcp` (añada `--permanent` para conservarlo tras un reinicio).
- **Diez contraseñas incorrectas seguidas** bloquean una cuenta durante diez minutos. Desbloquéela
  antes desde otra cuenta de administrador con `sudo faillock --user <name> --reset`.
- **Los inicios de sesión fallidos repetidos por la red** hacen que se bloquee la dirección: cinco
  fallos en diez minutos cuestan una hora, y cada nuevo bloqueo de la misma dirección dura más, hasta
  un día. Vigila el servidor SSH, lo único de aquí en lo que se puede iniciar sesión por la red, y deja
  en paz a la tailnet. `ame security addresses` lista lo que está bloqueado, y
  `ame security addresses <address>` vuelve a dejar entrar una dirección.
- **El servidor SSH está desactivado,** y rechaza los inicios de sesión de root cuando lo activa.
- **Las aplicaciones no pueden espiar el teclado de las demás.** A las aplicaciones Flatpak se les
  niegan X11 y los dispositivos de entrada
  ([Instalar software](software.md#what-flatpak-apps-may-not-do)), y el reasignador de teclas, que lee
  cada tecla, está desactivado hasta que lo pida.
- **El núcleo está reforzado:** la memoria se borra al asignarse, las direcciones del núcleo están
  ocultas, los programas no pueden leer la memoria de otros, y los módulos poco usados (protocolos de
  red y sistemas de archivos antiguos, FireWire) no pueden cargarse. `gdb -p` sobre un proceso que no
  inició usted necesita `sudo`.
- **El núcleo no se deja reescribir mientras se ejecuta.** El bloqueo (lockdown) impide cargar módulos
  que no son de confianza, escribir en `/dev/mem` y trazar la memoria del núcleo con BPF, así que un
  compromiso de root termina en el siguiente reinicio. Necesita Secure Boot activado y la clave
  registrada ([Hardware](hardware.md#secure-boot)). En un equipo en el que no se puede activar Secure
  Boot, quítelo con `sudo rpm-ostree kargs --delete=lockdown=integrity`. Las imágenes NVIDIA no lo
  usan.
- **Se vigila lo que sobreviviría a un reinicio, y se le avisa.** El registro de auditoría anota los
  cambios en las cuentas, en quién puede usar `sudo`, en lo que se inicia solo (sus entradas de inicio
  automático, sus propios servicios de systemd, los archivos de inicio de la shell) y en los ajustes de
  seguridad, y cada programa que abre un dispositivo de entrada. El vigilante de seguridad lo lee cada
  15 minutos e indica qué cambió y qué programa lo cambió.
  [El vigilante de seguridad](security-watcher.md).
- **Cada semana se ejecuta un análisis de virus** en sus carpetas personales y temporales, y **Lynis
  audita la configuración cada mes.** De forma predeterminada, ninguno de los dos elimina ni mueve
  nada: un falso positivo que le quita un archivo que quería es peor que casi todo lo que eliminaría.
  `ame security scan` muestra lo que encontró el último análisis, y `ame security scan now` ejecuta
  uno. [Analizar en busca de virus](viruses.md#scanning-for-viruses) hace lo mismo en una ventana, y
  [Ajustes](security-settings.md) hace que los análisis pongan en cuarentena o eliminen lo que encuentren.

## Merece la pena activarlo
