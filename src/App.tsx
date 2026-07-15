import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import Treemap from "./components/Treemap";
import Sidebar from "./components/Sidebar";
import {
  BigFile,
  Crumb,
  formatBytes,
  formatCount,
  Rect,
  ScanSummary,
  TypeSlice,
} from "./lib/types";

type Phase = "idle" | "scanning" | "ready";

export default function App() {
  const [phase, setPhase] = useState<Phase>("idle");
  const [pickedPath, setPickedPath] = useState<string>("");
  const [progress, setProgress] = useState({ files: 0, bytes: 0 });
  const [summary, setSummary] = useState<ScanSummary | null>(null);
  const [node, setNode] = useState(0);
  const [rects, setRects] = useState<Rect[]>([]);
  const [crumbs, setCrumbs] = useState<Crumb[]>([]);
  const [largest, setLargest] = useState<BigFile[]>([]);
  const [types, setTypes] = useState<TypeSlice[]>([]);
  const [error, setError] = useState<string>("");
  const viewport = useRef({ w: 0, h: 0 });

  // ---------------------------------------------------------- scan events
  useEffect(() => {
    const subscriptions = [
      listen<{ files: number; bytes: number }>("scan-progress", (event) =>
        setProgress(event.payload)
      ),
      listen<ScanSummary>("scan-done", (event) => {
        setSummary(event.payload);
        setPhase("ready");
        setNode(0);
      }),
      listen<string>("scan-error", (event) => {
        setError(event.payload);
        setPhase("idle");
      }),
    ];
    return () => {
      subscriptions.forEach((sub) => sub.then((unlisten) => unlisten()));
    };
  }, []);

  // ------------------------------------------------------- derived queries
  const refreshView = useCallback(async (nodeId: number) => {
    const { w, h } = viewport.current;
    if (w === 0) return;
    const [nextRects, nextCrumbs] = await Promise.all([
      invoke<Rect[]>("treemap", { node: nodeId, width: w, height: h }),
      invoke<Crumb[]>("breadcrumb", { node: nodeId }),
    ]);
    setRects(nextRects);
    setCrumbs(nextCrumbs);
  }, []);

  useEffect(() => {
    if (phase !== "ready") return;
    refreshView(node).catch((err) => setError(String(err)));
    invoke<BigFile[]>("largest_files", { limit: 30 }).then(setLargest).catch(() => {});
    invoke<TypeSlice[]>("type_breakdown", { limit: 9 }).then(setTypes).catch(() => {});
  }, [phase, node, refreshView]);

  const onResize = useCallback(
    (w: number, h: number) => {
      viewport.current = { w, h };
      if (phase === "ready") refreshView(node).catch(() => {});
    },
    [phase, node, refreshView]
  );

  // ---------------------------------------------------------------- actions
  const chooseFolder = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") {
      setPickedPath(selected);
      beginScan(selected);
    }
  };

  const beginScan = (path: string) => {
    setError("");
    setProgress({ files: 0, bytes: 0 });
    setPhase("scanning");
    invoke("start_scan", { path }).catch((err) => {
      setError(String(err));
      setPhase("idle");
    });
  };

  // ------------------------------------------------------------------- ui
  return (
    <div className="shell">
      <header className="topbar">
        <div className="wordmark">
          HEFT<span className="wordmark-dot">▪</span>
        </div>

        <div className="path-slot">
          {summary ? (
            <span className="path-text" title={summary.root_path}>
              {summary.root_path}
            </span>
          ) : (
            <span className="path-empty">no folder scanned</span>
          )}
        </div>

        {phase === "scanning" && (
          <div className="scan-live">
            <span className="pulse" />
            {formatCount(progress.files)} files · {formatBytes(progress.bytes)}
          </div>
        )}

        {phase === "ready" && summary && (
          <div className="scan-stats">
            {formatCount(summary.files)} files · {formatBytes(summary.bytes)}
            {summary.skipped > 0 ? ` · ${formatCount(summary.skipped)} skipped` : ""}
          </div>
        )}

        <div className="actions">
          {summary && phase === "ready" && (
            <button className="btn ghost" onClick={() => beginScan(summary.root_path)}>
              Rescan
            </button>
          )}
          <button
            className="btn primary"
            onClick={chooseFolder}
            disabled={phase === "scanning"}
          >
            {phase === "scanning" ? "Scanning…" : "Choose folder"}
          </button>
        </div>
      </header>

      {error && <div className="error-strip">{error}</div>}

      <main className="body">
        <section className="map-pane">
          {phase === "ready" && crumbs.length > 0 && (
            <nav className="crumbs">
              {crumbs.map((crumb, index) => (
                <span key={crumb.id}>
                  {index > 0 && <span className="crumb-sep">/</span>}
                  <button
                    className={`crumb ${index === crumbs.length - 1 ? "current" : ""}`}
                    onClick={() => setNode(crumb.id)}
                  >
                    {crumb.name}
                  </button>
                </span>
              ))}
            </nav>
          )}

          {phase === "ready" ? (
            <Treemap rects={rects} onZoom={setNode} onResize={onResize} />
          ) : (
            <div className="placeholder" onClick={phase === "idle" ? chooseFolder : undefined}>
              {phase === "scanning" ? (
                <>
                  <div className="ph-title">weighing {pickedPath || "…"}</div>
                  <div className="ph-sub">
                    {formatCount(progress.files)} files · {formatBytes(progress.bytes)}
                  </div>
                </>
              ) : (
                <>
                  <div className="ph-title">Pick a folder to weigh</div>
                  <div className="ph-sub">
                    every byte, mapped — click a directory to dive in
                  </div>
                </>
              )}
            </div>
          )}
        </section>

        <Sidebar largest={largest} types={types} />
      </main>
    </div>
  );
}
