# Diseño de App Personal Similar a Cue.im (Manus)

**Fecha de investigación:** 17 de octubre de 2026 (actualizado)  
**Objetivo:** Crear una aplicación propia inspirada en [https://cue.im](https://cue.im) (Cue by Manus), para **Linux y Windows**, con UI/UX lo más cercana posible a Cue, soporte de sandbox SPACE (Perplexity, solo Linux) y VMPal (donde aplique), y los cambios solicitados. Core en Rust + GUI desktop. Soporte automático vía GitHub Actions.

---

## 1. Investigación de Cue.im (Observaciones e Inferencias)

### Observaciones (evidencia directa)

- **Qué es:** App standalone de Manus (lanzada ~28 de septiembre 2026 junto a Manus 2.0). Permite crear **agentes personales** con identidad propia.
- **Identidad de cada agente:**
  - Teléfono propio (llamadas y mensajes; número US +1; suscripción separada).
  - Email propio (Google mailbox provisionado + direcciones @bot.cue.im).
  - Wallet / pagos (con presupuestos y aprobaciones; usa Link u otros).
  - Computer / workspace dedicado (cloud sandbox, Ubuntu, configurable).
- **Funcionalidades principales:**
  - Agentes trabajan de forma autónoma después de que el usuario se desconecta.
  - Group chat: varios agentes colaboran en una conversación compartida y se pasan trabajo.
  - Connectors / Routines: integración con servicios (Trip.com, Oura, Gmail, MyFitnessPal, etc.) vía MCP o OAuth.
  - Scan & create: crear agentes escaneando QR (ej. para servicios locales).
  - Casos de uso extensos: bookings & travel, shopping, communication & admin, money management, daily life, work & creativity.
- **Plataformas:** Web, macOS (Apple Silicon), Windows, Android, iOS (en revisión App Store en algunos momentos).
- **Acceso:** Early access con invite code. Gratuito en early access; teléfono de pago separado.
- **Infraestructura:** Comparte infraestructura con Manus (Cascade harness, etc.). Agentes tienen memoria propia.
- **No hay detalles técnicos públicos profundos** (código cerrado, no hay API pública documentada en el sitio).

**Fuentes principales:** cue.im, reviews de CellCog, VMEG, MyClaw, The Next Web, AIHub, etc.

### Inferencias

- El "computer" es un sandbox/VM cloud donde el agente puede ejecutar herramientas, navegar, etc.
- Los connectors son principalmente MCP servers o integraciones OAuth gestionadas.
- La colaboración entre agentes ocurre vía un bus de mensajes / group chat interno.
- Phone y Payment son features de alto riesgo/costo operativo (números, wallets, compliance) que el usuario quiere **excluir**.

### Desconocidos

- Implementación exacta del computer (¿Docker? ¿VM dedicada? ¿API específica?).
- Cómo se implementan las llamadas telefónicas y el wallet internamente.
- Modelo de memoria y persistencia.
- APIs internas o SDK.

---

## 2. Cambios Solicitados vs Cue

| Feature Cue                  | En esta app                                      | Notas |
|-----------------------------|--------------------------------------------------|-------|
| Phone                       | **No**                                           | Excluido explícitamente |
| Wallet / Payment            | **No**                                           | Excluido |
| Email propio                | **Sí** (incorporado)                             | Local o via servicios |
| Computer / workspace        | **Sí**, con vista de computer via **SPACE** (Linux nativo) **o VMPal** (donde esté disponible) | Backend pluggable; SPACE prioritario en Linux |
| Group chat / multi-agente   | **Sí** + bots se hablan entre sí                 | Orquestación interna |
| Crear bots / agentes        | **Sí**                                           | CLI + persistencia |
| Connectors                  | **Sí** + MCP local (.json) + por URL + Composio  | Extensible |
| Skills                      | **Sí** (crear a mano, import URL, auto-aplicar)  | Sistema de skills |
| Proveedores de modelos      | Lista específica + **Grok** (OAuth + API key) + OAuth/API key para ChatGPT/Codex y Claude/Claude Code | Ver sección de proveedores |
| Acceso a internet           | **Servicio local gratuito implementado**         | Sin API keys de pago |
| Memoria                     | **Sí** (por agente + global)                     | Persistente, editable |
| Instrucciones               | **Sí** (system prompt / instrucciones por agente)| Editable en UI |
| Automatizaciones            | **Sí** (schedules + event triggers)              | Como routines de Cue |
| Voz                         | ElevenLabs API para llamadas de voz con el bot   | No phone nativo |
| Ejecución                   | Core Rust + **GUI desktop** (Linux + Windows) que replica UI/UX de Cue + CLI opcional | Builds automáticos vía GitHub Actions |

---

## 3. Arquitectura Propuesta (Rust CLI)

### Principios
- **Linux + Windows first**: binarios nativos para ambas plataformas.
- **Local-first** donde sea posible (datos, memoria, skills, MCP local, internet local).
- **Cross-platform GUI** que replique el look & feel de Cue.
- **Extensible**: plugins via MCP, skills como Markdown/JSON, proveedores pluggable.
- **Seguridad**: permisos por agente, aprobaciones, aislamiento de sandboxes.
- **CI/CD**: GitHub Actions para builds, tests y releases automáticos.

### Componentes principales

1. **Core CLI** (clap + tokio)
2. **Agent Manager**: crear, listar, configurar, ejecutar bots.
3. **Provider Router**: abstracción sobre proveedores de LLM (con OAuth + API key).
4. **MCP Client/Server**: local (stdio + JSON config) y remoto (URL / Streamable HTTP). Usar crate `rmcp` (oficial).
5. **Composio Integration**: vía su API / MCP.
6. **Multi-agent / Groups**: bus de mensajes interno (tokio channels o SQLite + pub/sub) para que bots se hablen.
7. **Email Service**: IMAP/SMTP local o vía Composio/Gmail.
8. **Computer Backend**: SPACE (Linux) + VMPal (opcional).
9. **Skills Engine**: cargar, aplicar, importar.
10. **Internet Access Service**: servicio local gratuito (search + fetch).
11. **Voice (ElevenLabs)**: TTS/STT + Conversational AI / Speech Engine.
12. **Memory System**: memoria por agente + global, persistente.
13. **Instructions / System Prompt**: editable por agente.
14. **Automations Engine**: schedules + event triggers.
15. **Persistence**: SQLite o archivos (config, memoria, skills, conversaciones, automatizaciones).

### Diagrama de alto nivel

```
Usuario (CLI)
    │
    ▼
Agent Orchestrator (Rust)
    ├── Provider Router ──► ChatGPT/Codex (OAuth+API key), Claude/Claude Code (OAuth+API key), Grok (OAuth+API key), OpenRouter, CheaperInference, Ollama (local/cloud), LM Studio
    ├── MCP Manager ──────► Local .json configs (stdio) + Remote URLs
    ├── Composio Client ──► 1000+ toolkits
    ├── Skills Engine ────► Manual / Import URL / Auto-apply
    ├── Memory System ────► Por agente + global
    ├── Instructions ─────► System prompt editable por agente
    ├── Automations ──────► Schedules + event triggers
    ├── Multi-agent Bus ──► Groups + bots talking
    ├── Email Service
    ├── Computer Backend ─► SPACE (Linux) / VMPal
    ├── Local Internet Service (search/fetch gratuito)
    └── ElevenLabs Client ─► Voz / llamadas de voz
```

---

## 4. Detalle de Features

### 4.1 Proveedores para los bots

Todos expuestos de forma unificada (preferentemente OpenAI-compatible donde sea posible):

| Proveedor              | Tipo                  | Autenticación                  | Notas |
|------------------------|-----------------------|--------------------------------|-------|
| ChatGPT / Codex        | OpenAI API            | **OAuth + API key**            | Oficial + Codex CLI/API si aplica |
| Claude / Claude Code   | Anthropic API         | **OAuth + API key**            | + Claude Agent SDK / Claude Code |
| **Grok**               | xAI API               | **OAuth + API key**            | Modelos Grok (grok-beta, etc.) |
| OpenRouter             | OpenAI-compatible     | API key                        | Muchos modelos |
| CheaperInference      | OpenAI/Anthropic compatible | API key                   | Flat-rate, modelos open (Kimi, GLM, DeepSeek, etc.) |
| Ollama local           | OpenAI-compatible     | Ninguna / local                | `http://localhost:11434/v1` |
| Ollama Cloud           | OpenAI-compatible     | API key o signed-in            | `https://ollama.com/v1` |
| LM Studio              | OpenAI-compatible     | Ninguna / local                | `http://localhost:1234/v1` |

**Implementación:** trait `LlmProvider` con método `chat`, streaming, tools. Config por agente o global en `~/.config/agents/config.toml`.

**Autenticación (OAuth + API key):**
- **ChatGPT / Codex**: OAuth (login con cuenta OpenAI) y/o API key clásica. Soportar ambos.
- **Claude / Claude Code**: OAuth (login con cuenta Anthropic) y/o API key. Soportar ambos.
- **Grok**: OAuth (login con cuenta xAI/Grok) y/o API key. Modelos como grok-beta, grok-2, etc.
- Almacenamiento seguro de tokens/keys (Keyring / system credential store en Linux y Windows).

### 4.1.1 Memoria

- **Memoria por agente**: hechos, preferencias, contexto histórico persistente del agente.
- **Memoria global**: hechos compartidos entre agentes (opcional).
- Almacenamiento en SQLite o archivos (JSON/Markdown).
- Editable desde la UI (panel derecho o sección de settings del agente).
- Inyectada automáticamente en el system prompt o como contexto adicional.
- Herramientas: `remember`, `recall`, `forget` (con aprobación opcional).

### 4.1.2 Instrucciones

- **Instrucciones / System Prompt** editable por agente.
- Campo de texto largo en la UI del agente (panel derecho).
- Se aplica en cada conversación.
- Puede incluir plantillas o referenciar skills.
- Persistente por agente.

### 4.1.3 Automatizaciones

- **Schedules**: cron-like (diario, semanal, a una hora específica).
- **Event triggers**: nuevos emails, cambios en calendario, webhooks, mensajes en grupo, etc.
- Similar a las "routines" de Cue.
- Cada automatización tiene: trigger, agente responsable, prompt/acción, y condiciones de aprobación.
- Motor interno (tokio-cron o similar) que dispara las tareas.
- UI para crear, editar, activar/desactivar automatizaciones.

### 4.2 MCP Local (.json) y por URL

- **Local:** leer archivos JSON estilo Claude Desktop / Claude Code:
  ```json
  {
    "mcpServers": {
      "filesystem": {
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-filesystem", "/path"]
      }
    }
  }
  ```
- Spawnear procesos (stdio) con `rmcp` client.
- **Por URL:** Streamable HTTP / SSE MCP servers.
- Crate recomendada: **`rmcp`** (oficial Rust MCP SDK, muy activa).

### 4.3 Composio

- Integrar API de Composio para conectar 1000+ apps (Gmail, Slack, Notion, GitHub, etc.).
- Usar sesiones, tools, OAuth gestionado.
- Puede exponerse también como MCP.
- Docs: https://docs.composio.dev y GitHub ComposioHQ/composio.

### 4.4 Grupos y Bots que se hablan entre sí

- Crear grupos de bots.
- Bus interno de mensajes (tokio mpsc o SQLite queue).
- Un bot puede enviar mensaje a otro o al grupo.
- Orquestación simple: system prompt + tool "send_to_agent" / "message_group".
- Persistencia de conversaciones multi-agente.

### 4.5 Email incorporado

- Soporte IMAP/SMTP (crates: `async-imap`, `lettre` o similares).
- O vía Composio (Gmail connector).
- Cada bot puede tener configuración de email propia o compartida.
- Tools: `read_emails`, `send_email`, `search_inbox`.

### 4.6 Vista de VM con VMPal

**VMPal** (de TablePlus, lanzado ~octubre 2026):
- VM app para Apple Silicon Macs.
- Corre macOS, Windows 11, Ubuntu/Fedora, etc. con GPU.
- **Diseñado para AI agents**: control de pantalla, mouse, teclado, admin sin permisos del host.
- Built-in MCP + CLI (`vmpal`).
- Snapshots, port forwarding, etc.
- Docs: https://vmpal.com , CLI docs, blog "Give your AI agent a whole computer".

**Integración:**
- Detectar si `vmpal` está en PATH.
- Comandos: `vmpal list`, `vmpal exec`, screenshots, control.
- Exponer como tools MCP o tools nativas del agente.
- "Vista": poder mostrar screenshot o conectar a la UI de VMPal (o describir estado).
- Ideal para el "computer" local del bot (aislado).

### 4.7 Skills

- **Crear a mano:** archivos Markdown o JSON en `~/.agents/skills/` o por bot.
- **Importar con URL:** descargar SKILL.md o JSON desde URL y registrar.
- **El bot se la aplica a sí mismo:** inyectar en system prompt o como tool/resource MCP.
- Formato sugerido: similar a skills de Claude Code / Agent skills (Markdown con frontmatter).

### 4.8 ElevenLabs para llamada de voz

- Usar ElevenLabs API:
  - Text-to-Speech (Flash, etc.).
  - Speech-to-Text (Scribe).
  - Conversational AI / Speech Engine / Agents.
  - Outbound calls (con Twilio si se quiere, pero el usuario no quiere phone nativo; se puede hacer voz local o via API).
- Para "llamada de voz con el bot": WebSocket Speech Engine o TTS/STT loop local.
- Crates: llamar REST/WebSocket con `reqwest` + `tokio-tungstenite`.

### 4.9 Servicio local gratuito de acceso a internet

Implementar (o integrar) un servicio local que dé a los bots:
- Web search (sin API keys de pago).
- Fetch / scrape de páginas.
- Opcional: news, etc.

**Opciones:**
- Integrar o empaquetar algo como **wigolo** (local-first web intelligence, MCP).
- MCP Internet Server (ej. Sonesh-2202/mcp-internet).
- Implementar propio en Rust: DuckDuckGo HTML scrape + `reqwest` + `scraper` o `html5ever`.
- SearxNG local (Docker) + API.
- Agent-Reach style (usar CLIs existentes: yt-dlp, etc.).

**Recomendación:** módulo interno Rust + tool `web_search` / `web_fetch` que use fuentes gratuitas (DuckDuckGo, Wikipedia, etc.) + caching local.

---

## 5. Stack Técnico Recomendado (Rust)

- **CLI:** `clap` (derive)
- **Async:** `tokio`
- **HTTP:** `reqwest`
- **JSON/Config:** `serde`, `toml`, `json5`
- **DB:** `rusqlite` o `sqlx` (SQLite)
- **MCP:** `rmcp` (oficial)
- **Process:** `tokio::process`
- **Keychain (Mac):** `security-framework` o `keyring`
- **Email:** `lettre`, `async-imap`
- **Scraping/Search:** `scraper` + `reqwest`
- **Logging:** `tracing`
- **Error:** `anyhow` / `thiserror`

Optimizaciones Mac:
- Compilar con target `aarch64-apple-darwin`.
- Usar Metal si hace falta (poco probable).
- Integración nativa con VMPal y Keychain.

---

## 6. Estructura de Proyecto Sugerida

```
personal-agents/
├── Cargo.toml
├── src/
│   ├── main.rs              # CLI entry
│   ├── cli/                 # Comandos
│   ├── agents/              # Manager, runtime
│   ├── providers/           # LLM adapters
│   ├── mcp/                 # Client manager
│   ├── skills/
│   ├── email/
│   ├── voice/               # ElevenLabs
│   ├── internet/            # Local free service
│   ├── vmpal/               # Bridge
│   ├── composio/
│   └── storage/
├── config/
│   └── example.toml
└── skills/                  # Ejemplos
```

**Comando principal:**
```bash
agents create my-bot --provider ollama --model llama3.1
agents chat my-bot
agents group create work --bots bot1,bot2
agents skills import https://example.com/skill.md
agents mcp add local ./mcp.json
```

---

## 7. Plan de Implementación por Fases

1. **MVP CLI + Providers** (Ollama, OpenAI, Anthropic, OpenRouter, LM Studio).
2. **Agent persistence + chat simple**.
3. **MCP local + URL**.
4. **Skills básicas**.
5. **Multi-agent / groups**.
6. **Local Internet Service**.
7. **Composio**.
8. **Email**.
9. **Computer backends**: VMPal + SPACE client.
10. **GUI desktop** que replica el layout de Cue (sidebar agentes + chat + panel derecho + computer view).
11. **ElevenLabs voice**.
12. **Polish, Mac optimizations, packaging**.

---

## 8. Limitaciones y Próximos Pasos

- No se pudo decompilar la app de Cue (es closed-source). El diseño es **inspirado en comportamiento observado**, no una copia exacta.
- VMPal requiere instalación separada y licencia (one-time).
- ElevenLabs y algunos proveedores requieren API keys (el usuario las configura).
- El servicio de internet local debe respetar ToS de sitios y rate limits.
- Para producción real se necesitarían más tests de seguridad (prompt injection, aislamiento de VMs, etc.).

**Recomendación inmediata:** empezar con el core CLI + providers + MCP usando `rmcp`, y probar con Ollama local + un MCP filesystem.

---

## 9. Referencias Clave

- Cue: https://cue.im
- VMPal: https://vmpal.com
- Composio: https://docs.composio.dev
- MCP Spec: https://modelcontextprotocol.io
- rmcp: https://lib.rs/crates/rmcp
- ElevenLabs: https://elevenlabs.io/docs
- Ollama: https://docs.ollama.com
- LM Studio: https://lmstudio.ai/docs/developer
- CheaperInference: https://cheaperinference.com
- SPACE (Perplexity): https://github.com/perplexityai/SPACE
- SPACE Docs: https://space.docs.perplexity.ai/

---

## 10. Soporte de Sandbox: SPACE (Linux) + VMPal (opcional)

### SPACE (Sandboxed Platform for Agentic Code Execution) — Perplexity — **solo Linux**
- **Repositorio:** https://github.com/perplexityai/SPACE
- **Qué es:** Plataforma open-source (Apache 2.0) de sandboxes basados en **Firecracker microVMs**.
- Diseñada específicamente para ejecución de código de agentes AI de forma segura y aislada.
- Powers Perplexity Computer.
- **Características clave:**
  - MicroVMs aisladas con filesystem propio.
  - Templates de entornos.
  - Lifecycle: crear, pausar, suspender, reanudar, borrar (asíncrono con operation IDs).
  - Ejecución de comandos + streaming de output (SSE / data plane).
  - Warm pools, snapshots, E2B compatibility wrapper.
  - Implementado principalmente en Rust; API HTTP.
- **Documentación:** https://space.docs.perplexity.ai/

**Plataforma:**
- SPACE depende de Firecracker, btrfs, Linux networking, etc. → **solo corre nativamente en Linux**.
- En la app: disponible como backend de Computer **únicamente en Linux**.
- En Windows se puede ofrecer como opción remota (si el usuario tiene un host Linux con SPACE) o deshabilitarla.

**Integración en la app:**
- Backend de "Computer" pluggable: `SpaceComputer` (Linux) | `VmpalComputer` (si está disponible).
- Para SPACE: cliente HTTP en Rust (`reqwest`) para crear sandboxes, sessions de comando, stream output, gestionar lifecycle.
- Vista de computer: terminal streaming + file browser + opcional noVNC/screenshot.

### VMPal (opcional)
- Principalmente para Apple Silicon. Puede mantenerse como backend adicional si el usuario lo tiene, pero no es el foco principal.

El usuario puede elegir por agente o globalmente qué backend de computer usar (con detección automática de plataforma).

---

## 11. UI/UX: Replicar Exactamente la de Cue

Según descripciones públicas y reseñas (incluyendo un análisis detallado de la interfaz desktop):

### Layout observado de Cue (desktop)
- **Panel izquierdo (sidebar):** Lista de **Agentes** (no de chats). Botón "+" para crear Agent o crear Group Chat.
- **Centro:** Área de chat / conversación (con el agente seleccionado o el grupo).
- **Panel derecho:** Área funcional del agente seleccionado:
  - Configuración de identidad (email, etc.).
  - Entrada a su **Computer** (abre vista de escritorio remoto / panel de computer).
  - Otras settings.
  - Posibilidad de eliminar el agente.
- **Computer view:** Panel o ventana que muestra el escritorio del agente (browser abierto, acceso a archivos, terminal). En Cue es un cloud Ubuntu con remote desktop.

### Objetivo de diseño
La UI/UX debe ser **lo más idéntica posible** a Cue porque al usuario le gusta.

**Recomendaciones de implementación:**
- **GUI desktop** prioritaria para **Linux y Windows**.
- Stack sugerido (cross-platform):
  - **Tauri** (Rust backend + web frontend React/Svelte/Solid) → fácil de hacer UI pixel-perfect y builds para Linux + Windows.
  - O **egui** / **Slint** / **Iced** si se quiere 100% Rust nativo.
- Elementos a replicar:
  - Sidebar de agentes con avatares/nombres.
  - Chat central con streaming, tool calls visibles, aprobaciones.
  - Panel derecho de settings + botón "Open Computer".
  - Vista de Computer (streaming de SPACE en Linux + file browser).
  - Group chats.
  - Creación de agentes / grupos.
  - Gestión de skills, MCP, providers, email en settings.
- El CLI sigue existiendo para scripting, pero la experiencia principal es la GUI que se siente como Cue.

**Nota:** No hay screenshots oficiales públicos de alta resolución fácilmente accesibles sin la app (invite-only). El diseño se basa en descripciones textuales de usuarios y marketing. Se recomienda descargar Cue (si se tiene acceso) y hacer capturas para matching preciso de colores, tipografía, espaciados y componentes.

---

## 13. GitHub Actions para Soporte Automático (Linux + Windows)

Se configurará un workflow de GitHub Actions para:

- **Builds automáticos** de binarios nativos:
  - Linux: `x86_64-unknown-linux-gnu` (y opcionalmente `aarch64-unknown-linux-gnu`).
  - Windows: `x86_64-pc-windows-msvc` (y opcionalmente `aarch64-pc-windows-msvc`).
- **Tests** en ambas plataformas.
- **Releases** automáticos (tags → assets de GitHub Releases con binarios + instaladores si aplica).
- **Detección de plataforma** en la app para habilitar SPACE solo en Linux.
- Opcional: matrix builds, caching de cargo, y artefactos firmados.

Ejemplo de estructura de workflow (`.github/workflows/release.yml`):
- Trigger: push a tags `v*` o manual.
- Jobs separados o matrix para Linux y Windows.
- Compilar el crate Rust (y Tauri si se usa).
- Subir los binarios como release assets.

Esto permite que el usuario (o cualquiera) descargue builds listos para Linux y Windows sin necesidad de compilar localmente.

---

## 14. Plan de Implementación Actualizado

1. **MVP Core Rust + Providers** (incluyendo Grok, OAuth+API key para ChatGPT/Codex y Claude).
2. **Agent persistence + chat simple**.
3. **Memoria, Instrucciones y Automatizaciones** básicas.
4. **MCP local + URL**.
5. **Skills básicas**.
6. **Multi-agent / groups**.
7. **Local Internet Service**.
8. **Composio**.
9. **Email**.
10. **SPACE client** (solo Linux) + Computer view básica.
11. **GUI desktop** (Tauri o nativo) que replica el layout de Cue (Linux + Windows).
12. **GitHub Actions** para builds y releases automáticos.
13. **ElevenLabs voice**.
14. **Polish, packaging e instaladores**.

---

## 15. Arquitectura Actualizada (GUI + Core)

```
┌─────────────────────────────────────────────────────────┐
│  GUI Desktop (Tauri / egui / etc.) — Linux + Windows    │
│  - Sidebar Agents                                       │
│  - Chat center                                          │
│  - Right panel (settings + Computer)                    │
│  - Computer view (SPACE en Linux)                       │
└──────────────────────┬──────────────────────────────────┘
                       │ IPC / commands
┌──────────────────────▼──────────────────────────────────┐
│  Core Rust (tokio)                                      │
│  - Agent Orchestrator                                   │
│  - Provider Router                                      │
│  - MCP Manager                                          │
│  - Composio                                             │
│  - Skills Engine                                        │
│  - Multi-agent Bus                                      │
│  - Email / Voice (ElevenLabs)                           │
│  - Local Internet Service                               │
│  - Computer Backend: SpaceClient (Linux) | VmpalBridge  │
└─────────────────────────────────────────────────────────┘
```

---

**Fin del documento de diseño e investigación.**  
Este archivo puede usarse como blueprint para implementar la aplicación en Rust.
