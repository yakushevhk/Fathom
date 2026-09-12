ARG BASE_IMAGE=parallel:local
FROM ${BASE_IMAGE}
USER root
ARG ENGINES="@anthropic-ai/claude-code @openai/codex opencode-ai @qwen-code/qwen-code @earendil-works/pi-coding-agent"
RUN if [ -n "$ENGINES" ]; then npm install -g $ENGINES; fi

RUN if ! command -v grok >/dev/null 2>&1; then \
      HOME=/opt/grok GROK_BIN_DIR=/opt/grok/bin bash <(curl -fsSL https://x.ai/cli/install.sh) && \
      cp -f /opt/grok/downloads/grok-* /usr/local/bin/grok && \
      chmod a+rx /usr/local/bin/grok && \
      rm -rf /opt/grok; \
    fi

RUN if ! command -v hermes >/dev/null 2>&1; then \
      curl -fsSL https://hermes-agent.nousresearch.com/install.sh | bash -s -- --skip-setup --no-skills && \
      chmod -R a+rx /usr/local/lib/hermes-agent /usr/local/share/uv; \
    fi

COPY scripts/setup-router-agents.mjs /app/scripts/setup-router-agents.mjs
ENTRYPOINT ["docker-entrypoint.sh"]
CMD ["node", "dist-server/index.js"]
