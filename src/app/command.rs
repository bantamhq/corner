use std::io::{self, Write};
use std::process::Command;

use crossterm::{
    execute,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};

use super::{App, InputMode, PromptContext};

impl App {
    pub fn execute_command(&mut self) -> io::Result<()> {
        let cmd = match &self.input_mode {
            InputMode::Prompt(PromptContext::Command { buffer }) => {
                buffer.content().trim().to_string()
            }
            _ => return Ok(()),
        };

        match cmd.as_str() {
            "quit" => {
                self.save();
                self.should_quit = true;
                self.input_mode = InputMode::Normal;
            }
            "project" => {
                self.open_project_interface();
            }
            "scratchpad" => {
                self.open_scratchpad()?;
                self.input_mode = InputMode::Normal;
            }
            _ => {
                if !cmd.is_empty() {
                    self.set_status(format!("Unknown command: {cmd}"));
                }
                self.input_mode = InputMode::Normal;
            }
        }
        Ok(())
    }

    fn open_scratchpad(&mut self) -> io::Result<()> {
        let path = self.config.get_scratchpad_path();

        let editor = std::env::var("VISUAL")
            .or_else(|_| std::env::var("EDITOR"))
            .unwrap_or_else(|_| "vi".to_string());

        let mut parts = editor.split_whitespace();
        let program = parts.next().unwrap_or("vi");
        let editor_args: Vec<&str> = parts.collect();

        self.save();

        disable_raw_mode()?;
        execute!(io::stdout(), LeaveAlternateScreen)?;

        let status = Command::new(program).args(&editor_args).arg(&path).status();

        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, Clear(ClearType::All))?;
        io::stdout().flush()?;

        self.needs_redraw = true;

        match status {
            Ok(exit) if exit.success() => {}
            Ok(_) => {
                self.set_status("Editor exited with error");
            }
            Err(e) => {
                self.set_status(format!("Failed to open editor: {e}"));
            }
        }

        Ok(())
    }
}
