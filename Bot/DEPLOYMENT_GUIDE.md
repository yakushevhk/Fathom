# Parallel & AI Agents Infrastructure Guide

Полное руководство по архитектуре, конфигурации, развертыванию, AI-движкам и виртуальным компьютерам (Local VM) для Parallel на сервере `code.y7.hk`.

---

## 1. Общая архитектура системы

```text
                                [ Интернет / Клиенты ]
                                          │
                        (HTTPS 443 / WSS, порт 80 -> 443)
                                          ▼
                      ┌────────────────────────────────────────┐
                      │    Nginx (aaPanel Reverse Proxy)       │
                      │   SSL: Let's Encrypt (/root/.acme.sh)  │
                      └───────────────────┬────────────────────┘
                                          │
                ┌─────────────────────────┼────────────────────────┐
                │                         │                        │
         http://127.0.0.1:8799     http://127.0.0.1:8800    http://127.0.0.1:6080
         (Web UI, REST, SSE)       (Webhooks / Ingress)     (noVNC Desktop Viewer)
                │                         │                        │
                ▼                         ▼                        ▼
    ┌───────────────────────────────────────────┐    ┌───────────────────────────┐
    │       Docker: parallel (Harness)          │    │  Docker: parallel-        │
    │  network_mode: "host"                     │    │          computer (VM)    │
    │  Volumes:                                 │    │  XFCE4 + Cua Driver       │
    │   - /opt/parallel-data:/data              │    │  noVNC Web Viewer (:6901) │
    │   - /var/run/docker.sock                  │    │  Workspace:               │
    │   - /data/.parallel                       │    │   /data/.parallel/        │
    │                                           │    │    vm-home                │
    │  AI Engines:                              │    └───────────────────────────┘
    │   - opencode (Helium API router)          │
    │   - hermes (Nous Research ACP)            │
    │   - pi (Pi Coding Agent JSON-RPC)         │
    │   - claude (Anthropic CLI)                │
    │   - grok (xAI CLI ACP)                    │
    │   - qwen (Qwen Code ACP)                  │
    │   - antigravity (Google ACP runtime)      │
    └─────────────────────┬─────────────────────┘
                          │ (OpenAI-compatible protocol)
                          ▼
            https://router.y7.hk/v1
            Key: sk-helium-18aba15209499c35
            Models: Gemini 3.8/3.7, Claude Sonnet 4.6,
                    Qwen 3.8 Max, DeepSeek V4, Grok 4.6, etc.
```

---

## 2. Доступ и ссылки

* **Основная панель управления (Web UI / PWA):**  
  `https://code.y7.hk/`
* **Страница подключения нового устройства (Pairing):**  
  `https://code.y7.hk/pair#code=<PAIR_CODE>`
* **Прямой просмотр экрана виртуального компьютера (noVNC):**  
  `https://code.y7.hk/vnc/vnc.html#autoconnect=true&resize=scale&password=admin`
* **Healthcheck API:**  
  `https://code.y7.hk/api/health`
* **Local VM Status API:**  
  `https://code.y7.hk/api/local-computer`

---

## 3. Расположение файлов и директорий

| Путь | Назначение |
| :--- | :--- |
| `/opt/Parallel/deploy/` | Репозиторий деплоя, `docker-compose.yml`, `.env` |
| `/opt/parallel-data/` | Постоянный том данных контейнера (`/data` в контейнере) |
| `/opt/parallel-data/.parallel/` | База данных `messages.db`, `config.json`, сессии, ключи |
| `/opt/parallel-data/.parallel/vm-home/` | Рабочая папка виртуального компьютера ботов |
| `/opt/parallel-data/.local/bin/` | Установленные бинарники движков (`pi`, `hermes`, `claude`, `grok`, `qwen`, `antigravity`, `docker`, `uv`) |
| `/opt/parallel-data/.opencode/bin/` | Бинарник `opencode` |
| `/opt/parallel-data/.pi/agent/` | Конфигурация Pi (`models.json`, `auth.json`, `settings.json`) |
| `/opt/parallel-data/.hermes/` | Конфигурация Hermes (`config.yaml`, `.env`) |
| `/opt/parallel-data/.qwen/` | Конфигурация Qwen Code (`settings.json`) |
| `/opt/parallel-data/.grok/` | Конфигурация Grok (`config.toml`, `auth.json`) |
| `/www/server/panel/vhost/nginx/code.y7.hk.conf` | Конфигурация Nginx виртуального хоста и проксирования |
| `/www/server/panel/vhost/cert/code.y7.hk/` | SSL-сертификаты Let's Encrypt |

