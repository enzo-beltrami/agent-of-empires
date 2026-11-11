import { useCallback, useEffect, useState, type FormEvent, type ReactNode } from "react";
import { ExternalLink, X } from "lucide-react";

import {
  addDeploy,
  fetchCoordinatorSummary,
  fetchDeploys,
  removeDeploy,
  setDeployDeployed,
  type CoordinatorSummary,
  type CoordinatorSummaryRow,
  type Deploy,
} from "../lib/api";
import { deployLabel, isCoordinatorPath } from "../lib/coordinator";

const REFRESH_MS = 5000;

type BucketId = keyof CoordinatorSummary;

const BUCKETS: { id: BucketId; label: string; tone: string }[] = [
  { id: "wip", label: "WIP", tone: "text-status-running" },
  { id: "needs_action", label: "Need action", tone: "text-status-waiting" },
  { id: "idle", label: "Idle", tone: "text-status-idle" },
];

interface Props {
  onOpenSession: (id: string) => void;
}

/** Beside the coordinator's chat: what every session needs, and the PRs waiting to ship. */
export function CoordinatorPanel({ onOpenSession }: Props) {
  const [summary, setSummary] = useState<CoordinatorSummary | null>(null);
  const [deploys, setDeploys] = useState<Deploy[]>([]);
  const [open, setOpen] = useState<BucketId | null>("needs_action");
  const [url, setUrl] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [showDeployed, setShowDeployed] = useState(false);

  const refresh = useCallback(() => {
    void fetchCoordinatorSummary().then((s) => s && setSummary(s));
    void fetchDeploys().then((d) => d && setDeploys(d));
  }, []);

  useEffect(() => {
    refresh();
    const timer = window.setInterval(refresh, REFRESH_MS);
    return () => window.clearInterval(timer);
  }, [refresh]);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (!url.trim()) return;
    void addDeploy(url).then((result) => {
      if (result.ok) {
        setUrl("");
        setError(null);
        refresh();
      } else {
        setError(result.message);
      }
    });
  };
  const toggle = (deploy: Deploy) => void setDeployDeployed(deploy.id, !deploy.deployed_at).then(refresh);
  const remove = (deploy: Deploy) => void removeDeploy(deploy.id).then(refresh);

  const pending = deploys.filter((d) => !d.deployed_at);
  const deployed = deploys.filter((d) => d.deployed_at).reverse();

  return (
    <aside
      data-testid="coordinator-panel"
      className="hidden lg:flex w-80 shrink-0 flex-col gap-5 overflow-y-auto border-l border-surface-700/60 bg-surface-850 p-3"
    >
      <section aria-label="Sessions">
        <h2 className="mb-2 text-xs font-medium uppercase tracking-wide text-text-dim">Sessions</h2>
        <div className="grid grid-cols-3 gap-2">
          {BUCKETS.map((b) => (
            <button
              key={b.id}
              type="button"
              aria-pressed={open === b.id}
              onClick={() => setOpen(open === b.id ? null : b.id)}
              className={`flex flex-col items-start rounded-md border px-2.5 py-2 text-left cursor-pointer transition-colors ${
                open === b.id
                  ? "border-surface-600 bg-surface-800"
                  : "border-surface-700/60 bg-surface-900 hover:bg-surface-800"
              }`}
            >
              <span className={`font-mono text-xl leading-none tabular-nums ${b.tone}`} data-testid={`count-${b.id}`}>
                {summary ? summary[b.id].length : "–"}
              </span>
              <span className="mt-1 text-xs text-text-secondary">{b.label}</span>
            </button>
          ))}
        </div>
        {open && summary && <SessionRows rows={summary[open]} onOpen={onOpenSession} />}
      </section>

      <section aria-label="Deploy list">
        <h2 className="mb-2 text-xs font-medium uppercase tracking-wide text-text-dim">
          Deploy list {pending.length > 0 && <span className="tabular-nums">({pending.length})</span>}
        </h2>
        <form onSubmit={submit} className="flex gap-1.5">
          <input
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="Pull request URL"
            aria-label="Pull request URL"
            className="h-8 min-w-0 flex-1 rounded-md border border-surface-700 bg-surface-900 px-2 text-[13px] text-text-primary placeholder:text-text-dim focus:outline-none focus:ring-2 focus:ring-brand-500"
          />
          <button
            type="submit"
            className="h-8 shrink-0 rounded-md border border-surface-700 bg-surface-800 px-2.5 text-[13px] text-text-secondary cursor-pointer hover:bg-surface-700/60"
          >
            Add
          </button>
        </form>
        {error && <p className="mt-1.5 text-xs text-status-error">{error}</p>}
        {pending.length === 0 ? (
          <p className="mt-2 text-xs text-text-dim">Nothing waiting to deploy.</p>
        ) : (
          <ul className="mt-2">
            {pending.map((d) => (
              <DeployRow key={d.id} deploy={d} onToggle={toggle} onRemove={remove} />
            ))}
          </ul>
        )}
        {deployed.length > 0 && (
          <>
            <button
              type="button"
              onClick={() => setShowDeployed((v) => !v)}
              className="mt-2 text-xs text-text-dim cursor-pointer hover:text-text-secondary"
            >
              {showDeployed ? "Hide" : "Show"} deployed ({deployed.length})
            </button>
            {showDeployed && (
              <ul className="mt-1">
                {deployed.map((d) => (
                  <DeployRow key={d.id} deploy={d} onToggle={toggle} onRemove={remove} />
                ))}
              </ul>
            )}
          </>
        )}
      </section>
    </aside>
  );
}

