# Caliber

A terminal-based task journal for developers. Capture ideas without leaving your workflow, find them when you need them.

![caliber demo](examples/hero.gif)

- **Keyboard-driven** — Vim-style navigation that feels natural in the terminal
- **Zero-friction capture** — Press enter and start typing, organize later or don't
- **Project and hub journals** — Keep a journal per repo plus one that follows you everywhere
- **Schedule forward** — Tag a task with a future date and it surfaces when that day comes
- **Filter everything** — Search your entire history by tags, dates, or text
- **Plain markdown** — Commit it, edit it anywhere, let AI agents read it for context

## Why Caliber?

If you live in the terminal, context-switching to a GUI app just to jot down a task breaks your flow. Caliber lives where you work.

Tab over, press enter, start typing. That's it. No forms, no fields, no friction.

Keep a journal per project and a hub journal that follows you everywhere. When you're deep in a repo and remember you need to pay a bill, one keystroke switches context. No need to organize your task manager around your projects — the structure is already there.

Tasks and notes stay where you wrote them. When you want to review what's open, filter across your entire history by tags, dates, or text. When you don't have time to organize, don't. Things surface when you need them.

Your journal is a plain markdown file. Commit it to your repo, read it on your phone, edit it in another app, let your AI coding agent read it for context. No lock-in.

## Installation

```bash
cargo install caliber
```

Requires Rust 1.85+ (edition 2024).

## Quick Start

```bash
caliber                  # Open your journal
```

Launch Caliber in a git repository and it will prompt you to create a project journal. Otherwise, you'll start in your hub journal.

**The basics:**

| Key | Action |
|-----|--------|
| `Enter` | Add a new entry |
| `j` / `k` or `↓` / `↑` | Move up and down |
| `h` / `l` or `[` / `]` | Previous / next day |
| `Shift+Tab` | Cycle type: task → note → event |
| `/` | Open filter |
| `` ` `` | Toggle between project and hub journal |
| `?` | Help |

**Entry types:**

```markdown
- [ ] A task
- [x] A completed task
- A note
* An event
```

That's enough to get started. The sections below go deeper.

## Daily View

![daily view demo](examples/entry-ops.gif)

Your home base. Each day is a page in your journal. Navigate between days with `h` / `l` or `[` / `]`, or jump to any date with `\` to open the datepicker.

Entries from other days that target today appear at the top — one-time `@date` entries and recurring `@every-*` entries surface when the day comes.

| Key | Action |
|-----|--------|
| `h` / `l` or `[` / `]` | Previous / next day |
| `t` | Jump to today |
| `\` | Open datepicker |
| `Enter` | New entry at end |
| `o` / `O` | New entry below / above cursor |
| `i` | Edit selected entry |
| `d` | Delete entry |
| `c` | Toggle task completion |
| `z` | Hide / show completed tasks |
| `s` | Sort entries by type |

### Datepicker

![datepicker demo](examples/datepicker.gif)

Press `\` to open a calendar for quick navigation. Days with entries are highlighted. Navigate with `h/j/k/l` or arrow keys, switch months with `[` and `]`, and press `Enter` to jump to the selected date.

## Filtering

![filter view demo](examples/filter.gif)

Press `/` to search across your entire journal. Filter by entry type, tags, dates, or text — and combine them.

**Examples:**

| Query | Result |
|-------|--------|
| `!tasks` | All incomplete tasks |
| `#work` | Entries tagged #work |
| `!tasks #work` | Incomplete tasks tagged #work |
| `@after:yesterday @before:tomorrow` | Entries from today |
| `@recurring` | All recurring entries |
| `@later` | All entries with @date patterns |
| `meeting #standup` | Entries containing "meeting" with #standup tag |
| `not:#work` | Entries without #work tag |

Edit, toggle, or delete entries directly from filter results. Press `Enter` to quick-add a new entry to today without leaving the filter view.

### Date Filters

Dates in filters support natural language:

- `today`, `tomorrow`, `yesterday`
- `next-mon`, `last-fri`
- `7d` (7 days from now), `-7d` (7 days ago)
- Standard formats: `1/15`, `2025/01/15`

### Combining Filters

- Filters combine with AND: `!tasks #work meeting` finds incomplete tasks tagged #work containing "meeting"
- Entry type filters combine with OR: `!tasks !notes` shows both tasks and notes
- Negative filters combine with OR: `not:#work not:#personal` excludes entries with either tag
- One `@before:` and one `@after:` allowed per query

## Task Management

![selection mode demo](examples/selection.gif)

### Selection Mode

Press `v` to enter selection mode for batch operations. Select multiple entries, then act on all of them at once.

| Key | Action |
|-----|--------|
| `v` | Toggle selection on current entry |
| `V` | Select range from last selection to cursor |
| `d` | Delete all selected |
| `c` | Toggle completion on all selected |
| `y` | Yank (copy) all selected |
| `Esc` | Exit selection mode |

### Clipboard

`y` copies the selected entry (or entries in selection mode) to your system clipboard. `p` pastes clipboard content as new entries below the cursor — multi-line paste creates multiple entries.

### Tags

