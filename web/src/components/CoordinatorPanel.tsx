import { useCallback, useEffect, useState, type ReactNode } from "react";
import { ExternalLink, X } from "lucide-react";

import {
  fetchCoordinatorSummary,
  fetchDeploys,
  removeDeploy,
  setDeployDeployed,
  type CoordinatorSummary,
  type CoordinatorSummaryRow,
  type Deploy,
} from "../lib/api";
import { deployGroups, deployLabel, isCoordinatorPath, ticketLabel } from "../lib/coordinator";

const REFRESH_MS = 5000;

type Tab = "sessions" | "deploys";

const TABS: { id: Tab; label: string }[] = [
  { id: "sessions", label: "Sessions" },
  { id: "deploys", label: "Deploys" },
];

type BucketId = keyof CoordinatorSummary;

const BUCKETS: { id: BucketId; label: string; tone: string }[] = [
  { id: "wip", label: "WIP", tone: "text-status-running" },
  { id: "needs_action", label: "Need action", tone: "text-status-waiting" },
  { id: "idle", label: "Idle", tone: "text-status-idle" },
];

const TIME = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });

interface Props {
  onOpenSession: (id: string) => void;
}

/** Beside the coordinator's chat: what every session needs, and the PRs waiting to ship. */
export function CoordinatorPanel({ onOpenSession }: Props) {
  const [summary, setSummary] = useState<CoordinatorSummary | null>(null);
  const [deploys, setDeploys] = useState<Deploy[]>([]);
  const [open, setOpen] = useState<BucketId | null>("needs_action");
  const [tab, setTab] = useState<Tab>("sessions");

  const refresh = useCallback(() => {
    void fetchCoordinatorSummary().then((s) => s && setSummary(s));
    void fetchDeploys().then((d) => d && setDeploys(d));
  }, []);

  useEffect(() => {
    refresh();
    const timer = window.setInterval(refresh, REFRESH_MS);
    return () => window.clearInterval(timer);
  }, [refresh]);

  const toggle = (deploy: Deploy) => void setDeployDeployed(deploy.id, !deploy.deployed_at).then(refresh);
  const remove = (deploy: Deploy) => void removeDeploy(deploy.id).then(refresh);
  const pending = deploys.filter((d) => !d.deployed_at).length;

  return (
    <aside
      data-testid="coordinator-panel"
      className="hidden lg:flex w-80 shrink-0 flex-col border-l border-surface-700/60 bg-surface-850"
    >
      <div
        role="tablist"
        aria-label="Coordinator panel"
        className="flex shrink-0 gap-1 border-b border-surface-700/60 px-2"
      >
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            role="tab"
            id={`coordinator-tab-${t.id}`}
            aria-selected={tab === t.id}
            aria-controls={`coordinator-panel-${t.id}`}
            tabIndex={tab === t.id ? 0 : -1}
            onClick={() => setTab(t.id)}
            onKeyDown={(e) => {
              if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
              e.preventDefault();
              const next = TABS[(TABS.findIndex((x) => x.id === tab) + 1) % TABS.length]!.id;
              setTab(next);
              document.getElementById(`coordinator-tab-${next}`)?.focus();
            }}
            className={`-mb-px flex h-9 items-center gap-1.5 border-b-2 px-2 text-[13px] cursor-pointer transition-colors ${
              tab === t.id
                ? "border-brand-500 text-text-primary"
                : "border-transparent text-text-dim hover:text-text-secondary"
            }`}
          >
            {t.label}
            {t.id === "deploys" && pending > 0 && (
              <span
                data-testid="deploys-pending-count"
                className="rounded-full bg-surface-700 px-1.5 font-mono text-[11px] leading-4 tabular-nums text-text-secondary"
              >
                {pending}
              </span>
            )}
          </button>
        ))}
      </div>

      <section
        role="tabpanel"
        id="coordinator-panel-sessions"
        aria-labelledby="coordinator-tab-sessions"
        hidden={tab !== "sessions"}
        className="flex-1 overflow-y-auto p-3"
      >
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

      <section
        role="tabpanel"
        id="coordinator-panel-deploys"
        aria-labelledby="coordinator-tab-deploys"
        hidden={tab !== "deploys"}
        className="flex-1 overflow-y-auto p-3"
      >
        {deploys.length === 0 ? (
          <p className="text-xs text-text-dim">
            No pull requests yet. The coordinator adds them as sessions open them.
          </p>
        ) : (
          deployGroups(deploys, new Date()).map((g) => (
            <div key={g.label} className="mb-4">
              <h3 className="mb-1.5 text-xs font-medium uppercase tracking-wide text-text-dim">
                {g.label} <span className="tabular-nums">({g.deploys.length})</span>
              </h3>
              <ul className="flex flex-col gap-1.5">
                {g.deploys.map((d) => (
                  <DeployCard key={d.id} deploy={d} onToggle={toggle} onRemove={remove} onOpenSession={onOpenSession} />
                ))}
              </ul>
            </div>
          ))
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

function DeployCard({
  deploy,
  onToggle,
  onRemove,
  onOpenSession,
}: {
  deploy: Deploy;
  onToggle: (d: Deploy) => void;
  onRemove: (d: Deploy) => void;
  onOpenSession: (id: string) => void;
}) {
  const label = deployLabel(deploy);
  const pr = deployLabel({ url: deploy.url });
  const ticket = deploy.ticket ? ticketLabel(deploy.ticket) : null;
  const done = !!deploy.deployed_at;
  return (
    <li data-testid="deploy-card" className="group rounded-md border border-surface-700/60 bg-surface-900 px-2.5 py-2">
      <div className="flex items-start gap-2">
        <input
          type="checkbox"
          checked={done}
          onChange={() => onToggle(deploy)}
          aria-label={`Mark ${label} ${done ? "not deployed" : "deployed"}`}
          className="mt-0.5 shrink-0 cursor-pointer accent-brand-500"
        />
        <a
          href={deploy.url}
          target="_blank"
          rel="noreferrer"
          className={`min-w-0 flex-1 text-[13px] leading-snug hover:underline ${
            done ? "text-text-dim line-through" : "text-text-primary"
          }`}
        >
          {label}
        </a>
        <button
          type="button"
          onClick={() => onRemove(deploy)}
          aria-label={`Remove ${label}`}
          className="shrink-0 rounded p-0.5 text-text-dim opacity-0 cursor-pointer group-hover:opacity-100 focus:opacity-100 hover:text-text-secondary"
        >
          <X className="size-3.5" />
        </button>
      </div>
      <div className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-0.5 pl-5 font-mono text-[11px] text-text-dim">
        {pr !== label && (
          <a
            href={deploy.url}
            target="_blank"
            rel="noreferrer"
            className="flex items-center gap-0.5 hover:text-text-secondary"
          >
            {pr}
            <ExternalLink className="size-2.5" aria-hidden="true" />
          </a>
        )}
        {ticket &&
          (ticket.href ? (
            <a href={ticket.href} target="_blank" rel="noreferrer" className="text-brand-400 hover:underline">
              {ticket.text}
            </a>
          ) : (
            <span className="text-brand-400">{ticket.text}</span>
          ))}
        {deploy.session_id && (
          <button
            type="button"
            onClick={() => onOpenSession(deploy.session_id!)}
            disabled={!deploy.session_title}
            className="truncate enabled:cursor-pointer enabled:hover:text-text-secondary"
          >
            {deploy.session_title ?? "session gone"}
          </button>
        )}
        <span>
          {done
            ? `deployed ${TIME.format(new Date(deploy.deployed_at!))}`
            : `added ${TIME.format(new Date(deploy.added_at))}`}
        </span>
      </div>
      {deploy.note && <p className="mt-1 pl-5 text-xs text-text-secondary">{deploy.note}</p>}
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
