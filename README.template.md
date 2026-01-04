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

### Global and Project Journals

Caliber supports two journal contexts:

- **Global journal** - Your default journal stored in `~/.config/caliber/global_journal.md`
- **Project journal** - A project-specific journal at `.caliber/journal.md` in your project root

When you launch Caliber in a git repository, it automatically detects or offers to create a project journal. Toggle between journals with `` ` `` (backtick). The bottom-right corner shows `[GLOBAL]` or `[PROJECT]` to indicate which journal is active.

### Daily View

Your home base. View and manage entries for a single day with vim-style navigation.

- Navigate between days with `h/l` or `[/]`
- Jump to today with `t`
- Create entries with `Enter` (at end) or `o` (below current)
- Edit with `i`, delete with `x`, toggle task completion with `c`

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

**Favorite tags** let you quickly filter by commonly-used tags. Press `0-9` to instantly filter by a favorite tag. Type `#0` through `#9` while editing to auto-expand to the tag name. Default: 1=feature, 2=bug, 3=idea.

**Saved filters** let you define reusable filter queries. Use `$name` in any filter to expand it:
- `$t` → expands to `!tasks`
- `$someday` → expands to your custom filter
- Combine them: `$t #work` → `!tasks #work`

Date references are highlighted in red. Use `@date` to schedule an entry for a future day:

```markdown
- [ ] Review quarterly report @1/15
- [ ] Send birthday card @2024/03/20
```

**Natural date shortcuts** are converted automatically when you save:
- `@tomorrow` / `@yesterday` - converts to tomorrow/yesterday's date
- `@next-monday` (or `@next-mon`) - converts to next Monday
- `@last-friday` (or `@last-fri`) - converts to last Friday
- `@3d` / `@-3d` - converts to 3 days from now / 3 days ago

The entry will appear in the "Later" section when you view that date.

## Keyboard Reference

### Daily Mode

<!-- GENERATED:DAILY_KEYS -->

### Filter Mode

<!-- GENERATED:FILTER_KEYS -->

### Edit Mode

<!-- GENERATED:EDIT_KEYS -->

### Reorder Mode

Reorder entries within a day.

<!-- GENERATED:REORDER_KEYS -->

### Text Editing

These shortcuts work in all text input modes (Edit, Command, and Filter query):

<!-- GENERATED:TEXT_EDITING_KEYS -->

### Commands

<!-- GENERATED:COMMANDS -->

### Filter Syntax

<!-- GENERATED:FILTER_SYNTAX -->

DATE in filters supports natural language: `tomorrow`, `yesterday`, `next-mon`, `last-fri`, `7d`, `-7d`, plus standard formats.

**Combining filters:**

- Positive filters use AND: `!tasks #work meeting` finds incomplete tasks tagged #work containing "meeting"
- Negative filters use OR: `not:#work not:#personal` excludes entries with either tag
- Only one entry type allowed (use `not:` for "or" logic: `not:!events` shows tasks and notes)
- Only one `@before:` and one `@after:` allowed per query

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
# Custom global journal path (default: ~/.config/caliber/global_journal.md)
global_file = "/path/to/journal.md"

# Custom sort order for 's' command
sort_order = ["uncompleted", "notes", "events", "completed"]

# Favorite tags - press 0-9 to filter, type #0-#9 while editing to auto-expand
# Default: 1="feature", 2="bug", 3="idea"
[favorite_tags]
1 = "feature"
2 = "bug"
3 = "idea"

# Saved filters - use $name in filter queries to expand
[filters]
t = "!tasks"
n = "!notes"
e = "!events"
someday = "!tasks #someday"
stale = "!tasks @before:-7d"
```

Run `caliber init` to create an empty config file.

## License

MIT
