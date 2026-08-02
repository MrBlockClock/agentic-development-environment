import { useEffect, useState } from "react";

export type ToastTone = "info" | "ok" | "error";

export type Toast = {
  id: string;
  tone: ToastTone;
  message: string;
};

type Listener = (toast: Toast) => void;

const listeners = new Set<Listener>();

/** Fire-and-forget ephemeral UI notice (attach/save/connect). */
export function pushToast(input: {
  message: string;
  tone?: ToastTone;
}): void {
  const toast: Toast = {
    id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    tone: input.tone ?? "info",
    message: input.message.trim(),
  };
  if (!toast.message) return;
  for (const listener of listeners) listener(toast);
}

const TONE_CLASS: Record<ToastTone, string> = {
  info: "border-white/12 bg-[#121820] text-slate-200",
  ok: "border-emerald-400/25 bg-emerald-500/10 text-emerald-100",
  error: "border-amber-400/30 bg-amber-500/10 text-amber-100",
};

/**
 * Fixed corner stack for ephemeral feedback. Mount once near App root.
 */
export function ToastHost() {
  const [toasts, setToasts] = useState<Toast[]>([]);

  useEffect(() => {
    const onToast: Listener = (toast) => {
      setToasts((prev) => [...prev.slice(-4), toast]);
      window.setTimeout(() => {
        setToasts((prev) => prev.filter((item) => item.id !== toast.id));
      }, 4_500);
    };
    listeners.add(onToast);
    return () => {
      listeners.delete(onToast);
    };
  }, []);

  if (toasts.length === 0) return null;

  return (
    <div
      className="pointer-events-none fixed bottom-4 right-4 z-50 flex w-[min(22rem,calc(100vw-2rem))] flex-col gap-2"
      aria-live="polite"
      aria-relevant="additions"
    >
      {toasts.map((toast) => (
        <div
          key={toast.id}
          role={toast.tone === "error" ? "alert" : "status"}
          className={`pointer-events-auto rounded-lg border px-3 py-2 text-[11px] leading-4 shadow-lg ${TONE_CLASS[toast.tone]}`}
        >
          {toast.message}
        </div>
      ))}
    </div>
  );
}
