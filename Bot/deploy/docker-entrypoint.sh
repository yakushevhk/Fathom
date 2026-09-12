#!/bin/sh
set -e

# Ensure /data directory exists and has correct ownership for user maus
if [ "$(id -u)" = "0" ]; then
  mkdir -p /data
  chown -R maus:maus /data

  # Run setup script as maus
  if [ -f "/app/scripts/setup-router-agents.mjs" ]; then
    su maus -c "node /app/scripts/setup-router-agents.mjs" || true
  fi

  # Execute command as maus
  exec su maus -s /bin/sh -c "$*"
fi

# Fallback if running directly as non-root
if [ -f "/app/scripts/setup-router-agents.mjs" ]; then
  node /app/scripts/setup-router-agents.mjs || true
fi

exec "$@"
