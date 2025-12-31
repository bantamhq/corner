mod config;
mod storage;

use config::Config;
use storage::{Entry, EntryType, Line, TodoItem};

use std::io;
use std::path::PathBuf;
use chrono::{Days, Local, NaiveDate};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line as RatatuiLine, Span},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};

#[derive(PartialEq, Clone)]
enum Mode {
    Normal,
    Editing,
    Command,
    Todos,
}

// Wraps edit buffer with character-based cursor positioning.
// Rust strings are byte-indexed, but cursor movement must be character-based
// to correctly handle multi-byte UTF-8 (emojis, non-ASCII text).
struct CursorBuffer {
    content: String,
    cursor_char_pos: usize,
}

impl CursorBuffer {
    fn new(content: String) -> Self {
        let cursor_char_pos = content.chars().count();
        Self { content, cursor_char_pos }
    }

    fn empty() -> Self {
        Self { content: String::new(), cursor_char_pos: 0 }
    }

    fn cursor_byte_pos(&self) -> usize {
        self.content
            .char_indices()
            .nth(self.cursor_char_pos)
            .map(|(i, _)| i)
            .unwrap_or(self.content.len())
    }

    fn insert_char(&mut self, c: char) {
        let byte_pos = self.cursor_byte_pos();
        self.content.insert(byte_pos, c);
        self.cursor_char_pos += 1;
    }

    fn delete_char_before(&mut self) -> bool {
        if self.cursor_char_pos == 0 {
            return false;
        }
        let byte_pos = self.cursor_byte_pos();
        let prev_char_start = self.content[..byte_pos]
            .char_indices()
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.content.remove(prev_char_start);
        self.cursor_char_pos -= 1;
        true
    }

    fn move_left(&mut self) {
        if self.cursor_char_pos > 0 {
            self.cursor_char_pos -= 1;
        }
    }

    fn move_right(&mut self) {
        let char_count = self.content.chars().count();
        if self.cursor_char_pos < char_count {
            self.cursor_char_pos += 1;
        }
    }

    fn char_at_cursor(&self) -> Option<char> {
        self.content.chars().nth(self.cursor_char_pos)
    }

    fn text_before_cursor(&self) -> &str {
        let byte_pos = self.cursor_byte_pos();
        &self.content[..byte_pos]
    }

    fn text_after_cursor(&self) -> &str {
        let byte_pos = self.cursor_byte_pos();
        if byte_pos < self.content.len() {
            let char_len = self.char_at_cursor().map(|c| c.len_utf8()).unwrap_or(0);
            &self.content[byte_pos + char_len..]
        } else {
            ""
        }
    }

    fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    fn into_content(self) -> String {
        self.content
    }
}

struct App {
    current_date: NaiveDate,
    // `lines` preserves all content including non-entry text (blank lines, notes, etc.)
    // so the journal file can contain arbitrary markdown without data loss.
    // `entry_indices` maps selection index -> position in `lines` for navigable entries only.
    lines: Vec<Line>,
    entry_indices: Vec<usize>,
    selected: usize,
    edit_buffer: Option<CursorBuffer>,
    mode: Mode,
    command_buffer: String,
    should_quit: bool,
    status_message: Option<String>,
    todo_items: Vec<TodoItem>,
    todo_selected: usize,
}

impl App {
    fn new() -> io::Result<Self> {
        let current_date = Local::now().date_naive();
        let lines = storage::load_day_lines(current_date)?;
        let entry_indices = Self::compute_entry_indices(&lines);

        Ok(Self {
            current_date,
            lines,
            entry_indices,
            selected: 0,
            edit_buffer: None,
            mode: Mode::Normal,
            command_buffer: String::new(),
            should_quit: false,
            status_message: None,
            todo_items: Vec::new(),
            todo_selected: 0,
        })
    }

