# Juegos y aplicaciones Android

Ninguna de las dos cosas viene configurada, porque cada una necesita algo que la imagen niega a las
aplicaciones que no lo necesitan. Un solo comando para cada una lo configura, avisa primero de lo que
cambia y también lo deshace todo.

## Juegos

```bash
ame apps gaming
```

Esto instala **Steam** desde Flathub, con **ProtonPlus**, que añade otras versiones de Proton, y dos
herramientas que pueden usar los juegos de Steam: la superposición **MangoHud** y el compositor
**gamescope** de Valve. También ofrece **Heroic**, para las tiendas de Epic, GOG y Amazon, y
**Lutris**, para juegos de cualquier otro sitio.

En este escritorio se niega X11 a todo Flatpak, porque una aplicación en X11 puede leer lo que escribe
en otras aplicaciones X11
([lo que no pueden hacer las aplicaciones Flatpak](software.md#what-flatpak-apps-may-not-do)). Steam
y los juegos de Windows que ejecuta con Proton no dibujan con nada más, así que se vuelve a conceder
X11 a Steam y a los lanzadores que añada, solo para su cuenta. La página **Aplicaciones** de la
aplicación Seguridad lo muestra junto con todos los demás permisos
([Permisos de las aplicaciones](app-permissions.md)).

En las **Propiedades** de un juego en Steam, en **Opciones de lanzamiento**:

| Opción de lanzamiento | Qué hace |
| --- | --- |
| `mangohud %command%` | La superposición: fotogramas por segundo, tiempos de fotograma, temperaturas |
| `gamemoderun %command%` | El modo de rendimiento de la CPU mientras se ejecuta el juego, mediante GameMode |
| `gamescope -f -- %command%` | El juego en su propio compositor, a pantalla completa, lo que ayuda con los juegos que se llevan mal con Wayland |

Los mandos funcionan sin configurar nada: los de Xbox, PlayStation, Nintendo y Steam, 8BitDo y el
resto que conoce Steam. **Add another launcher** añade Heroic o Lutris más adelante, y
**Take it all away** los desinstala todos, con sus juegos, y retira lo que se concedió:

```bash
ame apps gaming launchers
ame apps gaming off
```

## Un planificador de CPU más rápido para juegos

El núcleo (kernel) decide qué programa se ejecuta en cada núcleo de la CPU y durante cuánto tiempo. Su
propio planificador sirve para casi todo. Un planificador sched-ext es un pequeño programa al que el
núcleo cede ese trabajo, y uno hecho para un tipo de tarea puede hacerlo mejor:

```bash
ame system scheduler
```

| Planificador | Para |
| --- | --- |
| `kernel` | Todo: el propio del núcleo, y el predeterminado |
| `lavd` | Juegos, fluidos aunque haya otro trabajo en marcha: hecho para consolas portátiles y portátiles para juegos |
| `bpfland` | Un escritorio que sigue respondiendo bien mientras algo pesado se compila o se renderiza |
| `flash` | Una temporización estable, para trabajar con audio y música |

La elección se mantiene tras reiniciar, y `ame system scheduler kernel` vuelve a poner el del núcleo.
Si un planificador sched-ext se detiene, el núcleo vuelve a encargarse por sí solo, así que lo peor
que puede pasar es que las cosas vayan más lentas. Un núcleo compilado antes del dwarves 1.32 de Fedora
no puede cargar los planificadores actuales; el comando lo indica y deja en su sitio el del núcleo.

## Aplicaciones Android

```bash
ame apps android
```

**Waydroid** ejecuta Android, LineageOS 20, en un contenedor junto a su escritorio, y las aplicaciones
Android que instale aparecen en la cuadrícula de aplicaciones como cualquier otra. Al configurarlo se
descarga Android de los servidores de Waydroid, alrededor de un gigabyte. Viene sin las aplicaciones de
Google ni Play Store, cuyas aplicaciones vienen con las condiciones propias de Google: consiga
aplicaciones en F-Droid, o instale un `.apk` que haya descargado:

```bash
waydroid app install ~/Downloads/some-app.apk
```

Abra **Waydroid** desde la cuadrícula de aplicaciones; el primer arranque tarda un minuto. Su servicio
de contenedor se ejecuta como root, así que solo se activa cuando lo configura, y puede detenerlo
cuando no lo use:

| Comando | Qué hace |
| --- | --- |
| `ame apps android start` | Inicia Android y lo abre |
| `ame apps android stop` | Lo detiene, hasta que lo vuelva a iniciar |
| `ame apps android off` | Quita Android, con todas las aplicaciones y todo lo guardado en él |

Waydroid no puede dibujar con el controlador de NVIDIA, así que en las imágenes NVIDIA Android dibuja
por software: bien para la mayoría de las aplicaciones, lento para los juegos.
