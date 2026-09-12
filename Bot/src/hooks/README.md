# Frontend Custom Hooks (`src/hooks/`)

## 1. Directory Purpose & Scope

The `src/hooks/` directory houses specialized, cross-cutting React hooks tailored for the desktop renderer and browser interface. 

The primary responsibility within this package is resolving **native view occlusion**—managing the interaction boundary between the React DOM layer and underlying Electron `WebContentsView` instances used for high-performance computer control and browser automation.

---

## 2. Core Hook: `useNativeViewObscured`

### 2.1 The Problem Space
In Electron applications that utilize native `WebContentsView` or OS-level native child windows (such as CUA displays, VNC viewports, or local VM streams), these native surfaces always paint directly into the OS compositing pipeline **above** the Chromium DOM tree. 

CSS rules like `z-index` have zero effect on native views. If a user opens a React modal, command palette, dropdown menu, or drawer that overlaps the native viewport bounds, the native view would punch through the UI, rendering the React dialog partially or completely invisible.

### 2.2 Mechanism & Implementation Details

```
+---------------------------------------------------------------------------------+
|                                 React DOM Tree                                  |
|                                                                                 |
|   [ Positioned Overlays: dialog, modal, drawer, popover, z-fixed ]              |
|                                     |                                           |
|                                     v                                           |
|                +-----------------------------------------+                      |
|                |         useNativeViewObscured           |                      |
|                |  - MutationObserver (overlay detection) |                      |
|                |  - ResizeObserver (geometry changes)    |                      |
|                |  - Scroll / Window Event Listeners      |                      |
|                +--------------------+--------------------+                      |
|                                     |                                           |
|                     Bounding Box Intersection Test                              |
|                 (aspectFitNativeViewBounds vs. Overlays)                        |
|                                     |                                           |
|                                     v                                           |
|                 obscured: boolean (true = hide/pause view)                      |
|                                     |                                           |
|                                     v                                           |
|                         Electron Native Host View                               |
|                     (WebContentsView / CUA Surface)                             |
+---------------------------------------------------------------------------------+
```

#### Selector Targeting
`useNativeViewObscured` dynamically scans the DOM for candidate overlays using:
```ts
const EXPLICIT_OVERLAY_SELECTOR =
  '[aria-modal="true"], [role="dialog"], [role="menu"], [popover], [data-native-view-overlay]';

const POSITIONED_OVERLAY_SELECTOR =
  `${EXPLICIT_OVERLAY_SELECTOR}, [class*="fixed"], [class*="absolute"]`;
```

#### Target Lifecycle Synchronization (`syncNativeViewResizeTargets`)
To prevent memory leaks and unneeded layout thrashing, the hook uses `syncNativeViewResizeTargets`:
- Tracks an active `Set` of observed DOM nodes.
- Attaches the shared `ResizeObserver` to newly appeared overlay candidates and unobserves elements when they leave the DOM or change roles.

#### Geometry Calculation
- Computes the aspect-ratio-fitted bounding box of the native view container via `aspectFitNativeViewBounds`.
- Tests for spatial intersection with any visible candidate overlay using `nativeViewOverlayIntersects`.
- Evaluates computed style properties (`position`, `z-index`, `visibility`, `display`) to ensure only truly elevated, opaque layers trigger occlusion.

---

## 3. Related Application Hooks Catalog

While `src/hooks/` contains hooks focused on native view lifecycle, several domain-specific custom hooks are located across `src/components/` and `src/lib/` following collocation principles. Developers and agents should leverage the following standard hooks:

| Hook | Location | Responsibility |
|---|---|---|
| `useStore` | `src/state/store.tsx` | Accesses global `AppState` and action `dispatch`. |
| `useStreaming` | `src/state/store.tsx` | Subscribes to per-frame assistant and reasoning token streams. |
| `useDesktopCapabilities`| `src/components/DesktopCapabilities.tsx` | Provides feature flags from the Electron desktop preload bridge (`window.ogb`). |
| `useComposerDraft` | `src/lib/drafts.ts` | State binding for uncommitted text drafts and attachments. |
| `useFocusMessage` | `src/lib/focus-message.ts` | Automatically scrolls to and highlights target messages from search hits. |
| `useBottomFollowResize`| `src/lib/bottom-follow.ts` | Manages automatic scroll-to-bottom pinning during active message streaming. |
| `usePageVisible` | `src/lib/page-visible.ts` | Suspends unnecessary background screenshot polling when the tab is hidden. |
| `useMcpServers` | `src/lib/mcp-servers.ts` | Synchronizes active MCP server status and tool registries. |
| `useSpeech` | `src/lib/tts/useSpeech.ts` | Real-time audio player state and speaking indicator tracking. |

---

## 4. Invariants & Lifecycle Constraints

1. **Avoid Layout Thrashing in Obscurity Checks**:
   - Never call `getBoundingClientRect()` unconditionally in tight render loops. `useNativeViewObscured` must rely on `ResizeObserver` callbacks and debounced scroll/event listeners.
2. **Cleanup of Observers**:
   - All `MutationObserver` and `ResizeObserver` instances must be disconnected and their observed target sets cleared in the hook cleanup phase (`useLayoutEffect` return).
3. **Aspect Ratio Fitting**:
   - Always supply the native view's true aspect ratio when calling `useNativeViewObscured`. Failure to do so can result in false positive occlusions on empty letterboxed container margins.

---

## 5. Verification & Testing

The hook behavior and target synchronization logic are tested using Vitest:
- Run the hook test suite:
  ```bash
  pnpm vitest run src/hooks/use-native-view-obscured.test.ts
  ```
- Validates:
  - Correct observation of newly mounted overlay nodes.
  - Immediate unobservation of stale or unmounted overlay elements.
  - Correct idempotency when receiving identical candidate sets.
