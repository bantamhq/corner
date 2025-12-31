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
| `i` | Insert entry below current |
| `e` | Edit selected entry |
| `j/k` | Navigate down/up |
| `x` | Toggle task complete |
| `d` | Delete entry |
| `h/l` or `[/]` | Previous/next day |
| `t` | Go to today |
| `Tab` | Switch to Tasks view |
| `:goto` or `:gt` | Go to date (YYYY/MM/DD or MM/DD) |
| `:q` | Quit |

### Edit Mode

| Key | Action |
|-----|--------|
| `Enter` | Save and add new entry below |
| `Tab` | Toggle entry type (task/note/event) |
| `Esc` | Save and return to Daily |

### Tasks View

Shows incomplete tasks across all days.

| Key | Action |
|-----|--------|
| `j/k` | Move down/up |
| `x` | Toggle task |
| `Enter` | Go to selected day |
| `Tab` | Return to Daily |

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
