// "About Parallel" — the version you are running and where to go next.
// Small on purpose: the interesting settings live in the settings panel, and
// this exists so a bug report can quote a version number.
import { useEffect, useRef } from "react";

import {
  APP_NAME,
  APP_REPOSITORY,
  DOCS_URL,
  LICENSE_URL,
  RELEASES_URL,
  appVersion,
  openExternalLink,
  platformLabel,
} from "@/lib/app-links";

export function AboutDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const closeRef = useRef<HTMLButtonElement>(null);
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    closeRef.current?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      } else if (event.key === "Tab") {
        const modal = dialogRef.current;
        if (!modal) return;
        const focusable = modal.querySelectorAll<HTMLElement>(
          'button:not([disabled]), [tabindex]:not([tabindex="-1"])'
        );
        if (!focusable.length) return;
        const first = focusable[0];
        const last = focusable[focusable.length - 1];
        if (event.shiftKey && document.activeElement === first) {
          event.preventDefault();
          last.focus();
        } else if (!event.shiftKey && document.activeElement === last) {
          event.preventDefault();
          first.focus();
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  const platform = platformLabel(window.ogb?.platform);

  return (
    <div
      className="modal-backdrop fixed inset-0 z-50 flex items-center justify-center p-6"
      onMouseDown={(event) => event.target === event.currentTarget && onClose()}
    >
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="about-dialog-title"
        className="animate-pop-in w-full max-w-[360px] rounded-2xl border border-hairline/50 bg-panel p-6 text-center shadow-2xl"
      >
        <img src="/app-icon.svg" alt="" width={56} height={56} className="mx-auto size-14" />
        <h2 id="about-dialog-title" className="mt-3 text-[17px] font-semibold text-ink">
          {APP_NAME}
        </h2>
        <p className="mt-1 text-[13px] text-ink-secondary">
          Version {appVersion()}
          {platform ? ` · ${platform}` : ""}
        </p>
        <p className="mt-3 text-[13px] leading-relaxed text-ink-secondary">
          An open-source desktop home for your agents. Apache 2.0 licensed.
        </p>
        <div className="mt-4 flex flex-wrap justify-center gap-x-4 gap-y-1.5 text-[13px]">
          <AboutLink href={APP_REPOSITORY} label="GitHub" />
          <AboutLink href={DOCS_URL} label="Docs" />
          <AboutLink href={RELEASES_URL} label="Releases" />
          <AboutLink href={LICENSE_URL} label="License" />
        </div>
        <button
          ref={closeRef}
          type="button"
          onClick={onClose}
          className="mt-5 w-full rounded-xl bg-raised px-4 py-2 text-[13px] font-medium text-ink hover:brightness-110"
        >
          Close
        </button>
      </div>
    </div>
  );
}

function AboutLink({ href, label }: { href: string; label: string }) {
  return (
    <button
      type="button"
      onClick={() => void openExternalLink(href)}
      className="text-accent hover:underline"
    >
      {label}
    </button>
  );
}
