# Text-to-Speech Subsystem (`server/tts/`)

## 1. Purpose and Scope

The `server/tts/` directory implements the speech synthesis backend for Parallel. It powers conversational spoken interactions and voice calls between users and AI bots. 

Crucially, speech synthesis executes entirely on the **server harness**, never within client renderers:
1. **Credential Protection**: API keys (such as ElevenLabs credentials) never leave the server and are not exposed in browser client JavaScript.
2. **Utterance Processing**: Long agent responses, code blocks, diffs, and formatting artifacts are sanitized and chunked into speakable sentences on the host before hitting audio APIs.
3. **Engine Portability**: The frontend player consumes opaque audio bytes (`audio/mp3` or `audio/wav`), remaining agnostic of whether audio was generated via local system utilities or remote cloud services.

---

## 2. Architecture and Data Flow

```
+-------------------------------------------------------------+
|                     Client Application                      |
|       (Requests /api/bots/:id/speak?text=... or voice call) |
+------------------------------+------------------------------+
                               | HTTP GET/POST
                               v
+-------------------------------------------------------------+
|                      server/tts/index.ts                    |
|  - Reads ~/.parallel/config.json                         |
|  - Validates provider setup (ElevenLabs vs System)          |
|  - Routes to speech synthesis engine                        |
+------------------------------+------------------------------+
                               |
            +------------------+------------------+
            |                                     |
            v                                     v
+-----------------------+             +-----------------------+
|  elevenlabs.ts (Cloud)|             | system-voices.ts (Mac)|
| - Verifies API scopes |             | - macOS /usr/bin/say  |
| - ElevenLabs Flash v2 |             | - 22.05 kHz mono WAV  |
| - 64kbps mono MP3     |             | - Zero-credential     |
+-----------+-----------+             +-----------+-----------+
            |                                     |
            +------------------+------------------+
                               |
                               v
+-------------------------------------------------------------+
|                    speech-text.ts Sanitizer                 |
|  - Drops markdown code blocks & technical formatting        |
|  - Converts technical diffs/paths into natural speech       |
|  - Splits paragraphs into sequential utterance segments     |
+-------------------------------------------------------------+
```

---

## 3. Key Modules and Exported APIs

### 1. `index.ts` (Subsystem Façade)
- **`voiceProvider(cfg: AppConfig): VoiceProvider`**: Resolves whether the configured provider is `"elevenlabs"` or `"system"`.
- **`providerConfigured(cfg: AppConfig): boolean`**: Checks if the required credentials or system binaries are present.
- **`voiceConfigured(cfg: AppConfig): boolean`**: Determines if both a provider and a default voice model are chosen.
- **`listVoices(cfg: AppConfig): Promise<Voice[]>`**: Queries available voices from ElevenLabs or the local macOS catalog.
- **`speak(cfg: AppConfig, text: string, voiceId?: string): Promise<Audio>`**: Main entrypoint to synthesize text into raw audio buffers. Throws `NoVoiceConfigured` when configuration is incomplete.

### 2. `speech-text.ts` (Spoken Register Processing)
Agents write for visual screens: markdown fences, technical syntax, diff blocks, file paths, and ASCII tables. Reading these verbatim produces an awful listening experience.

`speech-text.ts` performs synchronous, deterministic transformations:
- **`speakable(input: string): string`**:
  - Replaces triple-backtick code blocks with verbal descriptors (e.g. `` ```python ... ``` `` becomes `". (a Python code block) "`).
  - Shortens file paths (e.g., `server/drivers/acp/core.ts` becomes `core.ts`).
  - Strips markdown formatting, links, raw URLs, bullet markers, and excessive punctuation.
- **`toUtterances(input: string): string[]`**: Splits conversational prose along sentence and clause boundaries to support sequential pipelined playback.
- **`narrateTool(tool: string, input: unknown): string`**: Converts technical tool invocations (e.g., file reads or bash runs) into concise, human-sounding verbal status updates.

### 3. `elevenlabs.ts` (Cloud TTS Engine)
- Drives the ElevenLabs REST API (`/v1/text-to-speech/{voice_id}`).
- Uses the low-latency `eleven_flash_v2_5` model rendered in `mp3_44100_64` (64 kbps mono).
- **`verifyKey(key: string): Promise<VerifyResult>`**: Checks API keys against `/v1/voices` rather than `/v1/user`, accommodating restricted API keys that lack broad user permissions.

### 4. `system-voices.ts` (Zero-Config macOS Local Engine)
- Available on macOS (`process.platform === "darwin"`).
- Invokes `/usr/bin/say` directly via child process execution.
- Generates 16-bit little-endian 22.05 kHz mono WAV files (`--data-format=LEI16@22050`).
- Parses local system voices using `/usr/bin/say -v ?`.

---

## 4. Security Invariants and Credential Rules

1. **Write-Only Credential Exposure**: `describeVoice(cfg)` returns metadata indicating configuration readiness and voice IDs, but **never echoes back the ElevenLabs API key**.
2. **Platform Gating**: `system-voices.ts` is strictly Darwin-gated. Attempts to invoke system voices on Linux or Windows environments throw a descriptive error or fall back to ElevenLabs.
3. **No Unbounded Memory Buffering**: Utterance synthesis processes individual sentences sequentially, avoiding long-lived memory allocations for large audio streams.

---

## 5. Verification and Testing
TTS sanitization and synthesis pipelines are tested via Vitest:

```bash
# Run all TTS unit tests
pnpm vitest run server/tts/

# Test speech sanitization specifically
pnpm vitest run server/tts/speech-text.test.ts
```
