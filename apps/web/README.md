# Fathom Web Dashboard

Next.js 16 control plane and visual dashboard for the **Fathom** autonomous AI worker platform.

## Overview

The web dashboard provides real-time monitoring and control over the Fathom engine:
- **Chat & Steering**: Interactive task execution with live Server-Sent Events (SSE) streaming (`/sessions/:id/events`).
- **Coworkers Console**: Manage autonomous worker personas, scheduled cron jobs, and communication channels.
- **Governance & Policy Engine**: Inspect security policies, approve or reject gated tool executions, and review audit trails.
- **Credentials Vault**: Secure credential management for external APIs and MCP integrations.
- **Computer Use Panel**: Live interactive viewport for Playwright loopback automation sessions.
- **Observability & Memory**: Explore long-term semantic knowledge graphs, facts, and token usage metrics.

## Getting Started

### Prerequisites
- Node.js 20+ (or Bun / pnpm)
- Running Fathom server daemon (`fathom serve --port 8080`)

### Installation & Development

```bash
# Install dependencies
npm install

# Run the development server
npm run dev
```

Open [http://localhost:3000](http://localhost:3000) in your browser.

## Environment Configuration

Configure connection settings in `.env.local`:

```env
# URL to the Fathom Rust HTTP backend
NEXT_PUBLIC_FATHOM_API_URL=http://localhost:8080

# Optional API key for authenticated server deployments
NEXT_PUBLIC_FATHOM_API_KEY=your_api_key_here
```

## Architecture & Routes

- `/` — Live chat, agent tree visualizer, and prompt execution.
- `/coworkers` — Multi-agent fleet management, channel bindings, and schedule definitions.
- `/governance` — Policy configuration and audit logs.
- `/credentials` — Secrets and API keys vault.
- `/jobs` — Durable background jobs status and retry inspection.
- `/memories` — Knowledge base search and memory graph exploration.
