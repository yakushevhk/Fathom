ARG BASE_IMAGE=parallel:local
FROM ${BASE_IMAGE}
USER root
ARG ENGINES="@anthropic-ai/claude-code @openai/codex opencode-ai @qwen-code/qwen-code @earendil-works/pi-coding-agent"
RUN if [ -n "$ENGINES" ]; then npm install -g $ENGINES; fi

RUN if ! command -v grok >/dev/null 2>&1; then \
      curl -fsSL https://x.ai/cli/install.sh | HOME=/opt/grok GROK_BIN_DIR=/usr/local/bin bash && \
      chmod a+rx /usr/local/bin/grok /usr/local/bin/agent 2>/dev/null || true; \
    fi

RUN if ! command -v hermes >/dev/null 2>&1; then \
      curl -fsSL https://hermes-agent.nousresearch.com/install.sh | bash -s -- --skip-setup --no-skills && \
      chmod -R a+rx /usr/local/lib/hermes-agent /usr/local/share/uv; \
    fi

COPY scripts/setup-router-agents.mjs /app/scripts/setup-router-agents.mjs
COPY deploy/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod +x /usr/local/bin/docker-entrypoint.sh
ENTRYPOINT ["docker-entrypoint.sh"]
CMD ["node", "dist-server/index.js"]
