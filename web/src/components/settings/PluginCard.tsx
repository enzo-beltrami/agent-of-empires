import type { PluginView } from "../../lib/api";

interface Props {
  plugin: PluginView;
  readOnly: boolean;
  toggleBusy: boolean;
  onToggle: (enabled: boolean) => void;
}

/** One built-in plugin: identity, description, and its on/off switch. */
export function PluginCard({ plugin, readOnly, toggleBusy, onToggle }: Props) {
  return (
    <div className="rounded border border-surface-700 bg-surface-850 p-3" data-testid={`plugin-${plugin.id}`}>
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <span className="font-medium">{plugin.name}</span>
            <span className="text-xs text-text-dim">v{plugin.version}</span>
          </div>
          <p className="mt-1 text-xs text-text-dim">{plugin.description}</p>
        </div>
        {!readOnly && (
          <label className="flex shrink-0 items-center gap-1 text-xs">
            <input
              type="checkbox"
              role="switch"
              aria-label={`Enable ${plugin.name}`}
              checked={plugin.enabled}
              disabled={toggleBusy}
              onChange={(e) => onToggle(e.target.checked)}
            />
            Enabled
          </label>
        )}
      </div>
    </div>
  );
}