    fn compute_entry_indices(lines: &[Line]) -> Vec<usize> {
        lines
            .iter()
            .enumerate()
            .filter_map(|(i, line)| {
                if matches!(line, Line::Entry(_)) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect()
    }

    fn get_selected_entry(&self) -> Option<&Entry> {
        if self.entry_indices.is_empty() {
            return None;
        }
        let line_idx = self.entry_indices.get(self.selected)?;
        if let Line::Entry(entry) = &self.lines[*line_idx] {
            Some(entry)
        } else {
            None
        }
    }

    fn get_selected_entry_mut(&mut self) -> Option<&mut Entry> {
        if self.entry_indices.is_empty() {
            return None;
        }
        let line_idx = *self.entry_indices.get(self.selected)?;
        if let Line::Entry(entry) = &mut self.lines[line_idx] {
            Some(entry)
        } else {
            None
        }
    }

    fn save(&mut self) {
        if let Err(e) = storage::save_day_lines(self.current_date, &self.lines) {
            self.status_message = Some(format!("Failed to save: {}", e));
        }
    }

    fn goto_day(&mut self, date: NaiveDate) -> io::Result<()> {
        if date == self.current_date {
            return Ok(());
        }

        self.save();
        self.current_date = date;
        self.lines = storage::load_day_lines(date)?;
        self.entry_indices = Self::compute_entry_indices(&self.lines);
        self.selected = 0;
        self.edit_buffer = None;
        self.mode = Mode::Normal;

        Ok(())
    }

    fn prev_day(&mut self) -> io::Result<()> {
        if let Some(prev) = self.current_date.checked_sub_days(Days::new(1)) {
            self.goto_day(prev)?;
        }
        Ok(())
    }

    fn next_day(&mut self) -> io::Result<()> {
        if let Some(next) = self.current_date.checked_add_days(Days::new(1)) {
            self.goto_day(next)?;
        }
        Ok(())
    }

    fn goto_today(&mut self) -> io::Result<()> {
        self.goto_day(Local::now().date_naive())
    }

    fn add_entry(&mut self, entry: Entry, at_bottom: bool) {
        let insert_pos = if at_bottom || self.entry_indices.is_empty() {
            self.lines.len()
        } else {
            self.entry_indices[self.selected] + 1
        };

        self.lines.insert(insert_pos, Line::Entry(entry));
        self.entry_indices = Self::compute_entry_indices(&self.lines);

        self.selected = self.entry_indices
            .iter()
            .position(|&idx| idx == insert_pos)
            .unwrap_or(self.entry_indices.len().saturating_sub(1));

        self.edit_buffer = Some(CursorBuffer::empty());
        self.mode = Mode::Editing;
    }

    fn new_task(&mut self, at_bottom: bool) {
        self.add_entry(Entry::new_task(""), at_bottom);
    }

    fn new_note(&mut self, at_bottom: bool) {
        self.add_entry(Entry::new_note(""), at_bottom);
    }

    fn new_event(&mut self, at_bottom: bool) {
        self.add_entry(Entry::new_event(""), at_bottom);
    }

    fn commit_and_continue(&mut self) {
        let entry_type = self.get_selected_entry()
            .map(|e| e.entry_type.clone())
            .unwrap_or(EntryType::Task { completed: false });

        if let Some(buffer) = self.edit_buffer.take()
            && let Some(entry) = self.get_selected_entry_mut()
        {
            entry.content = buffer.into_content();
        }
        self.save();

        let new_entry = Entry {
            entry_type: match entry_type {
                EntryType::Task { .. } => EntryType::Task { completed: false },
                EntryType::Note => EntryType::Note,
                EntryType::Event => EntryType::Event,
            },
            content: String::new(),
        };

        self.add_entry(new_entry, false);
    }

    fn edit_selected(&mut self) {
        let content = self.get_selected_entry().map(|e| e.content.clone());
        if let Some(content) = content {
            self.edit_buffer = Some(CursorBuffer::new(content));
            self.mode = Mode::Editing;
        }
    }

    fn commit_edit(&mut self) {
        if let Some(buffer) = self.edit_buffer.take()
            && let Some(entry) = self.get_selected_entry_mut()
        {
            entry.content = buffer.into_content();
        }
        self.save();
        self.mode = Mode::Normal;
    }

    fn cancel_edit(&mut self) {
        // Delete empty entries on cancel - this cleans up placeholders created
        // when user starts a new entry then immediately cancels.
        if let Some(entry) = self.get_selected_entry()
            && entry.content.is_empty()
        {
            self.delete_selected();
        }
        self.edit_buffer = None;
        self.mode = Mode::Normal;
    }

    fn delete_selected(&mut self) {
        if self.entry_indices.is_empty() {
            return;
        }

        let line_idx = self.entry_indices[self.selected];
        self.lines.remove(line_idx);
        self.entry_indices = Self::compute_entry_indices(&self.lines);

        if !self.entry_indices.is_empty() && self.selected >= self.entry_indices.len() {
            self.selected = self.entry_indices.len() - 1;
        }
    }

    fn toggle_task(&mut self) {
        if let Some(entry) = self.get_selected_entry_mut() {
            entry.toggle_complete();
            self.save();
        }
    }

    fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    fn move_down(&mut self) {
        if !self.entry_indices.is_empty() && self.selected < self.entry_indices.len() - 1 {
            self.selected += 1;
        }
    }

    fn execute_command(&mut self) -> io::Result<()> {
        let cmd = self.command_buffer.trim();
        match cmd {
            "q" | "quit" => {
                self.save();
                self.should_quit = true;
            }
            "today" | "t" => {
                self.goto_today()?;
            }
            _ => {}
        }
        self.command_buffer.clear();
        self.mode = Mode::Normal;
        Ok(())
    }

    fn enter_todos_mode(&mut self) -> io::Result<()> {
        self.save();
        self.todo_items = storage::collect_all_todos()?;
        self.todo_selected = 0;
        self.mode = Mode::Todos;
        Ok(())
    }

    fn exit_todos_mode(&mut self) {
        self.mode = Mode::Normal;
    }

    fn todo_move_up(&mut self) {
        if self.todo_selected > 0 {
            self.todo_selected -= 1;
        }
    }

    fn todo_move_down(&mut self) {
        if !self.todo_items.is_empty() && self.todo_selected < self.todo_items.len() - 1 {
            self.todo_selected += 1;
        }
    }

    fn todo_jump_to_day(&mut self) -> io::Result<()> {
        if let Some(item) = self.todo_items.get(self.todo_selected) {
            let date = item.date;
            self.goto_day(date)?;
            self.mode = Mode::Normal;
        }
        Ok(())
    }

    fn todo_toggle(&mut self) -> io::Result<()> {
        let Some(item) = self.todo_items.get(self.todo_selected) else {
            return Ok(());
        };

        let date = item.date;
        let line_index = item.line_index;

        self.toggle_task_in_storage(date, line_index)?;
        self.todo_items[self.todo_selected].completed = !self.todo_items[self.todo_selected].completed;

        if date == self.current_date {
            self.reload_current_day()?;
        }

        Ok(())
    }

    fn toggle_task_in_storage(&self, date: NaiveDate, line_index: usize) -> io::Result<()> {
        let mut lines = storage::load_day_lines(date)?;
        if let Some(Line::Entry(entry)) = lines.get_mut(line_index) {
            entry.toggle_complete();
        }
        storage::save_day_lines(date, &lines)
    }

    fn reload_current_day(&mut self) -> io::Result<()> {
        self.lines = storage::load_day_lines(self.current_date)?;
        self.entry_indices = Self::compute_entry_indices(&self.lines);
        Ok(())
    }

    fn render_todos_view(&self) -> Vec<RatatuiLine<'static>> {
        let mut lines = Vec::new();
        let mut last_date: Option<NaiveDate> = None;

        for (idx, item) in self.todo_items.iter().enumerate() {
            if last_date != Some(item.date) {
                let date_str = item.date.format("%m/%d").to_string();
                lines.push(RatatuiLine::from(Span::styled(
                    date_str,
                    Style::default().fg(Color::Cyan)
                )));
                last_date = Some(item.date);
            }

            let is_selected = idx == self.todo_selected;
            let checkbox = if item.completed { "- [x] " } else { "- [ ] " };
            let content = format!("  {}{}", checkbox, item.content);

            let style = if item.completed {
                if is_selected {
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::REVERSED)
                } else {
                    Style::default().fg(Color::DarkGray)
                }
            } else if is_selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };

            lines.push(RatatuiLine::from(Span::styled(content, style)));
        }

