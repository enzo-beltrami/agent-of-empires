import { useCallback, useEffect, useState, type KeyboardEvent, type ReactNode } from "react";
import { ExternalLink, X } from "lucide-react";

import {
  addCoordinatorNote,
  fetchCoordinatorNotes,
  fetchCoordinatorSummary,
  fetchDeploys,
  removeCoordinatorNote,
  removeDeploy,
  updateCoordinatorNote,
  updateDeploy,
  type CoordinatorNote,
  type CoordinatorSummary,
  type CoordinatorSummaryRow,
  type Deploy,
} from "../lib/api";
import { deployGroups, deployLabel, isCoordinatorPath, ticketLabel } from "../lib/coordinator";

const REFRESH_MS = 5000;

type Tab = "sessions" | "deploys" | "notes";

const TABS: { id: Tab; label: string }[] = [
  { id: "sessions", label: "Sessions" },
  { id: "deploys", label: "Deploys" },
  { id: "notes", label: "Notes" },
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

  const toggle = (deploy: Deploy) => void updateDeploy(deploy.id, { deployed: !deploy.deployed_at }).then(refresh);
  const toggleTested = (deploy: Deploy) => void updateDeploy(deploy.id, { tested: !deploy.tested }).then(refresh);
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
              const step = e.key === "ArrowLeft" ? -1 : 1;
              const next = TABS[(TABS.findIndex((x) => x.id === tab) + step + TABS.length) % TABS.length]!.id;
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
                  <DeployCard
                    key={d.id}
                    deploy={d}
                    onToggle={toggle}
                    onToggleTested={toggleTested}
                    onRemove={remove}
                    onOpenSession={onOpenSession}
                  />
                ))}
              </ul>
            </div>
          ))
        )}
      </section>

      <section
        role="tabpanel"
        id="coordinator-panel-notes"
        aria-labelledby="coordinator-tab-notes"
        hidden={tab !== "notes"}
        className="flex-1 overflow-y-auto p-3"
      >
        <NotesList />
      </section>
    </aside>
  );
}

const FIELD =
  "w-full resize-y rounded-md border border-surface-700/60 bg-surface-900 p-2 text-[13px] leading-relaxed text-text-primary placeholder:text-text-dim focus:border-surface-600 focus:outline-none";
const SMALL_BUTTON =
  "h-7 rounded-md px-2 text-xs cursor-pointer transition-colors disabled:cursor-default disabled:opacity-50";

/** Mod+Enter runs `submit`; Escape runs `cancel`, kept from the dashboard's global Escape. */
function noteKeys(submit: () => void, cancel?: () => void) {
  return (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      submit();
    } else if (e.key === "Escape" && cancel) {
      e.preventDefault();
      e.stopPropagation();
      cancel();
    }
  };
}

/** The coordinator's notes as separate cards, newest first, each edited or removed on its own. */
function NotesList() {
  const [notes, setNotes] = useState<CoordinatorNote[] | null>(null);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(() => {
    void fetchCoordinatorNotes().then((n) => n && setNotes(n));
  }, []);

  useEffect(() => {
    refresh();
    const timer = window.setInterval(refresh, REFRESH_MS);
    return () => window.clearInterval(timer);
  }, [refresh]);

  const add = () => {
    if (!draft.trim()) return;
    void addCoordinatorNote(draft).then((result) => {
      if (result.ok) {
        setDraft("");
        setError(null);
        refresh();
      } else {
        setError(result.message ?? "Could not add the note.");
      }
    });
  };

  return (
    <>
      <textarea
        aria-label="New note"
        value={draft}
        rows={2}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={noteKeys(add)}
        placeholder="Add a note. The coordinator reads these and adds what you ask it to remember."
        className={FIELD}
      />
      <div className="mt-1.5 flex items-center justify-between gap-2">
        <span className="text-[11px] text-text-dim">Ctrl+Enter to add</span>
        <button
          type="button"
          onClick={add}
          disabled={!draft.trim()}
          className={`${SMALL_BUTTON} border border-surface-700 bg-surface-800 text-text-secondary enabled:hover:bg-surface-700/60`}
        >
          Add note
        </button>
      </div>
      {error && <p className="mt-1.5 text-xs text-status-error-text">{error}</p>}
      {notes && notes.length === 0 && <p className="mt-3 text-xs text-text-dim">No notes yet.</p>}
      <ul className="mt-3 flex flex-col gap-1.5">
        {[...(notes ?? [])].reverse().map((note) => (
          <NoteCard key={note.id} note={note} onChanged={refresh} />
        ))}
      </ul>
    </>
  );
}

