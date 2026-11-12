import { useCallback, useEffect, useRef, useState } from "react";

import { fetchPlugins, setPluginEnabled, type PluginListResponse, type PluginView } from "../../lib/api";
import { CancelButton, ConfirmButton, DANGER_BUTTON, Dialog } from "../Dialog";
import { useConfirmKeys, useDialogFocus } from "../dialogHooks";
import { PluginCard } from "./PluginCard";

const WEB_PLUGIN_ID = "aoe.web";

/** The built-in plugins with their on/off switches. Toggling needs an elevated
 *  session; the fetch interceptor handles `403 elevation_required`. */
export function PluginsSettings({ readOnly = false }: { readOnly?: boolean } = {}) {
  const [data, setData] = useState<PluginListResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirmWebOff, setConfirmWebOff] = useState<PluginView | null>(null);

  useEffect(() => {
    // Deferred so the effect body does not set state synchronously.
    const timer = setTimeout(() => {
      void fetchPlugins().then((next) => {
        if (next) setData(next);
        else setError("Failed to load plugins.");
      });
    }, 0);
    return () => clearTimeout(timer);
  }, []);

  const onToggle = async (plugin: PluginView, enabled: boolean) => {
    setBusy(true);
    setError(null);
    try {
      const result = await setPluginEnabled(plugin.id, enabled);
      if (result.kind === "ok") {
        setData(result.data);
      } else {
        setError(result.message);
      }
    } finally {
      setBusy(false);
    }
  };

  if (!data && !error) {
    return <p className="text-sm text-text-dim">Loading plugins…</p>;
  }

  return (
    <div className="space-y-3">
      {error && <p className="text-sm text-status-error">{error}</p>}
      {data && data.plugins.length === 0 && (
        <p className="text-xs text-text-dim" data-testid="plugins-empty">
          No plugins detected.
        </p>
      )}
      {data?.plugins.map((plugin) => (
        <PluginCard
          key={plugin.id}
          plugin={plugin}
          readOnly={readOnly}
          toggleBusy={busy}
          onToggle={(enabled) =>
            plugin.id === WEB_PLUGIN_ID && !enabled ? setConfirmWebOff(plugin) : void onToggle(plugin, enabled)
          }
        />
      ))}

      {confirmWebOff && (
        <WebDashboardOffConfirm
          onCancel={() => setConfirmWebOff(null)}
          onConfirm={() => {
            setConfirmWebOff(null);
            void onToggle(confirmWebOff, false);
          }}
        />
      )}
    </div>
  );
}

/** Disabling the Web dashboard plugin turns off the page the user is on once the Daemon restarts. */
function WebDashboardOffConfirm({ onConfirm, onCancel }: { onConfirm: () => void; onCancel: () => void }) {
  const confirmButtonRef = useRef<HTMLButtonElement | null>(null);
  // A ref, not state, so an Enter+click double fire is blocked before a re-render.
  const firedRef = useRef(false);
  const confirm = useCallback(() => {
    if (firedRef.current) return;
    firedRef.current = true;
    onConfirm();
  }, [onConfirm]);
  useDialogFocus(confirmButtonRef);
  useConfirmKeys(onCancel, confirm);

  return (
    <Dialog
      id="web-dashboard-off-dialog"
      title="Turn off the Web dashboard?"
      onDismiss={onCancel}
      footer={
        <>
          <CancelButton onClick={onCancel} />
          <ConfirmButton
            buttonRef={confirmButtonRef}
            onClick={confirm}
            testId="web-dashboard-off-confirm"
            className={DANGER_BUTTON}
          >
            Turn off
          </ConfirmButton>
        </>
      }
    >
      <p className="text-[13px] text-text-secondary">
        It keeps running until the Daemon restarts, then <code>aoe serve</code> won&apos;t start until you turn it back
        on.
      </p>
    </Dialog>
  );
}
