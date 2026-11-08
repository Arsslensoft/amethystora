# Ventanas en mosaico

PaperWM coloca las ventanas por usted. Cada área de trabajo es una tira de ventanas, una al lado de
otra y a toda altura, y la tira puede ser más ancha que la pantalla. Una ventana nueva se abre a la
derecha de la que está usando, las ventanas nunca se superponen, y la tira se desplaza para que la
ventana con el foco quede a la vista:

```
  ┌────────┬──────────────┬──────────────────────┬────────┐
  │  Mail  │   Terminal   │       Browser        │  Chat  │
  └────────┴──────────────┴──────────────────────┴────────┘
           ╰──────────── the screen ─────────────╯
```

Mail y Chat siguen abiertas, justo fuera del borde. `Super+Left` vuelve a Mail. Lleve el puntero
contra el borde izquierdo o derecho de la pantalla para ver lo que hay más allá, y pulse en una
ventana de allí para ir a ella. En un panel táctil, deslice tres dedos para mover la tira.

Las teclas siguen un mismo patrón. `Super` y una flecha llevan a otra ventana. Añada `Ctrl` y lo que
se mueve es la ventana. Añada `Shift` y pasa a otro monitor.

## Moverse

| Tecla | Qué hace |
| --- | --- |
| `Super+Left` / `Super+Right` | La ventana de la izquierda / derecha |
| `Super+Up` / `Super+Down` | La ventana de arriba / abajo, cuando varias ventanas comparten columna |
| `Super+Home` / `Super+End` | La primera / última ventana del área de trabajo |
| `Super+[` / `Super+]` | Desplazar la tira sin cambiar la ventana con el foco |
| `Super+Shift` y una flecha | El monitor en esa dirección |

## Organizar

| Tecla | Qué hace |
| --- | --- |
| `Super+Ctrl+Left` / `Super+Ctrl+Right` | Mover la ventana a la izquierda / derecha en la tira |
| `Super+Ctrl+Up` / `Super+Ctrl+Down` | Moverla arriba / abajo en su columna |
| `Super+I` | Traer la ventana de la derecha a esta columna, debajo de esta |
| `Super+O` | Sacar la ventana de abajo de esta columna a una columna propia |
| `Super+Shift+O` | Sacar esta ventana a una columna propia |
| `Super+Ctrl+Page Up` / `Page Down` | Moverla al área de trabajo anterior / siguiente |
| `Super+Shift+Ctrl` y una flecha | Moverla al monitor en esa dirección |
| `Super+T`, manteniendo `Super` pulsada | Llevar la ventana consigo mientras se mueve, y dejarla donde suelte la tecla |

## Tamaño

| Tecla | Qué hace |
| --- | --- |
| `Super+R` | Cambiar el ancho entre el 38 %, el 50 % y el 62 % de la pantalla (`Super+Alt+R` retrocede) |
| `Super+Shift+R` | Lo mismo para la altura de una ventana en una columna |
| `Super+F` | Ocupar todo el ancho de la pantalla, y volver |
| `Super+C` | Centrar la ventana en la pantalla |
| `Super+Shift+C` | Hasta dónde desplaza la tira una ventana con el foco: justo a la vista, al centro o al borde |
| `Super+Shift+W` | Dónde se abren las ventanas nuevas: a la derecha, a la izquierda o debajo de esta |

## Ventanas flotantes

Los diálogos flotan solos sobre la tira. `Super+Ctrl+Escape` saca de la tira la ventana con el foco
para que también flote, y la vuelve a poner en ella. `Super+Alt+Escape` oculta a la vez todas las
ventanas que flotan de esa forma, y las vuelve a mostrar.

## Y lo demás

| Tecla | Qué hace |
| --- | --- |
| `Super+N` | Una ventana nueva de la aplicación que está usando |
| `Super+Ctrl+B` | Ocultar o mostrar la barra superior en esta área de trabajo |

La configuración de PaperWM está en **PaperWM**, dentro del **Gestor de extensiones**. Incluye los
espacios entre ventanas, los anchos que recorre `Super+R` y todas las teclas, algunas de las cuales
esta página no menciona.

## Ventanas clásicas

Si prefiere ventanas que floten y se superpongan, como en Windows y macOS, cambie a la disposición
clásica:

```bash
ame desktop layout classic
```

Desactiva PaperWM, y las actualizaciones lo dejan desactivado. Las ventanas se abren entonces donde
GNOME las coloca y se arrastran por la barra de título. `Super+Up` maximiza y `Super+Down` restaura,
`Super+Left` / `Super+Right` colocan una ventana en esa mitad de la pantalla, y `Super+Shift+Left` /
`Super+Shift+Right` la mueven al monitor siguiente. `ame desktop layout tiling` recupera la tira. La
primera vez que inicia sesión, una notificación le ofrece la misma elección.