---

## 4. Настройка Nginx и SSL

Конфигурационный файл `/www/server/panel/vhost/nginx/code.y7.hk.conf`:
* Проксирует `80` на `443` (HTTP -> HTTPS).
* Проходит верификацию ACME через `/.well-known/acme-challenge/` в `/www/server/nginx/html`.
* Проксирует `/hooks/` на `http://127.0.0.1:8800`.
* Проксирует `/vnc/` и `/websockify` на `http://127.0.0.1:6080/` с поддержкой WebSocket.
* Проксирует корень `/` на `http://127.0.0.1:8799` с отключенной буферизацией (`proxy_buffering off`) и поддержкой Server-Sent Events (SSE) и WebSockets.

Сертификаты Let's Encrypt выпускаются через `acme.sh`:
```bash
/root/.acme.sh/acme.sh --issue -d code.y7.hk -w /www/server/nginx/html --server letsencrypt
```

---

## 5. Docker Compose окружение

Файл `/opt/Parallel/deploy/docker-compose.yml`:
* `network_mode: "host"` — Parallel слушает `127.0.0.1:8799` прямо на сетевом стеке хоста.
* Монтирование томов:
  * `/opt/parallel-data:/data` (данные, конфиги, домашняя папка пользователя `parallel`)
  * `/opt/parallel-data/dist-server:/app/dist-server` (патч для подстановки публичного URL VNC)
  * `/var/run/docker.sock:/var/run/docker.sock` (доступ к Docker хоста для управления VM)
* `group_add: ["986"]` — доступ к Docker сокету.
* Переменные окружения для провайдера Helium:
  * `OPENAI_API_KEY: "sk-helium-18aba15209499c35"`
  * `OPENAI_BASE_URL: "https://router.y7.hk/v1"`
  * `OPENROUTER_API_KEY: "sk-helium-18aba15209499c35"`
  * `OPENROUTER_BASE_URL: "https://router.y7.hk/v1"`
  * `ANTHROPIC_API_KEY: "sk-helium-18aba15209499c35"`
  * `ANTHROPIC_BASE_URL: "https://router.y7.hk/v1"`
  * `OPENCODE_API_KEY: "sk-helium-18aba15209499c35"`
  * `QWEN_API_KEY: "sk-helium-18aba15209499c35"`
  * `XAI_API_KEY: "sk-helium-18aba15209499c35"`

---

## 6. Конфигурация AI-движков

### 6.1. OpenCode
* Бинарник: `/data/.opencode/bin/opencode` (v1.18.30)
* Конфигурация: `/data/.config/opencode/opencode.jsonc`
* Учетные данные: `/data/.local/share/opencode/auth.json` (`helium api key`)

### 6.2. Hermes Agent (Nous Research)
* Бинарник: `/data/.local/bin/hermes`, `hermes-acp` (v0.19.0)
* Установлен через `uv tool install hermes-agent` с автономным Python 3.14
* Конфиг: `/data/.hermes/config.yaml`
```yaml
model:
  provider: custom
  base_url: https://router.y7.hk/v1
  name: antigravity/gemini-3.8-flash-high
providers:
  router.y7.hk:
    base_url: https://router.y7.hk/v1
    api_key: sk-helium-18aba15209499c35
```
* Секреты: `/data/.hermes/.env` (`OPENAI_API_KEY`, `OPENAI_BASE_URL` и т.д.)

