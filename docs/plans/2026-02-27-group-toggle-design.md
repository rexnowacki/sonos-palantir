# Group Toggle Design

**Date:** 2026-02-27

## Goal

Add "Family Room" speaker to config and expose a `g` keybinding in the TUI that toggles all speakers between grouped and ungrouped.

## Approach

Detect grouped state from existing `App.speakers` data (no new state needed). On `g`:
- If any two speakers share a `group_coordinator` → call `POST /ungroup` with `"all"`
- Otherwise → call `POST /group` with `["all"]`

## Changes

1. **`daemon/config.yaml`** — add `family: "Family Room"` to speakers section
2. **`tui/src/api.rs`** — add `group_all()` and `ungroup_all()` methods to `ApiClient`
3. **`tui/src/app.rs`** — add `is_grouped()` method: returns true if any speaker's `group_coordinator` differs from its own name (i.e. it's a group member, not a solo coordinator)
4. **`tui/src/main.rs`** — add `g` keybinding that reads `app.is_grouped()` and calls the appropriate API method
5. **`tui/src/ui.rs`** — add `g` to the help bar
