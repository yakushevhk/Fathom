# Harness Core (`server/harness/`)

## 1. Purpose and Scope

The `server/harness/` directory provides the fundamental runtime plumbing connecting the server's HTTP routing and state layers to the various provider driver instances. It abstracts multi-provider management into two core components:
1. **Provider Registry (`registry.ts`)**: Manages the lifecycle, configuration decoding, discovery, and instantiation of provider instances.
2. **Event Bus (`bus.ts`)**: Multiplexes disparate asynchronous event streams from all active provider instances into a single broadcast stream, tees events to disk logs, and strips sensitive credentials.

---

## 2. Architecture and Data Flow

```
+-------------------------------------------------------------+
|                      Server Config                          |
|             (~/.parallel/config.json -> instances)       |
+------------------------------+------------------------------+
                               |
                               v load(configs)
+-------------------------------------------------------------+
|               ProviderRegistry (registry.ts)                |
|  - byId: Map<InstanceId, RegistryEntry>                     |
|  - Live Instances vs. Shadow Instances                      |
|  - Engine binary installations & CLI resolution             |
+------------------------------+------------------------------+
                               | attach(instances)
                               v
+-------------------------------------------------------------+
|                    EventBus (bus.ts)                        |
|  - Verifies event origin invariant                          |
|  - Redacts sensitive credentials (redactSecrets)            |
|  - Appends to NDJSON log (~/.parallel/events/<id>.ndjson) |
|  - Delivers to in-memory listeners (SSE / message-db)       |
+------------------------------+------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|                     Downstream Consumers                    |
|           - SSE Fan-Out (/api/events)                       |
|           - Message Store / Thread Transcripts              |
|           - Comms & Activity Mirroring                      |
+-------------------------------------------------------------+
```

---

## 3. Key Components and Exported Classes

### `ProviderRegistry` (`registry.ts`)
The `ProviderRegistry` maintains the active pool of AI provider instances configured for the application.

#### Key Functions and Methods
- `load(configs: InstanceConfigMap)`: Dynamically spins up, updates, or reconfigures driver instances from disk config without requiring a server reboot.
- `get(instanceId: InstanceId)`: Returns the live `ProviderInstance` or undefined.
- `describe()`: Generates a serialized snapshot of all configured instances (including model catalogs, health status, auth state, and custom CLI overrides) for consumption by the UI.
- `dispose(instanceId: InstanceId)`: Gracefully tears down a specific provider instance without disrupting sibling instances.
- `installEngine(instanceId: InstanceId)`: Automates installation of underlying CLI dependencies via npm or managed package channels.

#### The Shadow Instance Invariant
If an instance refers to an unknown driver kind (e.g. from an experimental or newer version of Parallel) or fails configuration decoding, `registry.ts` **never throws a fatal error**. Instead, it instantiates a **`ShadowInstance`**:
```typescript
export interface ShadowInstance {
  instanceId: InstanceId;
  driverKind: string;
  displayName: string | undefined;
  cli: string | undefined;
  shadow: true;
  reason: string;
}
```
This invariant ensures forward and backward configuration compatibility; users can downgrade their server version without losing configuration settings for newer engines.

---

### `EventBus` (`bus.ts`)
The `EventBus` aggregates event streams across all provider adapters into a single, ordered pipeline.

#### Key Functions and Methods
- `attach(instances: ProviderInstance[])`: Subscribes to provider adapter event emitters and maps them to the bus.
- `detach(instanceId: string)`: Detaches and unsubscribes a specific provider instance.
- `publish(event: RuntimeEvent)`: Enforces origin invariants, redacts credentials, persists the event to disk, and broadcasts to memory subscribers.
- `subscribe(listener: RuntimeEventListener)`: Registers a callback for real-time event updates.

#### Key Invariants of the Event Bus
1. **Driver Origin Isolation**: An adapter is strictly prohibited from emitting events under a different `driverKind`. If an event's `provider` tag does not match the instance's registered driver kind, the event is dropped with an error.
2. **Credential Redaction**: Before writing any event to disk, the event passes through `redactSecrets()`. Tool invocations, request summaries, and model prompts have API keys, tokens, and authorization headers masked.
3. **Canonical Event Logging**: Every thread writes to an NDJSON log file (`EVENTS_DIR/<threadId>.ndjson`) with restrictive file permissions (`0o600`).
4. **Resilient Degradation**: If an event cannot be persisted to disk (e.g., due to disk space or filesystem permissions), the bus logs a warning and continues delivering in-memory events to active listeners rather than crashing the process.

---

## 4. Verification and Testing
Harness components are covered by targeted Vitest unit tests validating registry isolation and event bus persistence:

```bash
# Run harness tests
pnpm vitest run server/harness/registry.test.ts
pnpm vitest run server/harness/bus.test.ts
```
