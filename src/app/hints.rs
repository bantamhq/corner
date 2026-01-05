use crate::registry::{COMMANDS, Command, FILTER_SYNTAX, FilterCategory, FilterSyntax};

/// What kind of hints to display
#[derive(Clone, Debug, PartialEq)]
pub enum HintContext {
    /// No hints to display
    Inactive,
    /// Tag hints from current journal
    Tags {
        prefix: String,
        matches: Vec<String>,
    },
    /// Command hints (from registry)
    Commands {
        prefix: String,
        matches: Vec<&'static Command>,
    },
    /// Subargument hints for a command
    SubArgs {
        prefix: String,
        matches: Vec<&'static str>,
        command: &'static Command,
    },
    /// Filter type hints (!tasks, !notes, etc.)
    FilterTypes {
        prefix: String,
        matches: Vec<&'static FilterSyntax>,
    },
    /// Date operation hints (@before:, @after:, @overdue)
    DateOps {
        prefix: String,
        matches: Vec<&'static FilterSyntax>,
    },
    /// Negation hints (not:#, not:!, not:word)
    Negation {
        prefix: String,
        matches: Vec<&'static FilterSyntax>,
    },
}

/// Which input context we're computing hints for
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HintMode {
    /// Command mode (:)
    Command,
    /// Filter query mode (/)
    Filter,
    /// Entry editing mode
    Entry,
}

impl HintContext {
    /// Compute hints based on current input buffer and mode
    #[must_use]
    pub fn compute(input: &str, mode: HintMode, journal_tags: &[String]) -> Self {
        match mode {
            HintMode::Command => Self::compute_command_hints(input),
            HintMode::Filter => Self::compute_filter_hints(input, journal_tags),
            HintMode::Entry => Self::compute_entry_hints(input, journal_tags),
        }
    }

    fn compute_command_hints(input: &str) -> Self {
        let trimmed = input.trim_start();
        let has_trailing_space = trimmed.ends_with(' ');
        let prefix = trimmed.trim_end();
        let words: Vec<&str> = prefix.split_whitespace().collect();
        let first_word = words.first().copied().unwrap_or("");

        let matched_command = COMMANDS
            .iter()
            .find(|c| c.name == first_word || c.aliases.contains(&first_word));

        if let Some(cmd) = matched_command {
            if !cmd.subargs.is_empty() {
                // words[0] = command, words[1] = arg0, words[2] = arg1, etc.
                let num_complete_args = words.len().saturating_sub(1);

                let (arg_position, current_arg) = if has_trailing_space {
                    (num_complete_args, "")
                } else if num_complete_args > 0 {
                    (num_complete_args - 1, words.last().copied().unwrap_or(""))
                } else {
                    return Self::Commands {
                        prefix: first_word.to_string(),
                        matches: vec![cmd],
                    };
                };

                if arg_position < cmd.subargs.len() {
                    let subarg = &cmd.subargs[arg_position];
                    let matches: Vec<&'static str> = subarg
                        .options
                        .iter()
                        .filter(|opt| opt.starts_with(current_arg))
                        .copied()
                        .collect();

                    if !has_trailing_space
                        && matches.len() == 1
                        && matches[0] == current_arg
                        && arg_position + 1 >= cmd.subargs.len()
                    {
                        return Self::Inactive;
                    }

                    if !matches.is_empty() {
                        return Self::SubArgs {
                            prefix: current_arg.to_string(),
                            matches,
                            command: cmd,
                        };
                    }
                    return Self::Inactive;
                } else {
                    return Self::Inactive;
                }
            }

            return Self::Commands {
                prefix: first_word.to_string(),
                matches: vec![cmd],
            };
        }

        let matches: Vec<&'static Command> = COMMANDS
            .iter()
            .filter(|c| {
                c.name.starts_with(prefix) || c.aliases.iter().any(|a| a.starts_with(prefix))
            })
            .collect();

        if matches.is_empty() {
            Self::Inactive
        } else {
            Self::Commands {
                prefix: prefix.to_string(),
                matches,
            }
        }
    }

