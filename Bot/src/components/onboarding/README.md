# Onboarding & First-Run Experience

The `onboarding` directory provides the initial setup flow, feature discovery, provider engine setup, and guided tour walkthroughs for newly initialized or freshly onboarded Parallel workspaces.

---

## Directory Architecture

```text
src/components/onboarding/
├── WelcomeFlow.tsx            # Orchestrator card managing beat progression and mascot states
├── view-transition.ts         # View Transitions API helper for morphing card geometry
├── ProgressDots.tsx           # Multi-beat pagination indicator
├── Spotlight.tsx              # SVG/canvas backdrop overlay focusing live UI controls
├── GuidedTour.tsx             # Interactive, multi-step UI spotlight walkthrough
├── FirstConversationTour.tsx  # Post-onboarding interactive prompt suggestions
├── PhonePreview.tsx           # Visual companion mobile pairing QR mockup
│
├── beats/                     # Sequential onboarding steps ("beats")
│   ├── shared.tsx             # Layout wrappers, staggered entrance animations, and buttons
│   ├── HelloBeat.tsx          # Initial greeting and mascot introduction
│   ├── EnginesBeat.tsx        # Local CLI detection, provider sign-in, and API key entry
│   ├── PermissionsBeat.tsx    # OS-level microphone, notifications, and accessibility grants
│   ├── PhoneBeat.tsx          # Mobile companion pairing intro and QR prompt
│   └── MeetYourBotBeat.tsx    # Initial bot seeding (name, color avatar, and first soul rule)
│
└── reel/                      # Animated capability showcase carousel
    ├── FeatureReel.tsx        # Scene player container with navigation controls
    └── scenes/                # Isometric vector and canvas capability animations
        ├── AgentChat.tsx      # Multi-turn reasoning visualizer
        ├── Automations.tsx    # Cron and routine execution preview
        ├── Channels.tsx       # External chat integration preview
        ├── Hands.tsx          # Computer use / mouse & keyboard control preview
        ├── OrbitingApps.tsx   # Connected SaaS ecosystem preview
        └── Terminal.tsx       # Sandboxed shell execution preview
```

---

## Onboarding Sequence: The "Beats" Pipeline

The onboarding experience is orchestrated as a series of non-blocking, skippable "beats" using the browser's native **View Transitions API** (`view-transition.ts`). The main card container, mascot, and title morph smoothly between states rather than unmounting and cross-fading.

```text
[ HelloBeat ]
      │
      ▼
[ FeatureReel ] ──────────► (Optional visual showcase of agent capabilities)
      │
      ▼
[ EnginesBeat ] ──────────► (Discovers installed CLIs & manages provider authentication)
      │
      ▼
[ PermissionsBeat ] ──────► (Requests macOS/Windows microphone & notification rights)
      │
      ▼
[ PhoneBeat ] ────────────► (Presents companion app pairing options)
      │
      ▼
[ MeetYourBotBeat ] ──────► (Seeds first bot: name, color, and initial soul instructions)
      │
      ▼
[ GuidedTour ] ───────────► (Interactive spotlights pointing at live workspace controls)
```

### 1. `HelloBeat.tsx`
- Welcomes the user with the animated vector mascot (`MausAvatar`).
- Sets the tone and verifies hardware accelerated graphics.

### 2. `FeatureReel.tsx`
- Interactive animation reel demonstrating the 6 core pillars of Parallel:
  - Sandboxed terminal shell tasks.
  - Multi-app tool orchestration.
  - Computer use / direct GUI manipulation.
  - Channels (Telegram/Discord/Slack bridges).
  - Background autonomous routines.
  - Multi-agent collaboration.

### 3. `EnginesBeat.tsx` (Provider Detection & Key Entry)
- Scans `GET /api/instances` to inspect available local CLI engines:
  - Claude Code (`@anthropic-ai/claude-code`)
  - OpenAI Codex (`@openai/codex`)
  - Parallel / Router endpoints (`antigravity`, DeepSeek, Qwen)
- Dynamically renders `EngineSetup` inline under each engine row.
- Direct login commands or API key inputs trigger backend authentication; once authenticated, the status changes to "Ready".
- The guide mascot dynamically reacts: switches to `searching` while probing, `proud` when at least one engine is ready, or `curious` when configuration remains.

### 4. `PermissionsBeat.tsx`
- Requests system-level access:
  - Speech dictation / Microphone.
  - System notifications for completed tasks and waiting approvals.
- Gracefully degrades if the user declines or runs in an environment without hardware devices.

### 5. `MeetYourBotBeat.tsx` (Initial Bot Seeding)
- Personalizes the primary bot that ships with the workspace:
  - Sets display name.
  - Chooses mascot brand accent color (`MAUS_COLORS`).
  - Appends an initial behavioral guideline to the bot's `soul` (`soulWithLine(existing, line)`).
- Saves updates via `PATCH /api/bots/:id`. If the server write fails, the UI does not hard-block; the user is allowed to proceed into the workspace.

---

## Interactive Spotlight Tour (`GuidedTour.tsx`)

Once the initial card flow concludes, `GuidedTour` runs directly on the live workspace interface using targeted SVG masks (`Spotlight.tsx`):

1. **DOM Anchor Targeting**:
   - Queries real UI elements with `[data-tour="..."]` attributes (e.g., `data-tour="tools"`, `data-tour="composer"`, `data-tour="nav-automations"`).
2. **Autonomous UI Navigation**:
   - Rather than passively telling the user to click, the tour clicks and toggles panels autonomously (`press("nav-apps")`, `toggleComputer`, `showRoutines`) so the workspace dynamically reacts.
3. **State Persistence**:
   - Step progress is written to the server configuration (`hintSeenPatch`). If the user reloads the window mid-tour, the tour resumes at the exact step where they left off.
   - Hitting `Escape` or clicking "Skip" permanently dismisses the tour.

---

## Technical Invariants & Error Recovery

- **Non-blocking Skippability**: No failure in onboarding can brick the desktop app. Every beat provides a "Skip" or "Later" bypass.
- **View Transitions Graceful Degradation**: If `document.startViewTransition` is unsupported or `prefers-reduced-motion` is active, the component immediately falls back to synchronous DOM state updates.
- **Soul Line Cap**: Initial instruction inputs in `MeetYourBotBeat` are constrained (`SOUL_LINE_LIMIT = 200` chars) to encourage concise directives.

---

## Verification

To preview or smoke-test onboarding flows in isolation:
```tsx
// Inside developer tools or sandbox test routes:
<WelcomeFlow
  bot={mockBot}
  onDone={() => console.log("Onboarding completed")}
  replay={true}
  initialBeat="engines"
/>
```