function NoteCard({ note, onChanged }: { note: CoordinatorNote; onChanged: () => void }) {
  const [editing, setEditing] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const save = () => {
    if (editing === null || !editing.trim()) return;
    if (editing.trim() === note.text) {
      setEditing(null);
      return;
    }
    void updateCoordinatorNote(note.id, editing).then((result) => {
      if (result.ok) {
        setEditing(null);
        setError(null);
        onChanged();
      } else {
        setError(result.message ?? "Could not save the note.");
      }
    });
  };
  const remove = () => void removeCoordinatorNote(note.id).then(onChanged);
  const edited = note.updated_at !== note.created_at;

  return (
    <li data-testid="note-card" className="group rounded-md border border-surface-700/60 bg-surface-900 px-2.5 py-2">
      {editing === null ? (
        <p className="whitespace-pre-wrap break-words text-[13px] leading-relaxed text-text-primary">{note.text}</p>
      ) : (
        <textarea
          aria-label="Edit note"
          autoFocus
          value={editing}
          rows={Math.min(10, Math.max(2, editing.split("\n").length))}
          onChange={(e) => setEditing(e.target.value)}
          onKeyDown={noteKeys(save, () => setEditing(null))}
          className={FIELD}
        />
      )}
      {error && <p className="mt-1 text-xs text-status-error-text">{error}</p>}
      <div className="mt-1 flex items-center gap-1">
        <span className="flex-1 font-mono text-[11px] text-text-dim" title={note.created_at}>
          {edited ? "edited " : ""}
          {TIME.format(new Date(note.updated_at))}
        </span>
        {editing === null ? (
          <div className="flex gap-1 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
            <button
              type="button"
              onClick={() => {
                setEditing(note.text);
                setConfirming(false);
              }}
              className={`${SMALL_BUTTON} text-text-dim hover:bg-surface-800 hover:text-text-secondary`}
            >
              Edit
            </button>
            <button
              type="button"
              onClick={() => (confirming ? remove() : setConfirming(true))}
              onBlur={() => setConfirming(false)}
              className={`${SMALL_BUTTON} ${
                confirming
                  ? "bg-status-error/15 text-status-error-text"
                  : "text-text-dim hover:bg-surface-800 hover:text-text-secondary"
              }`}
            >
              {confirming ? "Confirm delete" : "Delete"}
            </button>
          </div>
        ) : (
          <>
            <button
              type="button"
              onClick={() => setEditing(null)}
              className={`${SMALL_BUTTON} text-text-dim hover:bg-surface-800 hover:text-text-secondary`}
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={save}
              disabled={!editing.trim()}
              className={`${SMALL_BUTTON} border border-surface-700 bg-surface-800 text-text-secondary enabled:hover:bg-surface-700/60`}
            >
              Save
            </button>
          </>
        )}
      </div>
    </li>
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
  onToggleTested,
  onRemove,
  onOpenSession,
}: {
  deploy: Deploy;
  onToggle: (d: Deploy) => void;
  onToggleTested: (d: Deploy) => void;
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
        <button
          type="button"
          onClick={() => onToggleTested(deploy)}
          aria-label={`${deploy.tested ? "Tested" : "Untested"}: mark ${label} ${deploy.tested ? "untested" : "tested"}`}
          className={`rounded-full border px-1.5 leading-4 cursor-pointer transition-colors ${
            deploy.tested
              ? "border-status-running/30 bg-status-running/10 text-status-running hover:bg-status-running/20"
              : "border-status-waiting/30 bg-status-waiting/10 text-status-waiting hover:bg-status-waiting/15"
          }`}
        >
          {deploy.tested ? "tested" : "untested"}
        </button>
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
