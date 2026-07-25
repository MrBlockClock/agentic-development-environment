import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
  type WheelEvent as ReactWheelEvent,
} from "react";

type GraphCanvasProps = {
  /** Content size in SVG user units (unscaled). */
  contentWidth: number;
  contentHeight: number;
  className?: string;
  minScale?: number;
  maxScale?: number;
  /** Bump to re-fit — lets the parent bind fit to a keyboard shortcut or relayout. */
  fitToken?: number;
  /**
   * Floor for `Fit`. Fitting a wide graph into a short pane can shrink labels
   * past readability; prefer a legible zoom the user pans over an illegible
   * overview.
   */
  fitMinScale?: number;
  toolbarExtra?: ReactNode;
  children: ReactNode;
};

/**
 * Pan (drag) + zoom (wheel) viewport for Atlas / Plan Map SVG graphs.
 * Toolbar: Fit · Zoom −/+ .
 */
export function GraphCanvas({
  contentWidth,
  contentHeight,
  className = "",
  minScale = 0.35,
  maxScale = 2.5,
  fitToken = 0,
  fitMinScale,
  toolbarExtra,
  children,
}: GraphCanvasProps) {
  const viewportRef = useRef<HTMLDivElement | null>(null);
  const [scale, setScale] = useState(1);
  const [offset, setOffset] = useState({ x: 16, y: 16 });
  const dragRef = useRef<{
    pointerId: number;
    startX: number;
    startY: number;
    originX: number;
    originY: number;
  } | null>(null);

  const clampScale = useCallback(
    (value: number) => Math.min(maxScale, Math.max(minScale, value)),
    [maxScale, minScale],
  );

  const fit = useCallback(() => {
    const el = viewportRef.current;
    if (!el) return;
    const pad = 32;
    const availW = Math.max(el.clientWidth - pad, 80);
    const availH = Math.max(el.clientHeight - pad, 80);
    const raw = Math.min(
      availW / Math.max(contentWidth, 1),
      availH / Math.max(contentHeight, 1),
    );
    const next = clampScale(Math.max(raw, fitMinScale ?? minScale));
    setScale(next);
    setOffset({ x: pad / 2, y: pad / 2 });
  }, [clampScale, contentHeight, contentWidth, fitMinScale, minScale]);

  // Re-fit only on an explicit token bump, so graphs that prefer 100% keep it.
  const lastFitToken = useRef(fitToken);
  useEffect(() => {
    if (fitToken === lastFitToken.current) return;
    lastFitToken.current = fitToken;
    fit();
  }, [fit, fitToken]);

  const onWheel = (event: ReactWheelEvent<HTMLDivElement>) => {
    event.preventDefault();
    const el = viewportRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const mx = event.clientX - rect.left;
    const my = event.clientY - rect.top;
    const factor = event.deltaY > 0 ? 0.9 : 1.1;
    const next = clampScale(scale * factor);
    // Zoom toward cursor
    const worldX = (mx - offset.x) / scale;
    const worldY = (my - offset.y) / scale;
    setScale(next);
    setOffset({
      x: mx - worldX * next,
      y: my - worldY * next,
    });
  };

  const onPointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    const target = event.target as HTMLElement;
    // Let node clicks work; pan from empty canvas / svg background.
    if (target.closest("[data-graph-node]")) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    dragRef.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      originX: offset.x,
      originY: offset.y,
    };
  };

  const onPointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    setOffset({
      x: drag.originX + (event.clientX - drag.startX),
      y: drag.originY + (event.clientY - drag.startY),
    });
  };

  const onPointerUp = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (dragRef.current?.pointerId === event.pointerId) {
      dragRef.current = null;
    }
  };

  return (
    <div className={`flex min-h-[320px] flex-col ${className}`}>
      <div className="flex shrink-0 flex-wrap items-center gap-1 border-b border-white/6 px-2 py-1">
        <span className="mr-1 text-[10px] uppercase tracking-wider text-slate-600">
          View
        </span>
        <button
          type="button"
          onClick={fit}
          className="rounded border border-white/10 bg-white/5 px-2 py-0.5 text-[10px] text-slate-300 hover:bg-white/10"
        >
          Fit
        </button>
        <button
          type="button"
          onClick={() => setScale((s) => clampScale(s / 1.15))}
          className="rounded border border-white/10 bg-white/5 px-2 py-0.5 text-[10px] text-slate-300 hover:bg-white/10"
        >
          −
        </button>
        <button
          type="button"
          onClick={() => setScale((s) => clampScale(s * 1.15))}
          className="rounded border border-white/10 bg-white/5 px-2 py-0.5 text-[10px] text-slate-300 hover:bg-white/10"
        >
          +
        </button>
        <span className="font-mono text-[10px] text-slate-500">
          {Math.round(scale * 100)}%
        </span>
        {toolbarExtra ? (
          <div className="ml-2 flex min-w-0 items-center gap-1.5">{toolbarExtra}</div>
        ) : null}
        <span className="ml-auto hidden text-[10px] text-slate-600 sm:inline">
          Drag empty space to pan · wheel to zoom
        </span>
      </div>
      <div
        ref={viewportRef}
        className="thin-scrollbar relative min-h-[280px] flex-1 cursor-grab overflow-hidden active:cursor-grabbing"
        onWheel={onWheel}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerUp}
      >
        <div
          style={{
            transform: `translate(${offset.x}px, ${offset.y}px) scale(${scale})`,
            transformOrigin: "0 0",
            width: contentWidth,
            height: contentHeight,
          }}
        >
          <svg
            width={contentWidth}
            height={contentHeight}
            className="block"
            style={{ overflow: "visible" }}
          >
            {children}
          </svg>
        </div>
      </div>
    </div>
  );
}
