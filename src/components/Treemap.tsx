import { useCallback, useEffect, useRef, useState } from "react";
import { colorFor, formatBytes, Rect } from "../lib/types";

interface Props {
  rects: Rect[];
  onZoom: (id: number) => void;
  onResize: (w: number, h: number) => void;
}

interface Hover {
  rect: Rect;
  mx: number;
  my: number;
}

const GAP = 1.5;
const INK = "#e9e6df";
const DIM = "#8f948d";

export default function Treemap({ rects, onZoom, onResize }: Props) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [hover, setHover] = useState<Hover | null>(null);

  // ---- resize observer: tell the parent our pixel budget
  useEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const observer = new ResizeObserver((entries) => {
      const { width, height } = entries[0].contentRect;
      if (width > 0 && height > 0) onResize(width, height);
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, [onResize]);

  // ---- draw
  const draw = useCallback(() => {
    const canvas = canvasRef.current;
    const wrap = wrapRef.current;
    if (!canvas || !wrap) return;
    const dpr = window.devicePixelRatio || 1;
    const { clientWidth: w, clientHeight: h } = wrap;
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
    canvas.style.width = `${w}px`;
    canvas.style.height = `${h}px`;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.scale(dpr, dpr);
    ctx.clearRect(0, 0, w, h);

    for (const rect of rects) {
      const x = rect.x + GAP / 2;
      const y = rect.y + GAP / 2;
      const rw = Math.max(rect.w - GAP, 0.5);
      const rh = Math.max(rect.h - GAP, 0.5);
      const hovered = hover?.rect.id === rect.id && hover?.rect.name === rect.name;

      ctx.fillStyle = colorFor(rect);
      ctx.globalAlpha = rect.is_dir ? 1 : hovered ? 1 : 0.92;
      ctx.fillRect(x, y, rw, rh);
      ctx.globalAlpha = 1;

      if (rect.is_dir) {
        // recessed directory: hairline border, contents implied
        ctx.strokeStyle = hovered ? "#e0a458" : "#3a3f47";
        ctx.lineWidth = hovered ? 1.5 : 1;
        ctx.strokeRect(x + 0.5, y + 0.5, rw - 1, rh - 1);
      } else if (hovered) {
        ctx.strokeStyle = INK;
        ctx.lineWidth = 1;
        ctx.strokeRect(x + 0.5, y + 0.5, rw - 1, rh - 1);
      }

      // labels only where they fit - never clipped mush
      if (rw > 64 && rh > 26) {
        ctx.font = "600 11px ui-monospace, 'JetBrains Mono', 'Cascadia Mono', monospace";
        ctx.fillStyle = rect.is_dir ? "#c9c5ba" : "#15171b";
        const label = truncate(ctx, rect.name, rw - 14);
        ctx.fillText(label, x + 7, y + 16);
        if (rh > 44) {
          ctx.font = "10px ui-monospace, monospace";
          ctx.fillStyle = rect.is_dir ? DIM : "#2c2f35";
          ctx.fillText(formatBytes(rect.size), x + 7, y + 30);
        }
      }
    }
  }, [rects, hover]);

  useEffect(() => {
    draw();
  }, [draw]);

  useEffect(() => {
    const handler = () => draw();
    window.addEventListener("resize", handler);
    return () => window.removeEventListener("resize", handler);
  }, [draw]);

  // ---- interaction
  const hit = (event: React.MouseEvent): Rect | null => {
    const bounds = canvasRef.current!.getBoundingClientRect();
    const mx = event.clientX - bounds.left;
    const my = event.clientY - bounds.top;
    for (const rect of rects) {
      if (mx >= rect.x && mx <= rect.x + rect.w && my >= rect.y && my <= rect.y + rect.h) {
        return rect;
      }
    }
    return null;
  };

  return (
    <div className="treemap-wrap" ref={wrapRef}>
      <canvas
        ref={canvasRef}
        onMouseMove={(event) => {
          const rect = hit(event);
          const bounds = canvasRef.current!.getBoundingClientRect();
          setHover(
            rect
              ? { rect, mx: event.clientX - bounds.left, my: event.clientY - bounds.top }
              : null
          );
        }}
        onMouseLeave={() => setHover(null)}
        onClick={(event) => {
          const rect = hit(event);
          if (rect && rect.is_dir && rect.id !== 4294967295) onZoom(rect.id);
        }}
        style={{ cursor: hover?.rect.is_dir ? "pointer" : "default" }}
      />
      {hover && (
        <div
          className="tooltip"
          style={{
            left: Math.min(hover.mx + 14, (wrapRef.current?.clientWidth ?? 400) - 240),
            top: hover.my + 14,
          }}
        >
          <div className="tooltip-name">{hover.rect.name}</div>
          <div className="tooltip-meta">
            {formatBytes(hover.rect.size)} · {(hover.rect.frac * 100).toFixed(1)}% of view
            {hover.rect.is_dir ? " · click to enter" : ""}
          </div>
        </div>
      )}
    </div>
  );
}

function truncate(ctx: CanvasRenderingContext2D, text: string, max: number): string {
  if (ctx.measureText(text).width <= max) return text;
  let lo = 0;
  let hi = text.length;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (ctx.measureText(text.slice(0, mid) + "…").width <= max) lo = mid;
    else hi = mid - 1;
  }
  return text.slice(0, lo) + "…";
}
