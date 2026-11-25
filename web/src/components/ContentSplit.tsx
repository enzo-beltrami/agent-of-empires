import { useCallback, useEffect, useRef, useState } from "react";

import { safeGetItem, safeSetItem } from "../lib/safeStorage";

const SPLIT_STORAGE_KEY = "aoe-split-ratio";
const DEFAULT_PANEL_WIDTH = 380;
const MIN_MAIN_WIDTH = 400;
const MIN_PANEL_WIDTH = 280;

interface Props {
  left: React.ReactNode;
  right: React.ReactNode;
  collapsed: boolean;
  onToggleCollapse: () => void;
}

function loadSavedWidth(): number {
  const saved = safeGetItem(SPLIT_STORAGE_KEY);
  if (saved) {
    const w = parseInt(saved, 10);
    if (w >= MIN_PANEL_WIDTH) return w;
  }
  return DEFAULT_PANEL_WIDTH;
}

export function ContentSplit({ left, right, collapsed, onToggleCollapse }: Props) {
  const [panelWidth, setPanelWidth] = useState(loadSavedWidth);
  // Publish the live width so the TopBar's right zone can size itself to match the panel column and extend the
  // divider up through the header, mirroring the sidebar.
  useEffect(() => {
    document.documentElement.style.setProperty("--aoe-right-panel-width", `${panelWidth}px`);
  }, [panelWidth]);
  const containerRef = useRef<HTMLDivElement>(null);
  const dragging = useRef(false);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    dragging.current = true;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
  }, []);

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!dragging.current || !containerRef.current) return;
      const rect = containerRef.current.getBoundingClientRect();
      const newPanelWidth = rect.right - e.clientX;
      const mainWidth = rect.width - newPanelWidth;

      if (newPanelWidth >= MIN_PANEL_WIDTH && mainWidth >= MIN_MAIN_WIDTH) {
        setPanelWidth(newPanelWidth);
      }
    };

    const handleMouseUp = () => {
      if (!dragging.current) return;
      dragging.current = false;
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      // Persist
      setPanelWidth((w) => {
        safeSetItem(SPLIT_STORAGE_KEY, String(w));
        return w;
      });
      // Trigger resize for terminal fit
      window.dispatchEvent(new Event("resize"));
    };

    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);
    return () => {
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUp);
    };
  }, []);

  // Re-fit terminal when collapsed state changes
  useEffect(() => {
    window.dispatchEvent(new Event("resize"));
  }, [collapsed]);

  return (
    <div ref={containerRef} className="flex-1 flex min-h-0 overflow-hidden relative">
      {/* Session */}
      <div className="flex-1 flex flex-col min-w-0 min-h-0">{left}</div>

      {!collapsed && (
        <>
          {/* Drag handle (desktop) */}
          <div
            data-testid="content-split-resize-handle"
            onMouseDown={handleMouseDown}
            onDoubleClick={onToggleCollapse}
            className="hidden md:block w-1 cursor-col-resize shrink-0 hover:bg-brand-600/50 transition-colors duration-75"
          />

          {/* Coordinator panel */}
          <div style={{ width: panelWidth }} className="flex shrink-0 flex-col min-h-0 overflow-hidden">
            {right}
          </div>
        </>
      )}
    </div>
  );
}
