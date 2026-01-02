# Caliber

A terminal-based bullet journal for developers who want to capture tasks and notes without leaving their workflow.

![caliber screenshot](screenshot.png)

- **Keyboard-driven** - Vim-style navigation that feels natural in the terminal
- **Markdown-native** - Your journal is just a markdown file you can read, edit, or sync anywhere
- **Cross-day visibility** - Schedule tasks for future dates and see incomplete items bubble up
- **Powerful filtering** - Search across your entire journal with tags, entry types, and text

## Why Caliber?

If you spend your day in the terminal, context-switching to a GUI app just to jot down a task breaks your flow. Caliber lives where you work.

**Use cases:**
- Capture tasks and notes as they come up while coding
- Start each day with a quick review of what needs attention
- Track tasks that span multiple days without losing sight of them
- Tag entries for projects, contexts, or priorities and filter across your history

**What makes it different:**
- Your data is a plain markdown file - no lock-in, easy to sync, readable outside the app
- Three entry types (tasks, notes, events) match bullet journal methodology
- The `@date` syntax lets you schedule entries for future days
- Filtering works across your entire journal, not just the current day

## Installation

```bash
cargo install caliber
```

Requires Rust 1.85+ (edition 2024).

## Quick Start

```bash
caliber              # Open your journal
caliber /path/to.md  # Use a specific journal file
caliber init         # Create config file at ~/.config/caliber/config.toml
```

## Features

### Daily View

Your home base. View and manage entries for a single day with vim-style navigation.

- Navigate between days with `h/l` or `[/]`
- Jump to today with `t`
- Create entries with `Enter` (at end) or `o` (below current)
- Edit with `e`, delete with `d`, toggle task completion with `x`

**Later entries** appear at the top - these are incomplete entries from other days that target the current date using `@date` syntax. They bubble up so you don't lose track of scheduled tasks.

### Filter View

Search and filter across your entire journal. Press `/` to open filter mode.

- Find all incomplete tasks: `!tasks`
- Find entries with a tag: `#project`
- Combine filters: `!tasks #work` finds incomplete tasks tagged #work
- Edit, toggle, or delete entries directly from results
- Press `v` to jump to an entry's source day

### Entry Types

```markdown
- [ ] Incomplete task
- [x] Completed task
- A note
* An event
```

Toggle between types with `Shift+Tab` while editing.

### Tags and Dates

Tags (`#project`, `#urgent`) are highlighted in yellow and can be filtered.

Date references are highlighted in red. Use `@date` to schedule an entry for a future day:

```markdown
- [ ] Review quarterly report @1/15
- [ ] Send birthday card @2024/03/20
```

The entry will appear in the "Later" section when you view that date.

## Keyboard Reference

### Daily View

| Key | Action |
|-----|--------|
| `Enter` | New entry at end |
| `o` | New entry below current |
| `e` | Edit selected entry |
| `j/k` or `↓/↑` | Navigate down/up |
| `g/G` | Jump to first/last entry |
| `x` | Toggle task complete |
| `d` | Delete entry |
| `u` | Undo delete |
| `h/l` or `[/]` | Previous/next day |
| `t` | Go to today |
| `s` | Sort entries |
| `m` | Move mode (reorder entries) |
| `T/N/E` | Quick filter tasks/notes/events |
| `/` | Open filter mode |
| `?` | Show help |
| `:goto` or `:gt` | Go to date (YYYY/MM/DD or MM/DD) |
| `:q` | Quit |

### Move Mode

Reorder entries within a day.

| Key | Action |
|-----|--------|
| `j/k` or `↓/↑` | Move entry down/up |
| `m` or `Enter` | Save new order |
| `Esc` | Cancel |

### Edit Mode

| Key | Action |
|-----|--------|
| `Enter` | Save and exit |
| `Tab` | Save and add new entry below |
| `Shift+Tab` | Cycle entry type (task/note/event) |
| `←/→` | Move cursor |
| `Esc` | Cancel (discard changes) |

### Filter View

| Key | Action |
|-----|--------|
| `j/k` or `↓/↑` | Navigate down/up |
| `g/G` | Jump to first/last |
| `e` | Edit entry |
| `x` | Toggle task |
| `d` | Delete entry |
| `r` | Refresh results |
| `v` | Jump to entry's source day |
| `Enter` | Quick add entry to today |
| `/` | Edit filter query |
| `Esc` | Exit filter mode |
| `?` | Show help |

### Filter Syntax

| Pattern | Matches |
|---------|---------|
| `!tasks` or `!t` | Incomplete tasks |
| `!tasks/done` | Completed tasks |
| `!tasks/all` | All tasks |
| `!notes` or `!n` | Notes only |
| `!events` or `!e` | Events only |
| `#tag` | Entries with tag |
| `word` | Entries containing text |
| `not:#tag` | Entries without tag |
| `not:!tasks` | Exclude tasks |
| `not:word` | Entries not containing text |

Combine filters: `!tasks #work` finds incomplete tasks tagged #work.

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

## Configuration

Config file: `~/.config/caliber/config.toml`

```toml
# Custom journal path (default: ~/.config/caliber/journals/journal.md)
default_file = "/path/to/journal.md"

# Custom sort order for 's' command
sort_order = ["uncompleted", "notes", "events", "completed"]
```

Run `caliber init` to create the config file.

## License

MIT
