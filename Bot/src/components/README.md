# Components Architecture (`src/components/`)

## 1. Directory Purpose & Scope

The `src/components/` directory contains all reusable and view-level React 19 components powering Parallel's user interface. These components handle transcript visualization, message composition, agent and channel navigation, computer control interfaces, multi-pane workspaces, workflow graphing, and configuration dialogs.

All components follow a local-first, optimistic interaction model with zero ungrounded state, integrating seamlessly with the global state management in `src/state/` and platform abstractions in `src/lib/`.

---

## 2. Architectural Structure & Component Hierarchy

```
+---------------------------------------------------------------------------------+
|                                     App Shell                                   |
|                                                                                 |
|  +---------------------+  +--------------------------------------------------+  |
|  |       Sidebar       |  |                 Viewport Container               |  |
|  |  - BotPickerList    |  |                                                  |  |
|  |  - SidebarThreadRow |  |  +--------------------------------------------+  |  |
|  |  - BotProjects      |  |  | Single: ChatView | GroupView               |  |  |
|  |  - SearchResults    |  |  | Dual:   SplitWorkspace (SplitRatio resizer)|  |  |
|  |  - SidebarProfile   |  |  | Flow:   TeamMapPage | RoutinesPage        |  |  |
|  +---------------------+  |  +--------------------------------------------+  |  |
|                           +-------------------------+------------------------+  |
|                                                     |                           |
|                                                     v                           |
|                           +--------------------------------------------------+  |
|                           |            Docked & Drawer Sidecars              |  |
|                           |  - ComputerPanel / RemoteDesktopPanel (CUA/VNC)  |  |
|                           |  - InspectorPanel (Live Turn Diagnostics)        |  |
|                           |  - McpServersPanel / McpToolInspector            |  |
|                           +--------------------------------------------------+  |
+---------------------------------------------------------------------------------+
|                               Floating Modals & Overlays                        |
|  - CommandPalette          - SettingsModal         - NewBotDialog                |
|  - MemoryManagerModal      - DagWorkflowVisualizer - SwarmCapabilityMatrix      |
|  - KeyboardShortcutsModal  - PluginsPanel          - CallView / GroupCallView    |
+---------------------------------------------------------------------------------+
```

---

## 3. Key View & Interaction Components

### 3.1 `ChatView.tsx` & `GroupView.tsx` (Conversation & Orchestration)
- **Role**: Primary conversation interfaces. `ChatView` handles 1:1 interactions between an operator and a bot; `GroupView` handles multi-agent team channels.
- **Transcript Virtualization / Windowing**: Implements `TRANSCRIPT_WINDOW_SIZE` from `src/lib/transcript-window.ts` to clamp mounted DOM elements, preventing performance degradation in deep transcripts with extensive inline images.
- **Interactive Message Cards**:
  - `ApprovalCard.tsx`: Surfaces tool execution confirmation requests from the agent harness. Supports single-call approval, session approval, and permanent "always allow" policies.
  - `VerifyCard.tsx`: Visualizes step-by-step agent verifications, unit test runs, and test outputs.
  - `OptionCard.tsx`: Interactive single/multi-choice structured prompts.
  - `ConnectorCard.tsx` & `SecretRequestCard.tsx`: Handles on-demand OAuth and API credential prompts required by tool drivers mid-execution.
  - `RoutineRunCard.tsx` & `GoalRunCard.tsx`: Displays execution telemetry for autonomous background routines and multi-step agent goals.
- **Bottom-Follow Scroll Dynamics**: Uses `useBottomFollowResize` (`src/lib/bottom-follow.ts`) to maintain pinned-to-bottom auto-scroll only while the operator has not scrolled up to inspect history.

### 3.2 `Composer.tsx` & Input Subsystem
- **Role**: High-fidelity prompt, command, and attachment input bar.
- **Draft Persistence**: Backed by `src/lib/drafts.ts`, maintaining per-bot and per-thread uncommitted drafts across navigation and failed send retries.
- **Mention Engine**: `MentionTextarea.tsx` and `src/lib/mentions.ts` allow `@bot` tagging within group rooms, with fuzzy auto-completion and role badge previews.
- **Slash Commands**: Parses `/goal`, `/steer`, `/clear`, `/call` via `src/lib/composer-commands.ts`.
- **Multimodal Attachment Pipeline**: Integrates `ComposerAttachments.tsx`, `AttachmentPreview.tsx`, and `src/lib/image-compress.ts` for handling dragged files, pasted screenshots, and clipboard image payloads.

