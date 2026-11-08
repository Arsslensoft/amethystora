# Hardware

Amethystora funciona mejor en hardware que Linux admite bien: gráficos Intel y AMD, y los portátiles
que sus fabricantes certifican para Linux. La mayoría de las cosas funcionan en cuanto se conectan;
esta página trata de las que necesitan algún paso por su parte.

## Secure Boot

Deje Secure Boot activado. Con él, el equipo solo arranca un núcleo que ha comprobado, y el
[bloqueo](security.md) del núcleo tiene en qué apoyarse.

Los módulos del núcleo incluidos en la imagen, para las bases de conexión DisplayLink y las cámaras
virtuales, están firmados con la clave propia de Amethystora, y hay que decirle al firmware una vez
que confíe en ella:

```bash
ame security secure-boot
```

Reinicie, y aparecerá una pantalla azul antes de que arranque el sistema: el gestor de MOK. Elija
**Enroll MOK**, luego **Continue**, luego **Yes**, y escriba la contraseña `amethystora`. La pantalla
usa la distribución de teclado de EE. UU. (QWERTY) sea cual sea la suya, lo que importa si su teclado
tiene las letras en otro sitio. `ame security status` le indica si las claves están registradas.

Si Secure Boot está desactivado, una notificación lo indica una vez. Actívelo en la configuración del
firmware del equipo y después ejecute el comando anterior.

## Gráficos

- **Intel y AMD** funcionan desde el primer momento. La imagen para desarrolladores añade ROCm para el
  cálculo en GPU con AMD.
- **NVIDIA** necesita las imágenes `-nvidia-open`, que llevan el controlador de núcleo abierto de NVIDIA ([Instalación](choosing.md#pick-an-image)).
  El controlador de NVIDIA se compila para el equipo y se firma con su propia clave, así que las
  imágenes NVIDIA no usan el bloqueo del núcleo.
- **¿Instaló la imagen sin el controlador de NVIDIA en un equipo con tarjeta NVIDIA?** No hay
  instalador para las imágenes NVIDIA, así que así es como empiezan la mayoría de los equipos con
  NVIDIA. Unos minutos después del primer arranque, una notificación ofrece **Switch images…**, que
  abre `ame system rebase` con la imagen `-nvidia-open` correspondiente; solo acepta imágenes firmadas
  y pregunta antes de cambiar. Una imagen NVIDIA en un equipo sin tarjeta NVIDIA ofrece la imagen sin
  el controlador, que mantiene el bloqueo activado. Nada cambia por sí solo, el aviso se da una vez
  por equipo, y **Don't ask again** lo detiene para siempre.