Add tags anywhere in an entry with `#tagname`. Remove the last tag from an entry with `x`, or all tags with `X`.

### Scheduling

Use `@date` to schedule an entry for a future day:

```markdown
- [ ] Review quarterly report @1/15
- [ ] Send birthday card @next-friday
```

Natural date shortcuts are converted when you save:

- `@today`, `@tomorrow`, `@yesterday`
- `@next-monday` (or `@next-mon`)
- `@last-friday` (or `@last-fri`)
- `@3d` (3 days from now), `@-3d` (3 days ago)

The entry appears in the "Later" section when you view the target date.

### Recurring Entries

Use `@every-*` patterns for entries that repeat on a schedule:

```markdown
- [ ] Daily standup @every-day
- [ ] Weekly review @every-monday
- [ ] Pay rent @every-1
```

Available patterns:

| Pattern | Recurrence |
|---------|------------|
| `@every-day` | Every day |
| `@every-weekday` | Monday through Friday |
| `@every-mon` through `@every-sun` | Weekly on that day |
| `@every-1` through `@every-31` | Monthly on that date |

Recurring entries appear on all matching days. When you complete one, a copy is marked done for today while the source entry stays open for future occurrences.

To edit or delete a recurring entry, press `o` to jump to its source.

### Undo / Redo

Most actions can be undone with `u` and redone with `U`. Undo history clears when you navigate to a different day, filter, or journal.

## Favorite Tags

![favorite tags demo](examples/favorite-tags.gif)

Slots `1-9` give you quick access to commonly-used tags.

| Key | Action |
|-----|--------|
| `1-9` | Filter by favorite tag |
| `Shift+1-9` | Append favorite tag to selected entry |
| `#1-9` (in edit mode) | Expand to favorite tag name |

Configure your favorites in `config.toml`:

```toml
[favorite_tags]
1 = "feature"
2 = "bug"
3 = "idea"
4 = "next"
```

**Example workflow:** Filter all open tasks with `/!tasks`, enter selection mode with `v`, select what you want to work on today, press `Shift+4` to tag them all with `#next`.

## Saved Filters

Define reusable filter queries in your config, then use `$name` to expand them.

```toml
[filters]
t = "!tasks"
n = "!notes"
next = "!tasks #next"
stale = "!tasks @before:-7d"
```

Now `/$next` expands to `!tasks #next`. Combine them: `$t #work` expands to `!tasks #work`.

## Keyboard Reference

### Daily Mode

<!-- GENERATED:DAILY_KEYS -->

### Filter Mode

<!-- GENERATED:FILTER_KEYS -->

### Edit Mode

<!-- GENERATED:EDIT_KEYS -->

### Reorder Mode

Press `r` to manually arrange entries within a day.

<!-- GENERATED:REORDER_KEYS -->

### Selection Mode

Press `v` for batch operations on multiple entries.

<!-- GENERATED:SELECTION_KEYS -->

### Date Mode

Press `\` to open the calendar picker.

<!-- GENERATED:DATE_KEYS -->

### Text Editing

These shortcuts work in all text input modes (Edit, Command, and Filter query):

<!-- GENERATED:TEXT_EDITING_KEYS -->

### Commands

<!-- GENERATED:COMMANDS -->

### Filter Syntax

<!-- GENERATED:FILTER_SYNTAX -->

## Journal Format

Caliber uses a markdown-compatible format:

```markdown
# 2025/01/15
- [ ] Incomplete task
- [x] Completed task
- A note
* An event

# 2025/01/16
- [ ] Follow up on review @1/20
- Meeting notes from standup #daily
```

Empty days are automatically removed. Non-entry lines (headers, blank lines, raw text) are preserved.

## Configuration

Config file: `~/.config/caliber/config.toml`

Run `caliber init` to create an empty config file. Configuration is optional — Caliber works out of the box with sensible defaults.

```toml
# Custom hub journal path (default: ~/.config/caliber/hub_journal.md)
hub_file = "/path/to/journal.md"

# Custom scratchpad path (default: ~/.config/caliber/scratchpad.md)
scratchpad_file = "~/notes/scratchpad.md"

# Start with completed tasks hidden (default: false)
hide_completed = false

# Custom sort order for 's' command
# Options: completed, uncompleted, notes, events
sort_order = ["uncompleted", "notes", "events", "completed"]

# Date format in header (default: "%m/%d/%y")
header_date_format = "%m/%d/%y"

# Default filter when pressing '/' (default: "!tasks")
default_filter = "!tasks"

# Favorite tags - press 1-9 to filter, Shift+1-9 to append, #1-9 to expand
[favorite_tags]
1 = "feature"
2 = "bug"
3 = "idea"

# Saved filters - use $name in filter queries to expand
[filters]
t = "!tasks"
n = "!notes"
e = "!events"
```

### Project Configuration

Project config at `.caliber/config.toml` overlays the base config — project values override matching keys, while unset options inherit from your base `config.toml`. Similarly, `hub_config.toml` can override settings specifically for your hub journal.

When you launch Caliber in a git repository, it will offer to create a project journal for you. If you declined or want to create one outside a git repo, run `caliber init project`.

## License

MIT
