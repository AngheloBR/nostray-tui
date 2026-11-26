# nostray

Temporizador Pomodoro para la terminal, pensado para mantener el enfoque en
cualquier actividad: programar, estudiar, practicar inglés, leer… TUI con
cronómetro grande, sesiones tipo, categorías, meta diaria, distribución del
tiempo, racha y un heatmap de actividad al estilo GitHub. Incluye una CLI
mínima para arrancar directo o ver un resumen.

[English](README.md)

## Capturas (en texto)

Generadas con el mismo código que dibuja la TUI (`TestBackend` de ratatui) y
datos ficticios. En la terminal real los dígitos, las barras y el heatmap van en
color (degradado violeta → cian; verde en los descansos y al cumplir la meta).

Terminal de 80×24, foco en curso con una tarea activa:

```
 nostray v0.1.0                        Clásico · Programar · ciclo 1/4  ? ayuda
╭ Timer ───────────────────────────────────────────────────────────────────────╮
│                        ██████ ██████    ██████ ██████                        │
│                            ██ ██     ██ ██  ██ ██  ██                        │
│                        ██████ ██████    ██  ██ ██  ██                        │
│                        ██         ██ ██ ██  ██ ██  ██                        │
│                        ██████ ██████    ██████ ██████                        │
│                                                                              │
│             ● ENFOQUE   Programar · Escribir parser de ejemplo 1/4           │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━   0%  │
╰──────────────────────────────────────────────────────────────────────────────╯
╭ Hoy ─────────────────────────────────────────────────────────────────────────╮
│ Meta ▰▰▰▰▰▰▰▰▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱▱  12%  1/8 │
│ 1 pomodoro · 25 min de foco                                                  │
╰──────────────────────────────────────────────────────────────────────────────╯
╭ Distribución · hoy ───────────────────────────────────────────── Tab ‹ 1/4 › ╮
│ Programar ▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰▰ 100%    25 min │
│                                                                              │
╰──────────────────────────────────────────────────────────────────────────────╯
  espacio  iniciar   c  categoría   Tab  panel   ?  ayuda   q  salir
```

Panel «Racha y actividad» (cada columna es una semana, de lunes a domingo; la
intensidad del color va de 0 a 4 según los pomodoros del día):

```
╭ Racha y actividad ───────────────────────────────────────╮
│ Racha 3 días  ·  mejor 5 días                            │
│   sep     oct     nov     dic       ene     feb     mar  │
│ L ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■  │
│ M ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■  │
│ X ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■  │
│ J ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■  │
│ V ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■  │
│ S ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■  │
│ D ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■    │
│ 15 pomodoros · 27 semanas            menos ■ ■ ■ ■ ■ más │
╰──────────────────────────────────────────────────────────╯
```

## Instalación

Necesitas Rust y Cargo (el `Cargo.toml` declara `rust-version = "1.88"`; se ha
desarrollado y probado con 1.100). Desde la raíz del repositorio:

```sh
cargo install --path .
```

Instala el binario `nostray` en `~/.cargo/bin` (asegúrate de tenerlo en el
`PATH`). Para los avisos de escritorio hace falta `notify-send` (paquete
`libnotify` en la mayoría de distribuciones); sin él, la app funciona igual y
avisa solo en pantalla.

## Uso

```sh
nostray                                   # TUI con el selector de sesión
nostray start                             # TUI directa con la sesión Clásico
nostray start --session deep --tag Inglés # sesión y categoría elegidas
nostray stats                             # resumen en texto y sale
nostray --help | --version
```

- `--session`: `clasico`, `deep`, `flow` o `personalizado` (por defecto `clasico`).
- `--tag`: una de las categorías de `config.toml`, sin distinguir mayúsculas.
  Si no existe, `nostray` lo indica, lista las válidas y sale con código 2.
- La TUI necesita una terminal interactiva; `stats` no.

Ejemplo de `nostray stats` (datos ficticios):