function SessionRows({ rows, onOpen }: { rows: CoordinatorSummaryRow[]; onOpen: (id: string) => void }) {
  if (rows.length === 0) return <p className="mt-2 text-xs text-text-dim">None.</p>;
  return (
    <ul className="mt-2">
      {rows.map((row) => (
        <li key={row.id}>
          <button
            type="button"
            onClick={() => onOpen(row.id)}
            title={row.project_path}
            className="flex h-7 w-full items-center gap-2 rounded-md px-1.5 text-left cursor-pointer hover:bg-surface-800"
          >
            <span className="min-w-0 flex-1 truncate font-mono text-[13px] text-text-primary">{row.title}</span>
            <span className="shrink-0 truncate font-mono text-[11px] text-text-dim">
              {row.project_path.split("/").filter(Boolean).pop()}
            </span>
          </button>
        </li>
      ))}
    </ul>
  );
}

function DeployRow({
  deploy,
  onToggle,
  onRemove,
}: {
  deploy: Deploy;
  onToggle: (d: Deploy) => void;
  onRemove: (d: Deploy) => void;
}) {
  const label = deployLabel(deploy);
  return (
    <li className="group flex h-7 items-center gap-2 rounded-md px-1.5 hover:bg-surface-800">
      <input
        type="checkbox"
        checked={!!deploy.deployed_at}
        onChange={() => onToggle(deploy)}
        aria-label={`Mark ${label} ${deploy.deployed_at ? "not deployed" : "deployed"}`}
        className="shrink-0 cursor-pointer accent-brand-500"
      />
      <a
        href={deploy.url}
        target="_blank"
        rel="noreferrer"
        title={deploy.note ?? deploy.url}
        className={`flex min-w-0 flex-1 items-center gap-1 font-mono text-[13px] hover:underline ${
          deploy.deployed_at ? "text-text-dim line-through" : "text-text-primary"
        }`}
      >
        <span className="truncate">{label}</span>
        <ExternalLink className="size-3 shrink-0 opacity-60" aria-hidden="true" />
      </a>
      <button
        type="button"
        onClick={() => onRemove(deploy)}
        aria-label={`Remove ${label}`}
        className="shrink-0 rounded p-0.5 text-text-dim opacity-0 cursor-pointer group-hover:opacity-100 focus:opacity-100 hover:text-text-secondary"
      >
        <X className="size-3.5" />
      </button>
    </li>
  );
}

/** Adds the panel beside a session's view when that session is the coordinator. */
export function WithCoordinatorPanel({
  projectPath,
  onOpenSession,
  children,
}: {
  projectPath: string;
  onOpenSession: (id: string) => void;
  children: ReactNode;
}) {
  if (!isCoordinatorPath(projectPath)) return children;
  return (
    <div className="flex h-full min-h-0">
      <div className="flex min-w-0 flex-1 flex-col">{children}</div>
      <CoordinatorPanel onOpenSession={onOpenSession} />
    </div>
  );
}