    fn compute_tag_hints(input: &str, journal_tags: &[String]) -> Self {
        if input.ends_with(' ') {
            return Self::Inactive;
        }

        let current_token = input.split_whitespace().last().unwrap_or("");

        if let Some(tag_prefix) = current_token.strip_prefix('#') {
            let matches: Vec<String> = journal_tags
                .iter()
                .filter(|t| t.to_lowercase().starts_with(&tag_prefix.to_lowercase()))
                .cloned()
                .collect();

            if matches.is_empty()
                || (matches.len() == 1 && matches[0].eq_ignore_ascii_case(tag_prefix))
            {
                return Self::Inactive;
            }
            return Self::Tags {
                prefix: tag_prefix.to_string(),
                matches,
            };
        }

        Self::Inactive
    }

    fn compute_entry_hints(input: &str, journal_tags: &[String]) -> Self {
        Self::compute_tag_hints(input, journal_tags)
    }

    fn compute_filter_hints(input: &str, journal_tags: &[String]) -> Self {
        if input.ends_with(' ') {
            return Self::Inactive;
        }

        let current_token = input.split_whitespace().last().unwrap_or("");

        if current_token.starts_with('#') {
            return Self::compute_tag_hints(input, journal_tags);
        }

        if let Some(type_prefix) = current_token.strip_prefix('!') {
            let matches: Vec<&'static FilterSyntax> = FILTER_SYNTAX
                .iter()
                .filter(|f| f.category == FilterCategory::EntryType)
                .filter(|f| {
                    f.syntax[1..].starts_with(type_prefix)
                        || f.aliases.iter().any(|a| a[1..].starts_with(type_prefix))
                })
                .collect();

            if matches.is_empty() {
                return Self::Inactive;
            }
            return Self::FilterTypes {
                prefix: type_prefix.to_string(),
                matches,
            };
        }

        if let Some(date_prefix) = current_token.strip_prefix('@') {
            let matches: Vec<&'static FilterSyntax> = FILTER_SYNTAX
                .iter()
                .filter(|f| f.category == FilterCategory::DateOp)
                .filter(|f| {
                    let syntax_suffix = &f.syntax[1..];
                    // Match if typing the operator OR typing the date argument
                    syntax_suffix.starts_with(date_prefix) || date_prefix.starts_with(syntax_suffix)
                })
                .collect();

            if matches.is_empty() {
                return Self::Inactive;
            }
            return Self::DateOps {
                prefix: date_prefix.to_string(),
                matches,
            };
        }

        if let Some(neg_prefix) = current_token.strip_prefix("not:") {
            let matches: Vec<&'static FilterSyntax> = FILTER_SYNTAX
                .iter()
                .filter(|f| f.category == FilterCategory::Negation)
                .filter(|f| f.syntax[4..].starts_with(neg_prefix))
                .collect();

            if matches.is_empty() {
                return Self::Inactive;
            }
            return Self::Negation {
                prefix: neg_prefix.to_string(),
                matches,
            };
        }

        Self::Inactive
    }

    #[must_use]
    pub fn first_completion(&self) -> Option<String> {
        match self {
            Self::Inactive => None,
            Self::Tags { prefix, matches } => {
                matches.first().map(|tag| tag[prefix.len()..].to_string())
            }
            Self::Commands { prefix, matches } => matches
                .first()
                .map(|cmd| cmd.name[prefix.len()..].to_string()),
            Self::SubArgs {
                prefix, matches, ..
            } => matches.first().map(|opt| opt[prefix.len()..].to_string()),
            Self::FilterTypes { prefix, matches } => matches
                .first()
                .map(|f| f.syntax[1 + prefix.len()..].to_string()),
            Self::DateOps { prefix, matches } => matches
                .first()
                .map(|f| f.syntax[1 + prefix.len()..].to_string()),
            Self::Negation { prefix, matches } => matches
                .first()
                .map(|f| f.syntax[4 + prefix.len()..].to_string()),
        }
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        !matches!(self, Self::Inactive)
    }
}
