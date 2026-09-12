#!/bin/sh
set -e

# Ensure /data directory exists and has correct ownership for user parallel (or maus)
if [ "$(id -u)" = "0" ]; then
  TARGET_USER="parallel"
  if ! id "$TARGET_USER" >/dev/null 2>&1; then
    TARGET_USER="maus"
  fi
  mkdir -p /data
  chown -R "$TARGET_USER:$TARGET_USER" /data

  # Remove any stale lease left by previous container stop/restart
  rm -f /data/.openmausbot/openmausbot-server.lease /data/.parallel/openmausbot-server.lease 2>/dev/null || true

  # Run setup script as target user
  if [ -f "/app/scripts/setup-router-agents.mjs" ]; then
    su "$TARGET_USER" -c "node /app/scripts/setup-router-agents.mjs" || true
  fi

  # Execute command as target user
  exec su "$TARGET_USER" -s /bin/sh -c "$*"
fi
# Fallback if running directly as non-root
rm -f /data/.openmausbot/openmausbot-server.lease /data/.parallel/openmausbot-server.lease 2>/dev/null || true
if [ -f "/app/scripts/setup-router-agents.mjs" ]; then
  node /app/scripts/setup-router-agents.mjs || true
fi

exec "$@"
