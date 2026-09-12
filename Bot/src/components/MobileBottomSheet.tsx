import React, { useEffect, useRef } from "react";
import { X } from "lucide-react";
import { triggerHaptic } from "@/lib/haptics";

interface BottomSheetProps {
  open: boolean;
  onClose: () => void;
  title: string;
  children: React.ReactNode;
}

export function MobileBottomSheet({ open, onClose, title, children }: BottomSheetProps) {
  const sheetRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (open) triggerHaptic("medium");
  }, [open]);

  if (!open) return null;

  return (
    <div
      role="dialog"
      aria-modal="true"
      onClick={onClose}
      className="modal-backdrop fixed inset-0 z-50 flex items-end sm:hidden bg-black/60 backdrop-blur-xs transition-opacity animate-fade-in"
    >
      <div
        ref={sheetRef}
        onClick={(e) => e.stopPropagation()}
        className="flex max-h-[85vh] w-full flex-col overflow-hidden rounded-t-3xl border-t border-hairline/60 bg-panel shadow-2xl animate-slide-up"
      >
        {/* Grab Handle */}
        <div className="flex w-full items-center justify-center pt-3 pb-1">
          <div className="h-1.5 w-12 rounded-full bg-hairline-strong/80" />
        </div>

        {/* Sheet Header */}
        <div className="flex items-center justify-between border-b border-hairline/40 px-5 py-3">
          <h3 className="text-base font-semibold text-ink">{title}</h3>
          <button
            type="button"
            onClick={onClose}
            className="flex size-7 items-center justify-center rounded-full bg-raised text-ink-secondary hover:text-ink"
          >
            <X size={16} />
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-5 pb-8">
          {children}
        </div>
      </div>
    </div>
  );
}
