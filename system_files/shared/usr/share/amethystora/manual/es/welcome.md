# Le damos la bienvenida a Amethystora

Amethystora es un escritorio que se configura una vez y después simplemente se usa. Está construido
sobre Fedora y GNOME, se actualiza solo en segundo plano, y una actualización que sale mal se deshace
con un simple reinicio.

No es un montón de paquetes que tenga que mantener en orden. Todo el sistema operativo es una sola
imagen, compilada, probada y firmada antes de llegar a su equipo, y sustituida por completo cuando
sale una nueva. Sus archivos, su configuración y sus aplicaciones están a su lado, intactos. Por eso
no hay nada que limpiar, ninguna actualización a medias de la que recuperarse y ninguna reinstalación
cada par de años.

Sobre esa base se asienta un escritorio pensado para manejarse con el teclado, tan bonito como
rápido: ventanas que se colocan en mosaico, seis áreas de trabajo que siempre están donde las dejó,
un cambio de tema que lo repinta todo a la vez y un agente de IA que sabe cómo funciona este sistema.

Hay dos decisiones que conviene tomar el primer día. Si prefiere ventanas que floten y se superpongan,
como en Windows y macOS, una notificación se las ofrece poco después de su primer inicio de sesión, y
`ame desktop layout classic` cambia a ellas en cualquier momento ([Ventanas clásicas](tiling.md#classic-windows)).
Si el equipo es para un niño, el [control parental](logins.md#parental-controls) limita su cuenta a
las aplicaciones y los horarios que usted elija.

## Cómo funciona este manual

La primera vez, léalo de principio a fin; cada página termina con un enlace a la siguiente. Después,
use la búsqueda: `Ctrl+K` o `/` lleva al cuadro de búsqueda, y los resultados van directamente a la
sección que necesita.

- **Para empezar** prepara un equipo nuevo: lo primero que hay que hacer y cómo instalar.
- **El escritorio** es todo lo que usa a diario: moverse por él, los atajos de teclado, los temas, la
  terminal, las aplicaciones web y el agente.
- **Software** explica cómo instalar cualquier cosa, desde aplicaciones hasta herramientas de
  desarrollo, sin dañar el sistema.
- **El sistema** trata las actualizaciones, la seguridad y el hardware.
- **Ayuda** es donde buscar cuando algo va mal.

Esta página se abre sola la primera vez que inicia sesión, y solo entonces. `Super+F1` abre este
manual desde cualquier lugar, igual que «Manual» en el menú de Amethystora (`Super+Alt+Space`). Desde
una terminal, `amethystora-manual` lo abre y `amethystora-manual keybindings` va directamente a una
página.

## Qué es suyo y qué es de la imagen

Una sola idea explica casi todo el comportamiento de Amethystora:

| Dónde | De quién | Qué le ocurre |
| --- | --- | --- |
| `/usr` | De la imagen | Solo lectura. Cada actualización lo sustituye entero. |
| `/etc` | De ambos | Los valores predeterminados de la imagen, combinados con sus cambios en cada actualización. |
| `/var` y `/home` | Suyo | Ninguna actualización lo toca. |
| `~/.config`, `~/.local` | Suyo | Su configuración, por cuenta. |

Las aplicaciones vienen de Flathub, las herramientas de línea de comandos de Homebrew, y los entornos
de desarrollo viven en contenedores. Ninguno de ellos cambia la imagen, y por eso una actualización
nunca puede estropearlos ni ellos pueden estropear una actualización. [Instalar software](software.md)
explica cada uno.

> **En resumen:** mantenga el sistema operativo intacto, guarde su trabajo a su lado y reinicie
> cuando termine la jornada. Ese es todo el mantenimiento que necesita.
