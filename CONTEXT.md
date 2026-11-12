# Agent of Empires

A session manager for AI coding agents. It runs many agents in parallel, each in its own persistent Session, optionally on its own branch or inside a sandbox, and lets the user watch and steer them from the web dashboard or the CLI.

## Language

### Sessions

**Session**:
One managed agent run together with its saved state: title, project, status, view, flags. The unit the user creates, attaches to, trashes and restores.
_Avoid_: Instance, agent session

**tmux session**:
The tmux server session that hosts a pane for a Session. One Session owns several: the agent pane, any Paired terminal, and any Tool session. Always write the "tmux" qualifier.
_Avoid_: session (unqualified), sub-session

**Agent conversation**:
The agent's own conversation on its side, identified by the agent's session id. Survives independently of the Session that wraps it.
_Avoid_: agent session, Claude session

**Title**:
A Session's display name. Chosen randomly from AoE2 civilization names unless the user or Smart rename sets it.
_Avoid_: name

**Smart rename**:
Replacing a Session's Title with one derived from the agent's work. Can be turned off per Project.
_Avoid_: auto-naming, agent-driven naming

**Status**:
The Session's observed activity state. Values: Running, Waiting, Idle, Error, Starting, Stopped, Unknown, Creating, Deleting.
_Avoid_: working (for Running), state

**Status detection**:
Working out a Session's Status, at one of three support levels: basic, pane parsing, or Agent hook.
_Avoid_: pane reconciliation

**Fork**:
A new Session seeded from a parent Session's Agent conversation.

**Scratch session**:
A Session with no Project. It runs in a temporary directory AoE owns.

**Group**:
A named container in the session sidebar, addressed by a slash-separated path. Can be collapsed or archived. Archived, Trash and Scratch are synthetic Groups.
_Avoid_: folder, section

**Archive**:
Soft-hiding a Session or Group. It stays restorable and shows under the Archived Group.

**Trash**:
Soft-deleting a Session. It stays restorable until purged.
_Avoid_: delete, remove, trashed shelf

**Purge**:
Permanently destroying a trashed Session and its resources.
_Avoid_: delete, hard delete

**Snooze**:
Hiding a Session until a chosen time. Expired snoozes lift on their own.

**Dismissed**:
A Session that is trashed, snoozed or archived. The "do not bother me" states as one set.

**Pin**:
Forcing a Session to the top of the dashboard. Pinning cancels Archive and Snooze.

**Favorite**:
A user mark on a Session. Favoriting cancels Archive and Snooze, like Pin. The web sidebar uses favorite only as a tiebreak within its attention tier.
_Avoid_: star, pin (different concept)

**Unread**:
A Session with output the user has not looked at since it last changed.

### Where sessions run

**Project**:
A registered directory in the project registry, usually a Repository, with its own defaults such as base branch and Smart rename. Scoped Global or to one Profile.
_Avoid_: saved project, repo (for the registry entry), folder

**Working directory**:
The directory a Session actually runs in. Usually a Project, a Worktree or a Workspace, but need not be registered.
_Avoid_: project path, project (for the unregistered case)

**Repository**:
A git repository on disk. The unit that Worktrees, diffs and Base branch act on.
_Avoid_: project (for the git object), directory

**Worktree**:
A git worktree and branch AoE creates or attaches for one Session. AoE-managed Worktrees are cleaned up with the Session.
_Avoid_: branch (when the worktree is meant)

**Workspace**:
A multi-repo Session's directory, holding one Worktree per Repository on a shared branch name.
_Avoid_: profile, multi-repo session, `/workspace` (the Sandbox mount)

**Base branch**:
The ref a Session's diff is compared against. Per-session override, then creation base, then Profile default, then the Repository's default branch. The Project default applies only when the Worktree branch is created and is then recorded as the creation base.
_Avoid_: PR target, diff base

**Sandbox**:
Running a Session's agent inside a per-session container with the Working directory mounted.
_Avoid_: container (for the feature), isolation

**Container runtime**:
The container engine a Sandbox uses: Docker, Podman or Apple Container.
_Avoid_: Docker (as the generic word)

**Profile**:
A named, isolated set of Sessions, Projects and setting overrides. One Profile is the default.
_Avoid_: workspace, separate workspace

**Substrate**:
The kind of place a Session runs: local, worktree, workspace, sandbox or scratch.
_Avoid_: substrate for tmux vs acp (that axis is Terminal view vs Structured view)

### Agents

**Agent**:
A supported AI coding CLI such as Claude Code, Codex or OpenCode, with its registry metadata: binary, detection, launch flags, hooks.
_Avoid_: tool (that word means an ACP tool call), agent CLI

**Custom agent**:
An Agent not built in, defined only through settings. Detect-as maps it onto a built-in Agent whose detection and ACP adapter it inherits.

**ACP**:
Agent Client Protocol. The JSON-RPC protocol AoE speaks to an Agent in Structured view. AoE is the client, the ACP adapter is the server.

**ACP adapter**:
The ACP-speaking program for an Agent, either native or a bundled shim.
_Avoid_: ACP server, agent (for the adapter process)

**Agent capability**:
A feature an Agent advertises over ACP: models, modes, thinking levels.

**Yolo mode**:
Launching an Agent with its permission prompts auto-approved.
_Avoid_: auto-approve

**Agent hook**:
A command AoE installs into an Agent's own hook settings so the Agent reports its Status to AoE.
_Avoid_: status hook (different concept), sidecar hook

**Status hook**:
A user-configured shell command AoE runs when a Session changes Status.
_Avoid_: agent hook (different concept), hook (unqualified)

**Repo hook**:
A lifecycle command declared in a Repository's own AoE config, run on Session create or launch, gated by hook trust.
_Avoid_: hook (unqualified), git hook

