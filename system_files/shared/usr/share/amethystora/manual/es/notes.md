# Notas y tareas

**Notas**, en la cuadrícula de aplicaciones, reúne sus notas y sus listas de tareas en una sola ventana.
Todo se queda en este equipo. No se sincroniza nada y la aplicación nunca se conecta a Internet.
Sustituye a Joplin y Planify, y trae lo que ya tenga en ellas (vea
[Si viene de Joplin o Planify](#coming-from-joplin-or-planify)).

La primera vez que la abra, le ofrecerá cifrar sus notas con una frase de contraseña. Es la opción
que recomienda, y el resto de esta página da por hecho que la eligió.

## Notas

Las notas se escriben en Markdown y se guardan en libretas, que pueden contener otras libretas. El
editor muestra el texto y la nota con formato uno al lado del otro. **Ctrl+E** alterna entre el texto,
ambos y la nota con formato sola.

- **Listas de comprobación:** `- [ ] like this`. Marque las casillas en la nota con formato.
- **Adjuntos:** pegue una imagen, suelte un archivo sobre la nota o use el clip. Las imágenes se ven en
  la nota. Pulse en cualquier otro adjunto para guardar una copia.
- **Etiquetas:** escriba en la fila de etiquetas, bajo el título, y pulse **Enter**. Las etiquetas
  aparecen en la barra lateral.
- **Mover una nota:** elija otra libreta en la parte superior de la nota, o arrastre la nota a una
  libreta de la barra lateral.
- **Versiones anteriores:** el reloj de la parte superior de una nota. Se guarda una versión cada vez
  que vuelve a cambiar una nota tras diez minutos sin tocarla, hasta las últimas 30. Al restaurar una,
  el texto al que sustituye también se guarda como versión.
- **Papelera:** una nota que elimina va a la papelera, donde puede restaurarla o eliminarla
  definitivamente.
- **PDF:** el menú **⋯** de una nota la exporta como PDF.

## Tareas

**Tareas**, en la parte superior de la barra lateral, cambia a sus listas de tareas:

| Vista | Muestra |
| --- | --- |
| Bandeja de entrada | Las tareas que no están en ningún proyecto |
| Hoy | Lo que vence hoy, y todo lo vencido |
| Próximas | La semana que viene, día a día, y todo lo posterior |
| Fijadas | Las tareas que ha fijado |
| Completadas | Lo que ha terminado, lo más reciente primero |

Los proyectos agrupan tareas y pueden dividirse en secciones. Las etiquetas atraviesan los proyectos.
Pulse en una tarea para establecer su fecha y hora, su repetición, su recordatorio, su prioridad, sus
etiquetas y sus notas, y para añadirle subtareas.

El campo **Añadir una tarea** entiende un poco de lenguaje natural, en inglés. Saca esto del título:

| Escriba | Para |
| --- | --- |
| `today`, `tomorrow`, `friday`, `next week`, `2026-10-31` | La fecha |
| `5pm`, `17:30` | La hora, con un recordatorio a esa hora |
| `every day`, `every 2 weeks`, `monthly` | Una tarea que se repite |
| De `p1` a `p4` | La prioridad; `p1` es la más alta |
| `@errand` | Una etiqueta, que se crea si es nueva |
| `#Garden` | Un proyecto que ya existe |

Al completar una tarea que se repite, pasa a su siguiente fecha.

### Recordatorios

Una tarea con hora puede avisarle, desde **En el momento** hasta **1 día antes**. Los recordatorios
aparecen como notificaciones mientras Notas está en marcha. Al cerrar la ventana, Notas sigue en marcha
en segundo plano hasta que hayan saltado los recordatorios pendientes. Puede desactivarlo en
**Configuración**. Mientras funciona así, sus notas están bloqueadas; solo se guardan, en memoria, los
títulos y las horas de los recordatorios.

## Si viene de Joplin o Planify

Abra **Configuración** y use **Importar**. Importar de nuevo el mismo archivo actualiza lo que vino de
él en lugar de añadirlo dos veces.

- **Joplin:** en Joplin, elija **Archivo**, **Exportar todo**, **JEX**. Importe el archivo `.jex`.
  Se traen las libretas, las notas, las etiquetas y los adjuntos. Las tareas pendientes de Joplin se
  convierten en tareas de la bandeja de entrada.
- **Planify:** en Planify, abra **Preferencias**, **Copia de seguridad**, y cree una copia. Importe su
  archivo `.json` desde `~/.var/app/io.github.alainm23.planify/data/io.github.alainm23.planify/backups`.
  Se traen los proyectos, las secciones, las tareas, las subtareas, las etiquetas, las fechas y las
  repeticiones.
- Los **archivos Markdown** van a una libreta llamada **Imported**.

Cuando lo tenga todo, puede quitar las aplicaciones antiguas en Bazaar, o con
`flatpak uninstall net.cozic.joplin_desktop io.github.alainm23.planify`.

## Teclas

| Tecla | Qué hace |
| --- | --- |
| **Ctrl+N** | Una nota nueva, o una tarea nueva en **Tareas** |
| **Ctrl+F** | Buscar notas o tareas |
| **Ctrl+1** / **Ctrl+2** | Notas / Tareas |
| **Ctrl+E** | Texto, ambos o la nota con formato |
| **Ctrl+B**, **Ctrl+I**, **Ctrl+K** | Negrita, cursiva, enlace |
| **Ctrl+L** | Bloquear |
| **Ctrl+,** | Configuración |