        if lines.is_empty() {
            lines.push(RatatuiLine::from(Span::styled(
                "(no incomplete tasks)",
                Style::default().fg(Color::DarkGray)
            )));
        }

        lines
    }

    fn render_day_view(&self) -> Vec<RatatuiLine<'static>> {
        let mut lines = Vec::new();

        for (entry_idx, &line_idx) in self.entry_indices.iter().enumerate() {
            if let Line::Entry(entry) = &self.lines[line_idx] {
                let is_selected = entry_idx == self.selected;

                let content = if is_selected && self.mode == Mode::Editing {
                    if let Some(ref buffer) = self.edit_buffer {
                        format!("{}{}", entry.prefix(), buffer.content)
                    } else {
                        format!("{}{}", entry.prefix(), entry.content)
                    }
                } else {
                    format!("{}{}", entry.prefix(), entry.content)
                };

                let style = if is_selected {
                    if self.mode == Mode::Editing {
                        Style::default()
                    } else {
                        Style::default().add_modifier(Modifier::REVERSED)
                    }
                } else {
                    Style::default()
                };

                let style = if matches!(entry.entry_type, EntryType::Task { completed: true }) {
                    style.fg(Color::DarkGray)
                } else {
                    style
                };

                lines.push(RatatuiLine::from(Span::styled(content, style)));
            }
        }

        if lines.is_empty() {
            lines.push(RatatuiLine::from(Span::styled(
                "(no entries - press n for task, o for note, e for event)",
                Style::default().fg(Color::DarkGray)
            )));
        }

        lines
    }

    fn render_editing_cursor(&self, lines: &mut [RatatuiLine<'static>]) {
        if self.mode != Mode::Editing {
            return;
        }

        let Some(ref buffer) = self.edit_buffer else { return };
        let Some(entry) = self.get_selected_entry() else { return };

        if self.selected < lines.len() {
            let before = buffer.text_before_cursor();
            let cursor_char = buffer.char_at_cursor().unwrap_or(' ');
            let after = buffer.text_after_cursor();

            lines[self.selected] = RatatuiLine::from(vec![
                Span::raw(format!("{}{}", entry.prefix(), before)),
                Span::styled(
                    cursor_char.to_string(),
                    Style::default().add_modifier(Modifier::REVERSED)
                ),
                Span::raw(after.to_string()),
            ]);
        }
    }

    fn render_footer(&self) -> RatatuiLine<'static> {
        match self.mode {
            Mode::Command => {
                RatatuiLine::from(vec![
                    Span::styled(":", Style::default().fg(Color::Yellow)),
                    Span::raw(self.command_buffer.clone()),
                    Span::styled("█", Style::default().fg(Color::White)),
                ])
            }
            Mode::Editing => {
                RatatuiLine::from(vec![
                    Span::styled(" EDITING ", Style::default().fg(Color::Black).bg(Color::Green)),
                    Span::styled("  Enter", Style::default().fg(Color::DarkGray)),
                    Span::styled(" commit  ", Style::default().fg(Color::DarkGray)),
                    Span::styled("Tab", Style::default().fg(Color::DarkGray)),
                    Span::styled(" add another  ", Style::default().fg(Color::DarkGray)),
                    Span::styled("Esc", Style::default().fg(Color::DarkGray)),
                    Span::styled(" cancel", Style::default().fg(Color::DarkGray)),
                ])
            }
            Mode::Normal => {
                RatatuiLine::from(vec![
                    Span::styled(" NORMAL ", Style::default().fg(Color::Black).bg(Color::Blue)),
                    Span::styled("  n", Style::default().fg(Color::DarkGray)),
                    Span::styled(" task  ", Style::default().fg(Color::DarkGray)),
                    Span::styled("o", Style::default().fg(Color::DarkGray)),
                    Span::styled(" note  ", Style::default().fg(Color::DarkGray)),
                    Span::styled("e", Style::default().fg(Color::DarkGray)),
                    Span::styled(" event  ", Style::default().fg(Color::DarkGray)),
                    Span::styled("Enter", Style::default().fg(Color::DarkGray)),
                    Span::styled(" edit  ", Style::default().fg(Color::DarkGray)),
                    Span::styled("x", Style::default().fg(Color::DarkGray)),
                    Span::styled(" toggle  ", Style::default().fg(Color::DarkGray)),
                    Span::styled("d", Style::default().fg(Color::DarkGray)),
                    Span::styled(" delete  ", Style::default().fg(Color::DarkGray)),
                    Span::styled("Tab", Style::default().fg(Color::DarkGray)),
                    Span::styled(" todos", Style::default().fg(Color::DarkGray)),
                ])
            }
            Mode::Todos => {
                RatatuiLine::from(vec![
                    Span::styled(" TODOS ", Style::default().fg(Color::Black).bg(Color::Magenta)),
                    Span::styled("  j/k", Style::default().fg(Color::DarkGray)),
                    Span::styled(" navigate  ", Style::default().fg(Color::DarkGray)),
                    Span::styled("x", Style::default().fg(Color::DarkGray)),
                    Span::styled(" complete  ", Style::default().fg(Color::DarkGray)),
                    Span::styled("Enter", Style::default().fg(Color::DarkGray)),
                    Span::styled(" go to day  ", Style::default().fg(Color::DarkGray)),
                    Span::styled("Tab/Esc", Style::default().fg(Color::DarkGray)),
                    Span::styled(" back", Style::default().fg(Color::DarkGray)),
                ])
            }
        }
    }

    fn handle_command_key(&mut self, key: KeyCode) -> io::Result<()> {
        match key {
            KeyCode::Enter => self.execute_command()?,
            KeyCode::Esc => {
                self.command_buffer.clear();
                self.mode = Mode::Normal;
            }
            KeyCode::Backspace => {
                if self.command_buffer.is_empty() {
                    self.mode = Mode::Normal;
                } else {
                    self.command_buffer.pop();
                }
            }
            KeyCode::Char(c) => self.command_buffer.push(c),
            _ => {}
        }
        Ok(())
    }

    fn handle_normal_key(&mut self, key: KeyCode, shift: bool) -> io::Result<()> {
        match key {
            KeyCode::Char(':') => self.mode = Mode::Command,
            KeyCode::Tab => self.enter_todos_mode()?,
            KeyCode::Char('n') | KeyCode::Char('N') => self.new_task(!shift),
            KeyCode::Char('o') | KeyCode::Char('O') => self.new_note(!shift),
            KeyCode::Char('e') | KeyCode::Char('E') => self.new_event(!shift),
            KeyCode::Enter => self.edit_selected(),
            KeyCode::Char('x') => self.toggle_task(),
            KeyCode::Char('d') => self.delete_selected(),
            KeyCode::Up | KeyCode::Char('k') => self.move_up(),
            KeyCode::Down | KeyCode::Char('j') => self.move_down(),
            KeyCode::Char('h') | KeyCode::Char('[') => self.prev_day()?,
            KeyCode::Char('l') | KeyCode::Char(']') => self.next_day()?,
            KeyCode::Char('t') => self.goto_today()?,
            _ => {}
        }
        Ok(())
    }

    fn handle_editing_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Tab => self.commit_and_continue(),
            KeyCode::Enter => self.commit_edit(),
            KeyCode::Esc => self.cancel_edit(),
            KeyCode::Backspace => {
                if let Some(ref mut buffer) = self.edit_buffer
                    && !buffer.delete_char_before()
                    && buffer.is_empty()
                {
                    self.delete_selected();
                    self.edit_buffer = None;
                    self.mode = Mode::Normal;
                }
            }
            KeyCode::Left => {
                if let Some(ref mut buffer) = self.edit_buffer {
                    buffer.move_left();
                }
            }
            KeyCode::Right => {
                if let Some(ref mut buffer) = self.edit_buffer {
                    buffer.move_right();
                }
            }
            KeyCode::Char(c) => {
                if let Some(ref mut buffer) = self.edit_buffer {
                    buffer.insert_char(c);
                }
            }
            _ => {}
        }
    }

    fn handle_todos_key(&mut self, key: KeyCode) -> io::Result<()> {
        match key {
            KeyCode::Tab | KeyCode::Esc => self.exit_todos_mode(),
            KeyCode::Up | KeyCode::Char('k') => self.todo_move_up(),
            KeyCode::Down | KeyCode::Char('j') => self.todo_move_down(),
            KeyCode::Enter => self.todo_jump_to_day()?,
            KeyCode::Char('x') => self.todo_toggle()?,
            KeyCode::Char(':') => self.mode = Mode::Command,
            _ => {}
        }
        Ok(())
    }
}