### Turns and views

**Structured view**:
A Session rendered from ACP state: plan, tool-call cards, diffs, approvals. The Agent runs under a Worker instead of a tmux pane.
_Avoid_: web dashboard (that is a Client), ACP-native, structured session

**Terminal view**:
A Session rendered as its raw tmux pane.
_Avoid_: terminal (unqualified)

**Paired terminal**:
A shell tmux session attached to a Session, on the host or inside its Sandbox.
_Avoid_: terminal (unqualified), cterm, host terminal

**Tool session**:
A persistent tmux session running a user-configured dev tool such as lazygit in a Session's Working directory.

**Turn**:
One prompt and response cycle of an Agent in Structured view. Can be in flight, steerable, or agent-initiated.

**Prompt**:
User text sent to a Session.
_Avoid_: message (the composer label), one-shot prompt (a CLI flag)

**Queue**:
Prompts held for a Session until its current Turn ends.
_Avoid_: parked, follow-up

**Steer**:
Delivering a Prompt into the running Turn instead of queuing it.

**Approval**:
A pending permission request from the Agent for one tool call, settled once.
_Avoid_: permission request, approval card

**Elicitation**:
A form-style request from the Agent asking the user for structured input. Not an Approval.

**Wakeup**:
A scheduled re-invocation of an Agent, started by the Agent itself rather than by a user Prompt.
_Avoid_: monitor

**Attach**:
Entering a Session's tmux pane. Unrelated to attaching a Worktree to an existing branch.

### Processes

**Daemon**:
The `aoe serve` process. Hosts the API, optional Web serving, the Supervisor, the Reconciler and push notifications. Target (#2734): always-on, required, owns all Session runtime state. Today the CLI still has direct Session-store and tmux operations.
_Avoid_: server, agent daemon

**Client**:
A program that pilots a Daemon: the CLI or the Web dashboard. Target (#2734): reaches the Daemon only through its API, holding no Session runtime state. Today the Web dashboard uses the Daemon for canonical Session state; the CLI cutover is incomplete.
_Avoid_: surface, frontend

**Remote Daemon**:
A Daemon on another machine that a Client reaches over the network. Plain Daemon means the one on the user's own machine.
_Avoid_: remote server, host

**Web serving**:
Target (#2734): a Daemon toggle for hosting the Web dashboard, not a separate process. Today the Daemon can run without Web serving; an explicit `aoe serve` request enables it.
_Avoid_: web server

**Supervisor**:
The part of the Daemon that owns the pool of Workers and tracks each one's phase.

**Worker**:
The detached per-Session process that owns the ACP adapter for a Structured view Session and outlives the Daemon.
_Avoid_: runner (the binary name only), structured-view worker

**Reconciler**:
The Daemon loop that brings saved Session state and the Supervisor's Worker pool into agreement: resume, idle stop, recovery.

**Idle reap**:
Stopping a Terminal view Session that has sat Idle past a threshold. Distinct from the Reconciler's idle stop for Workers.
_Avoid_: dormant

**Web dashboard**:
The browser Client the Daemon serves, protected by a rotating access token and an optional passphrase.
_Avoid_: PWA, web UI, structured view (that is a view, not a Client)

**Tunnel**:
A remote path to the Web dashboard through Cloudflare or Tailscale.

**Device**:
A signed-in browser login to the Web dashboard, listed in Connected Devices and revocable there.
_Avoid_: login session, subscription

**Principal**:
Target (#4394): who a request to the Daemon acts as: the local owner over the Unix socket, a Device, a Token holder presenting the access token, or a Proxy such as CityHall's. A Daemon serves exactly one human; users and accounts belong to CityHall. Today requests authenticate with a bearer token or passphrase login, and `read_only` is checked per request.
_Avoid_: user, owner, account

**Push subscription**:
A browser's Web Push registration, bound to the current access token and dropped when it rotates.
_Avoid_: device

### Plugins and events

**Plugin**:
A built-in feature compiled into the binary that the user can turn off, such as the Web dashboard (`aoe.web`).
_Avoid_: extension

**Plugin-originated prompt**:
A Prompt stamped with a sender id, as opposed to one the user typed. The Coordinator stamps its messages `aoe.coordinator`.

**Event**:
One ACP state change of a Session, persisted in the Event log and replayable.
_Avoid_: key event, session update

**Event log**:
The durable, append-only, per-Topic store of Events.
_Avoid_: event store, ledger

**Topic**:
The partition key of the Event log. For Session Events it is the Session id.

### Settings

**Setting**:
A user-facing value declared once, in one place. Every Client derives its view of it from that declaration.
_Avoid_: config field, option

**Repo config**:
Settings and Repo hooks a Repository carries in its own `.agent-of-empires` directory, layered over Profile and global Settings.
_Avoid_: repo override, project config

### Board

**Board**:
The Web dashboard view that lays Sessions out in Columns. One Board spans every Project; Dismissed Sessions are not on it.
_Avoid_: kanban, lanes view

**Column**:
A lane on the Board, either a Stage column or a Signal column, never both. Order on the Board is left to right and doubles as Signal priority.
_Avoid_: lane, bucket

**Card**:
A Session as shown on the Board. Sits in exactly one Column.
_Avoid_: approval card (different concept)

**Stage**:
The Column a user placed a Session in by hand. Sticky: it holds until the user moves the Card again.
_Avoid_: status, state, phase

**Signal**:
A transient condition on a Session raised by a Rule, such as a pending Approval or a PR with changes requested. While any Signal is active it decides the Card's Column; when it clears the Card returns to its Stage.
_Avoid_: alert, flag, automated status

**Rule**:
A predicate over a Session that, when true, raises a Signal for one Signal column. Rules only read; they never change a Stage.
_Avoid_: automation, trigger, filter