### 3.3 `SplitWorkspace.tsx` (Multi-Pane Workspace)
- **Role**: Enables side-by-side simultaneous execution and monitoring of two agents or channels.
- **Features**:
  - Interactive pointer-capture divider adjusting `splitRatio` (clamped between 0.2 and 0.8).
  - Synchronized focus pane management (`primary` vs. `secondary`) for keyboard shortcut targeting.
  - Independent transcript scrolling and composer lifecycles in each pane.

### 3.4 `Sidebar.tsx` & Navigation Subsystem
- **Role**: Navigation tree managing bots, groups, project folders, and system sections.
- **Folder Drag-and-Drop**: Supports arbitrary sorting and hierarchical organization via `src/lib/folder-order.ts`.
- **Thread Hierarchy**: `SidebarThreadRow.tsx` displays active tasks, unread indicators, run state, and parent bot associations.
- **Search & Global Palette**: `SearchResults.tsx` and `CommandPalette.tsx` provide fast full-text search across transcripts, bots, and actions.

### 3.5 `ComputerPanel.tsx`, `RemoteDesktopPanel.tsx` & CUA Viewer
- **Role**: Operator viewport for agent computer interaction (Computer Use Agent / CUA).
- **Multi-Backend Support**:
  - **Local Control**: Native macOS and Linux desktop automation displays (`MacLocalControl.tsx`, `LinuxLocalControl.tsx`).
  - **Cloud Box**: Remote sandbox environments provisioned via cloud backends (`CloudScreenPreview.tsx`, `CloudBackendPicker.tsx`).
  - **Browser Panel**: Embedded browser session control (`BrowserPanel.tsx`, `BrowserViewport.tsx`, `BrowserProfilesManager.tsx`).
  - **Android USB Subsystem**: Direct physical or emulated device control (`AndroidDevicePanel.tsx`).
- **Human Takeover / Lease Management**: Implements lease acquisition via `src/lib/computer-control.ts`, allowing operators to lock the bot out and drive manually via input events.

### 3.6 Workflow, Memory, & Swarm Visualizers
- **`DagWorkflowVisualizer.tsx`**: Renders DAG workflow execution paths, task dependencies, and execution states.
- **`MemoryManagerModal.tsx`**: Interface for inspecting, searching, editing, and wiping durable bot memories stored across semantic namespaces.
- **`SwarmCapabilityMatrix.tsx`**: Displays capability coverage and role assignments across configured bots.
- **`MermaidViewer.tsx`**: Renders live Mermaid.js architectural diagrams generated in agent assistant responses.
- **`KaTeXMath.tsx`**: Fast math formatting using KaTeX for scientific and engineering reasoning output.

---

## 4. Invariants & UI Patterns

1. **Memoization & Re-render Boundaries**:
   - High-churn streaming components (such as `ChatView` message rows) use `React.memo` and scoped context selectors (`useStreaming()`) to prevent entire tree re-renders during token streaming.
2. **Accessible Interaction & Haptics**:
   - All interactive controls trigger appropriate tactile feedback on supported platforms via `triggerHaptic()` (`src/lib/haptics.ts`).
3. **Occlusion & Desktop Preload Awareness**:
   - Floating panels and dialogs must coordinate with `useNativeViewObscured` to avoid visual collisions with Electron native WebContentsView overlays.
4. **Draft Safety**:
   - Unsent composer input must never be wiped without explicit operator confirmation. Network errors or process interruptions must retain the input via `useFailedComposerSends`.

---

## 5. Verification & Testing

Component tests are written using Vitest with React Testing Library:
- Run all component tests:
  ```bash
  pnpm vitest run src/components/
  ```
- Target specific component domains:
  ```bash
  pnpm vitest run src/components/ChatView.controls.test.ts
  pnpm vitest run src/components/ComputerPanel.test.ts
  pnpm vitest run src/components/GroupView.test.ts
  ```