fn main() -> Result<(), io::Error> {
    let args: Vec<String> = std::env::args().collect();

    if args.get(1).map(|s| s.as_str()) == Some("init") {
        return match Config::init() {
            Ok(true) => {
                println!("Created config file at: {}", config::get_config_path().display());
                Ok(())
            }
            Ok(false) => {
                println!("Config file already exists at: {}", config::get_config_path().display());
                Ok(())
            }
            Err(e) => {
                eprintln!("Failed to create config file: {}", e);
                Err(e)
            }
        };
    }

    let cli_file = args.get(1).map(PathBuf::from);
    let config = Config::load().unwrap_or_default();

    let journal_path = if let Some(path) = cli_file {
        if path.is_absolute() {
            path
        } else {
            std::env::current_dir()?.join(path)
        }
    } else {
        config.get_journal_path()
    };

    storage::set_journal_path(journal_path);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>) -> io::Result<()> {
    let mut app = App::new()?;

    loop {
        let date_display = app.current_date.format("%m/%d/%y").to_string();
        let is_todos_mode = app.mode == Mode::Todos;

        terminal.draw(|f| {
            let size = f.area();

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(1)])
                .split(size);

            let title = if is_todos_mode {
                " Todos ".to_string()
            } else {
                format!(" {} ", date_display)
            };

            let main_block = Block::default()
                .title(Span::styled(title, Style::default().fg(Color::Cyan)))
                .title_alignment(Alignment::Left)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White));

            let inner = main_block.inner(chunks[0]);
            let padded = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Min(1),
                    Constraint::Length(1),
                ])
                .split(inner);

            let content_area = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(2),
                    Constraint::Min(1),
                    Constraint::Length(2),
                ])
                .split(padded[1])[1];

            f.render_widget(main_block, chunks[0]);

            let mut lines = if is_todos_mode {
                app.render_todos_view()
            } else {
                app.render_day_view()
            };

            if !is_todos_mode {
                app.render_editing_cursor(&mut lines);

                if app.mode == Mode::Editing
                    && let Some(ref buffer) = app.edit_buffer
                    && let Some(entry) = app.get_selected_entry()
                {
                    let prefix_len = entry.prefix().chars().count();
                    let cursor_col = prefix_len + buffer.cursor_char_pos;

                    let cursor_x = content_area.x + cursor_col as u16;
                    let cursor_y = content_area.y + app.selected as u16;
                    if cursor_x < content_area.x + content_area.width
                        && cursor_y < content_area.y + content_area.height
                    {
                        f.set_cursor_position((cursor_x, cursor_y));
                    }
                }
            }

            let content = Paragraph::new(lines);
            f.render_widget(content, content_area);

            let footer = Paragraph::new(app.render_footer());
            f.render_widget(footer, chunks[1]);
        })?;

        if event::poll(std::time::Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            match app.mode {
                Mode::Command => app.handle_command_key(key.code)?,
                Mode::Normal => {
                    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
                    app.handle_normal_key(key.code, shift)?;
                }
                Mode::Editing => app.handle_editing_key(key.code),
                Mode::Todos => app.handle_todos_key(key.code)?,
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
