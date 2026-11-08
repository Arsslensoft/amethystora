# En la red

## Protección del navegador

```bash
ame security browser
```

Una extensión ve todas las páginas que abre, y así es como llegan la mayoría de los ladrones de
contraseñas. Con la protección del navegador activada, los navegadores solo instalan las extensiones
de una lista: los gestores de contraseñas Bitwarden, 1Password, Proton Pass y KeePassXC, y las
extensiones que añada con `ame security browser allow <id>`. Además, las páginas web no pueden acceder
a dispositivos USB, serie, HID ni Bluetooth. Cubre Firefox, Brave, Chrome, Chromium y Edge, instalados
ahora o más adelante, para todas las cuentas del equipo. Los demás navegadores no están cubiertos.

Permita las extensiones que usa antes de activarla. Firefox elimina las que no están en la lista, con
su configuración, la próxima vez que se inicia; los demás navegadores desactivan las suyas. Firefox
muestra el ID de cada extensión en `about:support`, en Complementos, y los demás en su página de
extensiones con el modo de desarrollador activado. Los temas, diccionarios y paquetes de idioma de
Firefox no se tocan.

Lo que permitió está en `/etc/amethystora/browser-extensions`, un ID por línea. Para retirar uno,
quite su línea y vuelva a ejecutar `ame security browser on`. Está desactivada de forma
predeterminada: Firefox viene exactamente como lo compila Fedora, y lo que puede instalar lo decide
usted, no la imagen.

## Protección de red

```bash
ame security settings network
```

Suricata inspecciona cada conexión que hace y recibe este equipo con las reglas Emerging Threats Open,
que descarga cada día:

- **`watch`** informa de lo que reconoce y no bloquea nada. Empiece por aquí: una semana así muestra
  qué estorbaría el bloqueo.
- **`block`** además corta las conexiones a malware conocido y sus servidores de mando, exploits,
  páginas de phishing y criptomineros, y a las direcciones de las listas de bloqueo de Spamhaus y
  DShield. Lo que las reglas reconocen con menos certeza solo se notifica.

Cada conexión bloqueada genera una notificación. **Red**, en la aplicación **Seguridad**, lista lo que
se bloqueó y se detectó, igual que `ame security connections`. Si una conexión era suya y la quería,
**Permitir**, o `ame security connections allow <rule>`, excluye esa regla en este equipo, y
`ame security connections block <rule>` la vuelve a poner.

Pase lo que pase con ella, sus conexiones siguen funcionando: mientras Suricata se inicia, se
reinicia, se queda atrás o se detiene, pasan sin inspeccionar en lugar de no pasar. Una conexión
cifrada se inspecciona hasta su negociación inicial, el nombre del servidor y el certificado, porque
lo que sigue no se puede leer. Su registro guarda lo que reconoció y nada más, nunca una lista de los
sitios que visita. Inspecciona las conexiones propias de este equipo, no lo que reenvía para máquinas
virtuales o contenedores ejecutados como root, y ocupa unos cientos de megabytes de memoria. Está
desactivada de forma predeterminada: bloquear algo es una decisión sobre su propio tráfico, y le
corresponde a usted.
