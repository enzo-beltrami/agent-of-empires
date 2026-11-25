// Mocked sidebar API surface for sidebar specs.

import type { Page, Route } from "@playwright/test";
import { sessionResponse } from "./sessions";

export interface MockSessionInput {
  id: string;
  title: string;
  project_path: string;
  branch: string | null;
  created_at?: string;
  group?: string;
  /** Null means "No organization" on the org axis (#3283). */
  remote_owner?: string | null;
  /** Defaults to `${remote_owner}@example.com`. */
  remote_owner_key?: string | null;
  /** Extra SessionResponse fields merged over the defaults. */
  fields?: Record<string, unknown>;
}

type MockSession = MockSessionInput & { created_at: string };

function fillCreatedAt(s: MockSessionInput, fallbackIndex: number): MockSession {
  return {
    ...s,
    created_at: s.created_at ?? new Date(Date.UTC(2025, 0, 1 + fallbackIndex)).toISOString(),
  };
}

function toResponse(s: MockSession) {
  return sessionResponse({
    id: s.id,
    title: s.title,
    project_path: s.project_path,
    group_path: s.group ?? "",
    created_at: s.created_at,
    branch: s.branch,
    remote_owner: s.remote_owner ?? null,
    remote_owner_key:
      s.remote_owner_key !== undefined ? s.remote_owner_key : s.remote_owner ? `${s.remote_owner}@example.com` : null,
    ...s.fields,
  });
}

/** The server's workspace id (see useWorkspaces.ts). */
export function workspaceId(s: { project_path: string; branch: string | null; id: string }): string {
  return s.branch ? `${s.project_path}::${s.branch}` : `${s.project_path}::__session__::${s.id}`;
}

export interface SidebarMockHandle {
  puts: Array<{ order?: string[] }>;
  /** One-shot override for the next PUT's response. */
  nextPutResponse: { status?: number; body?: string } | null;
  readOnly: boolean;
  /** `PATCH /api/sessions/:id/group` bodies, in order, when `persistGroupMoves` is set. */
  groupMoves: Array<{ id: string; group: string }>;
}

export interface SidebarMockOptions {
  sessions: MockSessionInput[];
  /** Defaults to the input order. */
  ordering?: string[];
  readOnly?: boolean;
  /** A successful PUT replaces the served ordering, like the real server. */
  persistPutOrdering?: boolean;
  /** A group PATCH moves the session and bumps the runtime revision, like the real server. */
  persistGroupMoves?: boolean;
}

export async function installSidebarMocks(page: Page, opts: SidebarMockOptions): Promise<SidebarMockHandle> {
  const filled = opts.sessions.map((s, i) => fillCreatedAt(s, i));
  const handle: SidebarMockHandle = {
    puts: [],
    nextPutResponse: null,
    readOnly: !!opts.readOnly,
    groupMoves: [],
  };
  let revision = 1;

  let ordering = opts.ordering ?? filled.map((s) => workspaceId(s));

  await page.route("**/api/login/status*", (r) => r.fulfill({ json: { required: false, authenticated: true } }));
  await page.route(/\/api\/sessions(\?.*)?$/, (r) => {
    if (r.request().method() !== "GET") return r.fulfill({ status: 400 });
    return r.fulfill({
      headers: { "aoe-runtime-epoch": "mock-runtime", "aoe-runtime-revision": String(revision) },
      json: {
        sessions: filled.map(toResponse),
        workspace_ordering: ordering,
      },
    });
  });
  if (opts.persistGroupMoves) {
    await page.route(/\/api\/sessions\/[^/]+\/group$/, (r) => {
      const id = decodeURIComponent(new URL(r.request().url()).pathname.split("/").at(-2)!);
      const session = filled.find((s) => s.id === id);
      if (r.request().method() !== "PATCH" || !session) return r.fulfill({ status: 400 });
      const { group } = r.request().postDataJSON() as { group: string };
      handle.groupMoves.push({ id, group });
      session.group = group;
      revision += 1;
      return r.fulfill({
        headers: { "aoe-runtime-epoch": "mock-runtime", "aoe-runtime-revision": String(revision) },
        json: toResponse(session),
      });
    });
  }
  await page.route("**/api/workspace-ordering", (r: Route) => {
    if (r.request().method() === "PUT") {
      const body = r.request().postDataJSON() as { order?: string[] };
      handle.puts.push(body ?? {});
      const override = handle.nextPutResponse;
      handle.nextPutResponse = null;
      if (override) return r.fulfill(override);
      if (opts.persistPutOrdering && body?.order) ordering = body.order;
    }
    return r.fulfill({ json: { order: [] } });
  });
  await page.route("**/api/about", (r) =>
    r.fulfill({
      json: {
        read_only: handle.readOnly,
        auth_mode: "none",
        behind_tunnel: false,
        profile: "default",
      },
    }),
  );
  for (const path of ["settings", "themes", "agents", "profiles", "groups", "devices"]) {
    await page.route(`**/api/${path}`, (r) => r.fulfill({ json: [] }));
  }
  await page.route("**/api/docker/status", (r) => r.fulfill({ json: {} }));

  return handle;
}

export function threeSessionsInOneRepo(): MockSessionInput[] {
  return [
    {
      id: "s-a",
      title: "alpha",
      project_path: "/tmp/repo",
      branch: "feature/a",
      created_at: "2025-03-01T00:00:00Z",
    },
    {
      id: "s-b",
      title: "beta",
      project_path: "/tmp/repo",
      branch: "feature/b",
      created_at: "2025-02-01T00:00:00Z",
    },
    {
      id: "s-c",
      title: "gamma",
      project_path: "/tmp/repo",
      branch: "feature/c",
      created_at: "2025-01-01T00:00:00Z",
    },
  ];
}
