# 🥤 Cola — Enhanced Real-Time Log Viewer

`Cola` es un reemplazo enriquecido de `tail -f` escrito en Rust. Analiza archivos de logs (como Nginx, Apache o Syslog/SSH) en tiempo real, geolocaliza direcciones IP para mostrar banderas del país de origen en formato emoji unicode, resalta palabras clave con colores ANSI y cuenta con filtros avanzados interactivos al vuelo.

Está diseñado para ser altamente eficiente, ligero y totalmente autocontenido mediante compilaciones 100% estáticas.

---

## ✨ Características

*   **Monitoreo en Tiempo Real**: Sigue la escritura de archivos de log de forma continua de manera eficiente sin sobrecargar la CPU, soportando rotaciones de logs y truncados.
*   **Geolocalización de IPs**: Detecta IPs IPv4 y consulta su origen en milisegundos usando una base de datos MaxMind GeoIP2 local (`.mmdb`), traduciendo los códigos de país en emojis de banderas.
*   **Caché en Memoria**: Implementa una caché concurrente thread-safe para evitar consultas repetitivas de IP a disco y optimizar el rendimiento.
*   **Auto-detección Sticky**: Detecta el formato de log al vuelo (Nginx Combined, Apache Common/Combined y Syslog/SSH) y bloquea el parser para maximizar el desempeño en streams rápidos.
*   **Filtros Interactivos**: Te permite marcar y desmarcar filtros clave en tiempo real usando teclas rápidas durante el stream de logs.
*   **Filtros por Consola**: Soporta filtros de inclusión (`-i`) y exclusión (`-e`) avanzados mediante expresiones regulares (Regex).

---

## 🚀 Requisitos Previos (Geolocalización)

Para que el resolvedor de IPs pinte las banderas, necesitas una copia local de la base de datos MaxMind GeoLite2-Country en formato `.mmdb`.

Hemos incluido un script automático para descargarla e instalarla en la ruta por defecto:

```bash
chmod +x download_geoip.sh
./download_geoip.sh
```

El script colocará la base de datos en `~/.cola/GeoLite2-Country.mmdb` (donde `Cola` la buscará automáticamente al iniciar).

---

## 📦 Instalación

### Opción A: Instalación Global (Requiere `sudo`)
Mueve el binario a una ruta del PATH del sistema para poder invocarlo desde cualquier parte escribiendo simplemente `cola`:

```bash
sudo cp cola /usr/local/bin/
sudo chmod +x /usr/local/bin/cola
```

### Opción B: Instalación Local (Sin `sudo`)
Si no dispones de accesos de administrador en el servidor remoto:

```bash
mkdir -p ~/.local/bin
cp cola ~/.local/bin/
chmod +x ~/.local/bin/cola
```

Asegúrate de tener `~/.local/bin` en tu variable `$PATH` dentro de tu `.bashrc` o `.zshrc`:
```bash
export PATH="$HOME/.local/bin:$PATH"
```

---

## 💡 Controles y Filtros Interactivos

Mientras `Cola` está imprimiendo logs en tu terminal, puedes presionar teclas para activar/desactivar filtros instantáneamente:

| Tecla | Acción | Descripción |
| :---: | :--- | :--- |
| **`1`** | **Ocultar Estáticos** | Oculta peticiones a imágenes, fuentes, archivos `.css`, `.js`, etc. |
| **`2`** | **Mostrar Solo Errores** | Muestra solo respuestas HTTP `>= 400` u operaciones fallidas de SSH. |
| **`3`** | **Ocultar Bots** | Filtra accesos de rastreadores conocidos (Googlebot, Bingbot, etc.). |
| **`4`** | **Mostrar Solo SSH** | Muestra de manera exclusiva las líneas originadas por `sshd`. |
| **`h`** | **Ayuda** | Imprime un menú de ayuda interactiva en la pantalla actual. |
| **`Ctrl+C`**| **Salir y Estadísticas** | Detiene el programa mostrando el rendimiento y hit-rate de la caché de IPs. |

---

## 🛠️ Instrucciones de Uso (CLI)

### Monitoreo básico (con auto-detección de formato)
```bash
cola /var/log/nginx/access.log
```

### Especificar líneas iniciales a leer (equivalente a tail -n)
Por defecto lee las últimas 10 líneas existentes antes de empezar a monitorizar:
```bash
cola /var/log/nginx/access.log -n 30
```

### Forzar un formato específico
Opciones: `auto` (por defecto), `nginx`, `apache`, `syslog`.
```bash
cola /var/log/auth.log -f syslog
```

### Desactivar geolocalización (modo sin banderas)
```bash
cola /var/log/nginx/access.log --no-geo
```

### Filtrado estático por línea de comando
*   **Inclusión** (Mostrar solo IPs o mensajes coincidentes):
    ```bash
    cola /var/log/nginx/access.log -i "POST"
    ```
*   **Exclusión** (Ocultar líneas coincidentes):
    ```bash
    cola /var/log/nginx/access.log -e "127.0.0.1"
    ```
*   **Combinado**:
    ```bash
    cola /var/log/nginx/access.log -i "/api/" -e "Googlebot"
    ```

---

## ⚙️ Compilación y Distribución

Para empaquetar una nueva versión de distribución desde tu máquina de desarrollo, ejecuta el script de empaquetado:

```bash
./build_dist.sh
```

Esto compilará el código de forma estática con el target `x86_64-unknown-linux-musl` y generará un archivo comprimido listo para producción llamado **`cola-dist.tar.gz`**, que contiene:
*   `cola`: El binario estático ejecutable portable.
*   `download_geoip.sh`: Script instalador de la base de datos GeoIP.
*   `README.md`: Este manual de uso.