### 6.3. Pi Coding Agent
* Бинарник: `/data/.local/bin/pi` (v0.85.1, `@earendil-works/pi-coding-agent`)
* Конфиг моделей: `/data/.pi/agent/models.json` (провайдер `helium`, роутер `https://router.y7.hk/v1`)
* Конфиг настроек: `/data/.pi/agent/settings.json` (default: `antigravity/gemini-3.8-flash-high`)
* Авторизация: `/data/.pi/agent/auth.json`

### 6.4. Qwen Code
* Бинарник: `/data/.local/bin/qwen` (v0.23.3, `@qwen-code/qwen-code`)
* Конфиг: `/data/.qwen/settings.json`
```json
{
  "authType": "openai",
  "openaiBaseUrl": "https://router.y7.hk/v1",
  "openaiApiKey": "sk-helium-18aba15209499c35",
  "model": "alibaba/qwen3.8-max"
}
```

### 6.5. xAI Grok CLI
* Бинарник: `/data/.local/bin/grok` (v1.0.25)
* Конфиг: `/data/.grok/config.toml` (секции `[model.gemini]`, `[model.claude]`, `[model.qwen]`)
* Авторизация: `/data/.grok/auth.json`

### 6.6. Google Antigravity
* Официальный исполняемый файл: `/data/.local/bin/agy_acp_server.par`, `localharness_external`
* Установлен в `/data/.parallel/tools/antigravity-acp/linux-x64/versions/38f62d01b32deb0907b3d39a71ec301fd36369f6ffd1cf262d4af385177f79df/`

### 6.7. Claude Code
* Бинарник: `/data/.local/bin/claude` (v2.1.269, `@anthropic-ai/claude-code`)

---

## 7. Виртуальные компьютеры ботов (Local VM / Cua)

### 7.1. Как устроен компьютер бота
1. **Базовый образ:** `docker.io/trycua/xfce-cua@sha256:274eb636f5cf3fc58f705916ee72b7a701270b3877369d08533a385c5325be9b`
2. **Финальный образ:** `localhost/parallel/cua-local-vm:driver-0.20.0-v5`
   * Содержит XFCE4 Desktop, Chromium, `cua-driver 0.20.0` и японские шрифты Noto CJK.
   * Драйвер `cua-driver` запускается supervisor-ом и слушает сокет `/run/user/1000/parallel-cua.sock`.
   * noVNC сервер слушает внутри порт `6901` и проброшен на хост `127.0.0.1:6080`.
3. **Безопасность контейнера:**
   * `--cap-drop ALL --cap-add SETUID --cap-add SETGID`
   * Память: 4 GB, CPUs: 2, PIDs limit: 512, shm-size: 512m.
   * Рабочая папка: `/home/cua/workspace` смонтирована в `/data/.parallel/vm-home`.

### 7.2. Команды управления контейнером VM
```bash
# Перезапустить контейнер компьютера
docker restart parallel-computer

# Посмотреть логи виртуального компьютера
docker logs --tail 50 parallel-computer

# Проверить статус доступности через API
curl -s http://127.0.0.1:8799/api/local-computer | jq .

# Сделать скриншот с экрана бота
curl -s -X POST http://127.0.0.1:8799/api/local-computer/screenshot
```

---

## 8. Полезные сервисные команды

```bash
# Сгенерировать новый код сопряжения для браузера / телефона (5 минут)
docker exec openmausbot node dist-server/openmausbot.js pair --public-url https://code.y7.hk

# Посмотреть активные сессии
docker exec openmausbot node dist-server/openmausbot.js sessions

# Перезапустить Parallel
docker compose -f /opt/Parallel/deploy/docker-compose.yml restart omb

# Посмотреть логи Parallel
docker logs -f --tail 50 openmausbot

# Проверить синтаксис и перезагрузить Nginx
/www/server/nginx/sbin/nginx -t && /www/server/nginx/sbin/nginx -s reload
```