```
nostray · resumen al 25-11-2026
Hoy      3/8 pomodoros (37 %) · 75 min de foco
7 días   5 pomodoros · 150 min de foco
         Programar   33 %     50 min
         Estudiar    33 %     50 min
         Inglés      17 %     25 min
         Leer        17 %     25 min
Racha    3 días · mejor 3 días
```

## Sesiones

| Sesión        | Enfoque | Descanso | Descanso largo            |
|---------------|---------|----------|---------------------------|
| Clásico       | 25 min  | 5 min    | 15 min tras 4 focos       |
| Deep work     | 50 min  | 10 min   | —                         |
| Flow          | 90 min  | 20 min   | —                         |
| Personalizado | 1–180   | 1–60     | 1–90 min tras 1–12 focos  |

La sesión Personalizado se edita en la TUI (`m` → Personalizado → `e`) o en
`config.toml`. Al terminar una fase, la siguiente queda en espera hasta que
pulses espacio. Cambiar de sesión con un ciclo en curso pide confirmación y lo
reinicia.

## Categorías

Cada foco se asigna a una categoría (por defecto: Programar, Estudiar, Inglés,
Leer, Otro). Se cambia con `c` y se guarda la vigente al cerrar el ciclo. La
lista se edita en `config.toml`: de 1 a 8 nombres únicos de 1 a 20 caracteres.
El historial guarda el nombre tal cual, así que una categoría que quites de la
configuración sigue contando en las estadísticas.

## Paneles

- **Timer**: dígitos grandes, estado (enfoque / descanso / pausa / en espera),
  categoría, tarea activa y barra de progreso.
- **Hoy**: porcentaje de la meta diaria (por defecto 8 pomodoros) con barra,
  pomodoros y minutos de foco. Puede pasar de 100 %.
- **Distribución**: minutos y porcentaje por categoría, hoy o últimos 7 días
  (`v` con el panel enfocado). Los porcentajes suman 100.
- **Racha y actividad**: racha actual y mejor racha (días consecutivos con al
  menos un pomodoro; la actual sigue viva hasta que acaba el día) y heatmap de
  hasta 53 semanas, tantas como quepan. Nivel de cada día respecto a la meta:
  con meta 8, 1–2 → nivel 1, 3–4 → 2, 5–6 → 3, 7 o más → 4.
- **Tareas** (opcional): pomodoros hechos/estimados, tarea activa; cada foco
  completado suma uno a la tarea activa.
- **Log**: una línea por fase cerrada (hora, sesión, categoría o descanso,
  tarea, duración, ✓ o «saltado»), la más reciente arriba.

Solo cuentan para las estadísticas los focos completados; los saltados y los
descansos aparecen en el Log pero no suman.

### Tamaños de terminal

| Ancho   | Disposición                                                        |
|---------|--------------------------------------------------------------------|
| < 100   | Una columna: Timer y Hoy fijos; el resto apilado si cabe, o en un hueco que rota con `Tab` |
| 100–139 | Dos columnas: Timer, Hoy, Distribución · Actividad, Tareas, Log    |
| ≥ 140   | Dos columnas; dígitos a escala grande si hay altura                |

Por debajo de 40×12 se muestra un aviso de tamaño mínimo.

## Atajos

| Tecla         | Acción                                         |
|---------------|------------------------------------------------|
| espacio       | iniciar · pausar · reanudar                    |
| `s`           | saltar la fase actual                          |
| `r`           | reiniciar la fase actual                       |
| `m`           | cambiar de sesión                              |
| `c`           | cambiar de categoría (flechas o `1`–`8`)       |
| `Tab` / `Shift+Tab` | panel siguiente / anterior (rota los ocultos) |
| `?`           | ayuda con todos los atajos                     |
| `q`           | salir (pide confirmación si hay un ciclo en curso) |
| `Ctrl+C`      | salir ya                                       |

Con el panel enfocado:

| Panel        | Teclas                                                              |
|--------------|---------------------------------------------------------------------|
| Tareas       | `a` alta · `e` editar · `x` borrar · `⏎` activa (o la desmarca) · `+`/`-` estimado (1–20) · `↑↓` mover |
| Log          | `↑↓` · RePág/AvPág · Inicio/Fin                                     |
| Distribución | `v` hoy / 7 días                                                    |

