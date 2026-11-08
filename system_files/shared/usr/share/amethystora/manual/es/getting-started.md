# Primeros pasos

Unos minutos ahora y, a partir de entonces, el equipo se cuida solo. Ninguno de estos pasos es
obligatorio, pero merece la pena hacer cada uno una vez.

## 1. Permitir que Secure Boot confíe en la imagen

Si Secure Boot está activado, los módulos del núcleo incluidos en la imagen (bases de conexión
DisplayLink, cámaras virtuales) solo se cargan cuando su clave de firma está registrada. Puede que el
primer arranque ya se lo haya pedido; si no, ejecute:

```bash
ame security secure-boot
```

En el siguiente reinicio aparece una pantalla azul, el gestor de MOK. Elija **Enroll MOK**, luego
**Continue**, luego **Yes**, y escriba la contraseña `amethystora`. La pantalla usa la distribución
de teclado de EE. UU. (QWERTY). [Hardware](hardware.md#secure-boot) tiene los detalles.

## 2. Comprobar que todo está activado

```bash
ame security status
```

Una pantalla con lo que hacen realmente los ajustes de seguridad del equipo, y qué hacer con todo lo
que esté desactivado. No cambia nada y no pide ninguna contraseña. El mismo informe está en la
cuadrícula de aplicaciones como **Seguridad**, que además analiza en busca de virus
([Seguridad](viruses.md#scanning-for-viruses)).

## 3. Instalar sus aplicaciones

Las aplicaciones que trae Amethystora (Calculadora, Editor de texto, Gestor de extensiones y las demás)
se instalan solas la primera vez que el equipo tiene conexión, y aparecen en la cuadrícula de
aplicaciones a medida que termina cada una. Para tenerlas de inmediato, ejecute `ame apps flatpaks`.
Desinstale las que no quiera; no volverán a instalarse.

Después abra **Bazaar**, el centro de software, e instale desde Flathub lo que use. El navegador es
Firefox; si prefiere Chrome o Brave, también están en Bazaar. [Instalar software](software.md) trata
las herramientas de línea de comandos y todo lo demás.

¿Viene de otro equipo con Amethystora del que hizo una copia de seguridad? `ame restore-setup`
recupera de la copia sus aplicaciones, contenedores, extensiones y tema
([Copias de seguridad](backups.md)).

## 4. Personalizarlo

- `Super+Ctrl+Shift+Space` elige un tema. Repinta a la vez todo el escritorio, la terminal y el
  prompt. [Temas](themes.md).
- Las ventanas se colocan solas en mosaico, una al lado de otra, en una tira que se desplaza. Abra dos
  o tres aplicaciones y muévase entre ellas con `Super+Left` y `Super+Right`.
  [Ventanas en mosaico](tiling.md). Si prefiere ventanas que floten y se superpongan,
  `ame desktop layout classic` cambia a ellas ([Ventanas clásicas](tiling.md#classic-windows)).
- `Super+Alt+Space` abre el menú de Amethystora, que llega desde el teclado a todo lo que describe
  este manual.

## 5. Configurar una copia de seguridad

```bash
ame backup
```

Una copia de seguridad diaria en un lugar del que el ransomware de este equipo no puede borrar nada.
El comando le guía para elegir dónde. [Seguridad](backups.md).

## 6. Aprender las teclas

Merece la pena leer la página de [Atajos de teclado](../keybindings.md). Téngala abierta en una
segunda área de trabajo durante su primera semana.

## Y después, olvídese

Las actualizaciones llegan solas y se aplican la próxima vez que reinicie. Apague el equipo al
terminar la jornada y se mantendrá al día sin que tenga que pensar en ello.
[Actualizaciones](updates.md) explica qué ocurre y cómo deshacer una.
