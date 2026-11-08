# Proteger sus notas

## Cifrado

Con el cifrado activado, sus notas, tareas y adjuntos se escriben en el disco cifrados con
AES-256-GCM, con una clave aleatoria que solo puede abrir su frase de contraseña, o una
[llave de acceso](#passkeys) que añada. La frase de contraseña se refuerza con scrypt, que cuesta
128 MiB de memoria por cada intento, así que probar frases una tras otra es lento.

Sigue siendo seguro frente a las computadoras cuánticas. Lo que rompe una computadora cuántica es la
criptografía de clave pública (RSA, curvas elípticas), y aquí no se usa ninguna. Contra un cifrado como
AES, lo más que puede hacer una computadora cuántica es reducir a la mitad la longitud de la clave. Eso
deja a AES-256 con una fortaleza de 128 bits, y por eso AES-256 es lo que el NIST y la CNSA 2.0 de la
NSA mantienen para los años posteriores a las computadoras cuánticas.

> **Nadie puede recuperar una frase de contraseña olvidada.** Ni Amethystora, ni un administrador,
> ni nadie. Guárdela en un lugar seguro, y conserve una [copia de seguridad](#backing-up).

Sus notas se bloquean:

- tras 10 minutos sin usar Notas (**Configuración**, **Bloquear en su ausencia**),
- cuando se bloquea la pantalla, se suspende el equipo o se cierra la ventana,
- cuando pulsa **Ctrl+L** o **Bloquear** en la barra lateral.

**Configuración** también cambia la frase de contraseña, y desactiva o vuelve a activar el cifrado.

### Llaves de acceso

Una llave de seguridad FIDO2 (YubiKey, Thetis o un modelo con huella dactilar como la YubiKey Bio)
puede abrir sus notas, para que no tenga que escribir la frase de contraseña cada vez. La frase de
contraseña sigue funcionando, y es lo que abre sus notas si pierde la llave: una llave de acceso es
una segunda forma de entrar, nunca la única.

Para añadir una, conecte la llave, abra **Configuración** y elija **Añadir una llave de acceso**.
Introduzca su frase de contraseña y el PIN de la llave, o deje el PIN vacío si la llave lee su huella
dactilar. La llave parpadea dos veces; tóquela cada vez. A partir de entonces, la pantalla de bloqueo
ofrece **Abrir con una llave de acceso**, que pide el PIN de la llave, o su huella dactilar, y un
toque.

- **La llave necesita un PIN**, y un modelo con huella dactilar, un dedo registrado. Configúrelos en
  Firefox, en `about:webauthn`. Un toque por sí solo nunca abre sus notas: quien encuentre la llave no
  puede usarla sin el PIN o su dedo, y la llave bloquea su PIN tras ocho intentos fallidos.
- **Añada una segunda llave de reserva**, de una en una. Cada una aparece en **Configuración**, donde
  **Quitar** hace que deje de abrir sus notas.
- **Si pierde una llave**, quítela. Alguien que tenga la llave, conozca su PIN y tenga una copia
  antigua de su carpeta de notas aún podría abrir esa copia. Desactivar el cifrado y volver a activarlo
  da a sus notas una clave nueva, que ninguna llave de acceso quitada abre; después, vuelva a añadir
  las llaves que aún tenga.
- Una [copia de seguridad](#backing-up) siempre pide la frase de contraseña: las llaves de acceso no
  forman parte de ella.

La llave no guarda nada de sus notas. Contiene un secreto que nunca sale de ella y, una vez comprobado
el PIN o la huella, calcula un valor (HMAC-SHA256, el `hmac-secret` de FIDO2) con el que se cifra una
segunda copia de la clave de sus notas. Lo que hay en el disco sigue cifrado solo con AES-256.

## Copias de seguridad

Como no se sincroniza nada, guarde una copia en otro lugar. En **Configuración**:

- **Copia de seguridad en un archivo** escribe un único archivo `.amnotes` con todo dentro, adjuntos
  incluidos, cifrado como sus notas. Se abre con la frase de contraseña que tenía al hacerla, en
  cualquier equipo: elíjalo en **Importar**.
- **Exportar como Markdown** escribe cada nota como un archivo Markdown en una carpeta por libreta, y
  cada proyecto como una lista de comprobación. Estos archivos **no** están cifrados.

Sus notas en sí están en `~/.local/share/amethystora-notes`.
