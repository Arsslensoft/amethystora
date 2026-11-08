# El vigilante de seguridad

Cada 15 minutos, o en el momento en que ocurren las cosas si la
[vigilancia en tiempo real](security-settings.md) está activada, el vigilante examina:

- **El registro de auditoría**: las cuentas, las reglas de `sudo`, lo que se inicia solo, los ajustes
  de seguridad y los módulos del núcleo cargados desde una sesión, cada uno con el programa que hizo el
  cambio. Un programa que abre el teclado directamente se notifica la primera vez que ese programa lo
  hace; los juegos y los reasignadores de teclas también lo hacen.
- **El diario**: una cuenta bloqueada tras contraseñas incorrectas, tres o más contraseñas incorrectas
  de `sudo` o de administrador, y los dispositivos USB que bloqueó la
  [protección USB](devices-security.md#block-usb-devices-you-did-not-plug-in).
- **`/etc` frente a la imagen**: la imagen guarda su propia copia de `/etc` en `/usr/etc`, así que un
  ajuste de seguridad cambiado, o un archivo de `/etc` que sustituye a uno de la imagen en `/usr/lib`,
  aparece como **Ajustes de la imagen** en el informe.
- **Lo que no debería existir en este sistema**: un programa setuid fuera de `/usr`,
  `/etc/ld.so.preload`, un archivo normal en `/dev`, un módulo del núcleo que no vino con la imagen o
  no está firmado, y una interfaz de red que lee todos los paquetes. Son las
  **Comprobaciones de rootkits** del informe.
- **Lo que acepta conexiones**: cada programa que escucha en la red, al que puede llegar cualquier
  dispositivo de su tailnet. Uno nuevo se notifica una vez.
- **Lo que su sesión encuentra primero**: un programa en `~/.local/bin`, `~/bin` o la carpeta de
  Homebrew que se llame como uno de los comandos del sistema, y que su shell ejecuta en lugar del real.
  Un `sudo` o un `ssh` falso ahí es una forma antigua de robar una contraseña, así que esos se notifican
  con más fuerza; la carpeta de Homebrew solo cuenta para esos. Y un lanzador de su cuadrícula de
  aplicaciones que no creó ninguna exportación de contenedor. Cada uno se notifica cuando aparece, y de
  nuevo si cambia.
- La **protección de red**, mientras está activada: cada conexión que bloqueó, con la forma de volver
  a permitirla, y lo que reconoció sin bloquear cuando la regla lo califica de grave.
  [Protección de red](network-security.md#network-protection).
- **El propio informe**: cuando una comprobación que indica que algo se desactivó, como el
  cortafuegos, las actualizaciones firmadas o el registro de auditoría, deja de estar bien.

Todo lo nuevo llega como notificación, y espera en el informe hasta que lo lea:

```bash
ame security events
```

`ame security events now` mira antes. Si hizo usted el cambio, o instaló algo que lo hizo, no hay nada
que hacer.