La terminal se restaura al salir de forma normal, con `Ctrl+C`, con
SIGTERM/SIGHUP/SIGINT y también si el programa entra en pánico.

## Avisos

Al terminar sola una fase de foco o de descanso, `nostray` lanza
`notify-send --app-name=nostray -- <título> <cuerpo>` (sin pasar por una
shell). Saltar una fase no genera aviso. Si `notify-send` no está instalado, se
avisa una vez en la TUI y se sigue sin notificaciones de escritorio. No hay
sonido.

## Datos

| Archivo                                       | Contenido               |
|-----------------------------------------------|-------------------------|
| `$XDG_DATA_HOME/nostray/history.json`         | historial de fases      |
| `$XDG_DATA_HOME/nostray/tasks.json`           | tareas                  |
| `$XDG_CONFIG_HOME/nostray/config.toml`        | configuración           |

Si no están definidas las variables XDG, se usan `~/.local/share/nostray/` y
`~/.config/nostray/`.

- Cada escritura es atómica (archivo temporal + `sync` + renombrado).
- Si un archivo no se puede leer (JSON/TOML corrupto o de una versión
  desconocida) no se borra: se renombra a `<archivo>.bak` (o `.bak.1`, `.bak.2`…),
  se arranca con ese archivo vacío y se avisa en pantalla.
- En `config.toml`, un valor fuera de rango se sustituye por el de por defecto
  solo en memoria (con aviso); el archivo no se toca.
- Si `config.toml` no existe, se crea con valores por defecto y comentarios.

`config.toml` por defecto:

```toml
# nostray: configuración

# Pomodoros por día (1–24).
daily_goal = 8

# Categorías de enfoque (1–8, únicas, 1–20 caracteres).
categories = ["Programar", "Estudiar", "Inglés", "Leer", "Otro"]

# Sesión Personalizado, en minutos.
[custom]
focus_min = 45        # 1–180
break_min = 10        # 1–60
long_break_min = 20   # 1–90
cycles = 3           # 1–12: descanso largo tras N focos
```

Para probar sin tocar tus datos:

```sh
XDG_DATA_HOME=/tmp/nostray-prueba/data XDG_CONFIG_HOME=/tmp/nostray-prueba/config nostray
```

## Renombrar el programa

El nombre sale de una sola constante, `APP_NAME` en `src/main.rs`; de ella
derivan las rutas de datos, el título y los textos. Para renombrar:

1. Cambia `APP_NAME` en `src/main.rs`.
2. Cambia `name` en la sección `[[bin]]` de `Cargo.toml` al mismo valor (un test,
   `app_name_coincide_con_binario`, falla si no coinciden).
3. Opcional: el `name` del paquete y `repository` en `Cargo.toml`.

Los datos guardados con el nombre anterior no se migran: quedan en su carpeta
antigua.

## Limitaciones de la v1

- La duración de las fases se mide con el reloj monotónico del sistema, que en
  Linux no avanza mientras el equipo está suspendido: una fase en curso durante
  una suspensión termina más tarde.
- Sin bloqueo entre instancias: si abres dos a la vez, la última que guarda gana.
- Al salir con un ciclo a medias, el tiempo de esa fase no se guarda.
- Cuando la TUI guarda la configuración (al editar la sesión Personalizado),
  `config.toml` se reescribe sin los comentarios.
- `nostray stats` usa la misma carga que la TUI: si falta `config.toml` lo crea
  y si un archivo está corrupto lo renombra a `.bak`.
- Desarrollado y probado en Linux (Fedora, X11). Fuera de alcance en esta
  versión: sonido, integración con git, sincronización, red, Wayland, temas
  configurables, plugins y exportación.

## Desarrollo

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Los tests usan un reloj falso (sin esperas reales), directorios temporales
dentro de `target/` y renderizan cada panel con `TestBackend`.

## Licencia

Bajo tu elección de [MIT](LICENSE-MIT) o [Apache-2.0](LICENSE-APACHE).
