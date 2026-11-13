# Changelog

## 2026-11-12: personal-fork cleanup

The fork was trimmed to the `aoe serve` daemon, its web dashboard and the
coordinator. Removed:

- Terminal UI. Bare `aoe` now prints help; `aoe acp attach` is gone.
- Usage telemetry. Logging (`debug.log`, `aoe logs`) is unchanged.
- Version and update checks, including plugin updates.
- Third-party plugins: install, registry, plugin pages and panes. Only the
  built-in `aoe.web` remains (`aoe plugin list|enable|disable`).
- Tip of the day.
- Upstream infrastructure: website, marketing, Nix, governance files, release
  workflows and user guides.

Also added since the last upstream sync: a tested/untested tag on coordinator
deploys, and a coordinator Notes tab readable and editable through MCP
(`read_notes`, `write_notes`).

### Updating another machine

The app directory below is `~/.config/agent-of-empires` (or
`$XDG_CONFIG_HOME/agent-of-empires`) on both Linux and macOS.

1. Pull and rebuild:

   ```sh
   cd ~/agent-of-empires   # wherever the fork is checked out
   git pull
   cargo build --profile dev-release --features web
   ```

2. Restart the daemon however it runs on that machine, for example
   `systemctl --user restart aoe` on Linux. On macOS, stop and start
   `aoe serve` (or its launchd job).

3. Remove the config keys that no longer exist. Back up first:

   ```sh
   cd ~/.config/agent-of-empires
   cp -p config.toml config.toml.bak-cleanup
   aoe plugin list 2>&1 | grep -i unrecognized
   ```

   Delete every key that line lists from `config.toml`. Here it was:

   - the whole `[telemetry]` and `[updates]` sections
   - `[diff]`: `context_lines`, `split_view` (drop the `[diff]` header if it is
     left empty)
   - `[theme]`: `color_mode`
   - `[session]`: `click_action`, `confirm_before_quit`, `default_attach_mode`,
     `host_tab_title`, `live_send_exit_chord`, `live_send_leader`,
     `live_send_on_view_switch`, `mouse_capture`, `new_session_mode`,
     `preview_link_prefixes`, `show_activity_age`, `show_shortcut_bar`,
     `show_tips`, `sidebar_position`, `split_terminal_height_percent`,
     `split_terminal_min_height`, `split_terminal_min_width`, `strict_hotkeys`

   Rerun the `grep` command until it prints nothing. Saving any setting from
   the web dashboard also rewrites the file without them.

4. Delete files nothing reads any more:

   ```sh
   cd ~/.config/agent-of-empires
   rm -f plugin_events.db plugin_events.db-shm plugin_events.db-wal \
     .telemetry.lock update_cache.json
   rm -rf plugin-workers
   ```

5. Refresh the coordinator instructions. They are only written when missing,
   so copy them to pick up the deploy tag and notes tools:

   ```sh
   cp ~/agent-of-empires/src/session/coordinator/{CLAUDE,WORKFLOW}.md \
     ~/.config/agent-of-empires/coordinator/
   ```

   Then restart the coordinator session from the dashboard.

6. Optional: `aoe sounds install`, then enable sounds under Settings, Sound.

7. Reload the dashboard in the browser. If an MCP client reports a lost
   connection, reconnect it.
