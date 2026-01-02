# Caliber

A simple, bullet journal-inspired way to capture tasks and notes while you work in the terminal.

![caliber screenshot](screenshot.png)

## Installation

```bash
cargo install caliber
```

Requires Rust 1.85+ (edition 2024).

## Usage

```bash
caliber              # Open journal
caliber /path/to.md  # Use specific journal file
caliber init         # Create config file
```

### Daily View

| Key | Action |
|-----|--------|
| `Enter` | New entry at end |
| `o` | New entry below current |
| `e` | Edit selected entry |
| `j/k` | Navigate down/up |
| `g/G` | Jump to first/last entry |
| `x` | Toggle task complete |
| `d` | Delete entry |
| `u` | Undo delete |
| `h/l` or `[]` | Previous/next day |
| `t` | Go to today |
| `s` | Sort entries |
| `m` | Move mode (rearrange entries) |
| `T/N/E` | Quick filter tasks/notes/events |
| `/` | Filter mode |
| `?` | Show help |
| `:goto` or `:gt` | Go to date (YYYY/MM/DD or MM/DD) |
| `:q` | Quit |

### Move Mode

Rearrange entries within a day.

| Key | Action |
|-----|--------|
| `j/k` | Move entry down/up |
| `m` or `Enter` | Save new order |
| `Esc` | Cancel |

### Edit Mode

| Key | Action |
|-----|--------|
| `Enter` | Save and exit |
| `Tab` | Save and add new entry below (same type) |
| `Shift+Tab` | Toggle entry type (task/note/event) |
| `←/→` | Move cursor |
| `Esc` | Cancel (discard changes) |

### Filter Mode

Search and filter entries across all days. Press `/` to enter a filter query.

| Key | Action |
|-----|--------|
| `j/k` | Navigate down/up |
| `g/G` | Jump to first/last |
| `e` | Edit entry |
| `x` | Toggle task |
| `d` | Delete entry |
| `r` | Refresh results |
| `v` | Jump to day (view in context) |
| `/` | Edit filter query |
| `Esc` | Exit filter mode |
| `?` | Show help |

#### Filter Syntax

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
```

## Configuration

Config file: `~/.config/caliber/config.toml`

```toml
# Custom default journal path
default_file = "/path/to/journal.md"
```

Default journal location: `~/.config/caliber/journals/journal.md`

## License

MIT
