import {
  useCallback,
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
    const next = clampScale(
      Math.min(availW / Math.max(contentWidth, 1), availH / Math.max(contentHeight, 1)),
    );
    setScale(next);
    setOffset({ x: pad / 2, y: pad / 2 });
  }, [clampScale, contentHeight, contentWidth]);

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
        <span className="ml-auto text-[10px] text-slate-600">
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
