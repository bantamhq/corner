use crate::registry::{COMMANDS, Command, FILTER_SYNTAX, FilterCategory, FilterSyntax};

/// Date values available for autocomplete after @before: or @after:
pub static DATE_VALUES: &[(&str, &str)] = &[
    ("d", "Relative days (d1, d7, d30). Append + for future."),
    ("today", "Today"),
    ("tomorrow", "Tomorrow"),
    ("yesterday", "Yesterday"),
    ("mon", "Monday"),
    ("tue", "Tuesday"),
    ("wed", "Wednesday"),
    ("thu", "Thursday"),
    ("fri", "Friday"),
    ("sat", "Saturday"),
    ("sun", "Sunday"),
];

/// What kind of hints to display
#[derive(Clone, Debug, PartialEq)]
pub enum HintContext {
    /// No hints to display
    Inactive,
    /// Guidance text (help message only, shown at bottom)
    GuidanceMessage { message: &'static str },
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
    /// Date value hints (after @before: or @after:)
    DateValues {
        prefix: String,
        op: &'static str,
        matches: Vec<(&'static str, &'static str)>,
    },
    /// Saved filter hints ($name)
    SavedFilters {
        prefix: String,
        matches: Vec<String>,
    },
    /// Negation hints - wraps inner context for recursive hints
    Negation { inner: Box<HintContext> },
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
    pub fn compute(
        input: &str,
        mode: HintMode,
        journal_tags: &[String],
        saved_filters: &[String],
    ) -> Self {
        match mode {
            HintMode::Command => Self::compute_command_hints(input),
            HintMode::Filter => Self::compute_filter_hints(input, journal_tags, saved_filters),
            HintMode::Entry => Self::compute_entry_hints(input, journal_tags),
        }
    }

    fn compute_command_hints(input: &str) -> Self {
        let trimmed = input.trim_start();
        let has_trailing_space = trimmed.ends_with(' ');
        let prefix = trimmed.trim_end();
        let words: Vec<&str> = prefix.split_whitespace().collect();
        let first_word = words.first().copied().unwrap_or("");

        let matched_command = COMMANDS.iter().find(|c| c.name == first_word);

        if let Some(cmd) = matched_command {
            if !cmd.subargs.is_empty() {
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

                let max_args = cmd.subargs.len();

                if arg_position < max_args {
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
                        && arg_position + 1 >= max_args
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
            .filter(|c| c.name.starts_with(prefix))
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

    fn match_tags(prefix: &str, journal_tags: &[String]) -> Option<(String, Vec<String>)> {
        let matches: Vec<String> = journal_tags
            .iter()
            .filter(|t| t.to_lowercase().starts_with(&prefix.to_lowercase()))
            .cloned()
            .collect();

        if matches.is_empty() || (matches.len() == 1 && matches[0].eq_ignore_ascii_case(prefix)) {
            None
        } else {
            Some((prefix.to_string(), matches))
        }
    }

    fn compute_tag_hints(input: &str, journal_tags: &[String]) -> Self {
        if input.ends_with(' ') {
            return Self::Inactive;
        }

        let current_token = input.split_whitespace().last().unwrap_or("");

        if let Some(tag_prefix) = current_token.strip_prefix('#')
            && let Some((prefix, matches)) = Self::match_tags(tag_prefix, journal_tags)
        {
            return Self::Tags { prefix, matches };
        }

        Self::Inactive
    }

    fn compute_entry_hints(input: &str, journal_tags: &[String]) -> Self {
        Self::compute_tag_hints(input, journal_tags)
    }

    fn compute_filter_hints(
        input: &str,
        journal_tags: &[String],
        saved_filters: &[String],
    ) -> Self {
        if input.is_empty() {
            return Self::GuidanceMessage {
                message: "Type to search, or use ! @ # $ not: for filters",
            };
        }

        if input.ends_with(' ') {
            return Self::Inactive;
        }

        let current_token = input.split_whitespace().last().unwrap_or("");

        if let Some(neg_suffix) = current_token.strip_prefix("not:") {
            let inner = Self::compute_filter_token(neg_suffix, journal_tags, saved_filters);
            if matches!(inner, Self::Inactive) && neg_suffix.is_empty() {
                return Self::Negation {
                    inner: Box::new(Self::GuidanceMessage {
                        message: "! @ # or text to negate",
                    }),
                };
            }
            if matches!(inner, Self::Inactive) {
                return Self::Inactive;
            }
            return Self::Negation {
                inner: Box::new(inner),
            };
        }

        Self::compute_filter_token(current_token, journal_tags, saved_filters)
    }

    fn compute_filter_token(
        token: &str,
        journal_tags: &[String],
        saved_filters: &[String],
    ) -> Self {
        if let Some(tag_prefix) = token.strip_prefix('#')
            && let Some((prefix, matches)) = Self::match_tags(tag_prefix, journal_tags)
        {
            return Self::Tags { prefix, matches };
        }

        if let Some(type_prefix) = token.strip_prefix('!') {
            let matches: Vec<&'static FilterSyntax> = FILTER_SYNTAX
                .iter()
                .filter(|f| f.category == FilterCategory::EntryType)
                .filter(|f| {
                    f.syntax
                        .get(1..)
                        .is_some_and(|s| s.starts_with(type_prefix))
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

        if let Some(date_prefix) = token.strip_prefix('@') {
            if let Some(date_value) = date_prefix.strip_prefix("before:") {
                return Self::compute_date_value_hints(date_value, "before");
            }
            if let Some(date_value) = date_prefix.strip_prefix("after:") {
                return Self::compute_date_value_hints(date_value, "after");
            }

            let matches: Vec<&'static FilterSyntax> = FILTER_SYNTAX
                .iter()
                .filter(|f| f.category == FilterCategory::DateOp)
                .filter(|f| {
                    f.syntax
                        .get(1..)
                        .is_some_and(|s| s.starts_with(date_prefix))
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

        if let Some(filter_prefix) = token.strip_prefix('$') {
            let matches: Vec<String> = saved_filters
                .iter()
                .filter(|f| f.to_lowercase().starts_with(&filter_prefix.to_lowercase()))
                .cloned()
                .collect();

            if matches.is_empty()
                || (matches.len() == 1 && matches[0].eq_ignore_ascii_case(filter_prefix))
            {
                return Self::Inactive;
            }
            return Self::SavedFilters {
                prefix: filter_prefix.to_string(),
                matches,
            };
        }

        Self::Inactive
    }

    fn is_valid_relative_days(rest: &str) -> bool {
        if rest.is_empty() {
            return true;
        }
        let (digits, suffix) = if let Some(d) = rest.strip_suffix('+') {
            (d, true)
        } else {
            (rest, false)
        };
        if digits.is_empty() {
            return suffix;
        }
        digits.len() <= 3 && digits.chars().all(|c| c.is_ascii_digit()) && !digits.starts_with('0')
    }

    fn compute_date_value_hints(value_prefix: &str, op: &'static str) -> Self {
        let value_lower = value_prefix.to_lowercase();

        if let Some(rest) = value_lower.strip_prefix('d')
            && Self::is_valid_relative_days(rest)
        {
            return Self::DateValues {
                prefix: value_prefix.to_string(),
                op,
                matches: vec![("d", "Relative days (d1, d7, d30). Append + for future.")],
            };
        }

        let matches: Vec<(&'static str, &'static str)> = DATE_VALUES
            .iter()
            .filter(|(syntax, _)| syntax.starts_with(&value_lower))
            .copied()
            .collect();

        if matches.is_empty() {
            return Self::Inactive;
        }

        Self::DateValues {
            prefix: value_prefix.to_string(),
            op,
            matches,
        }
    }

    fn suffix_after(s: &str, prefix_len: usize) -> String {
        s.get(prefix_len..).unwrap_or("").to_string()
    }

    #[must_use]
    pub fn first_completion(&self) -> Option<String> {
        match self {
            Self::Inactive | Self::GuidanceMessage { .. } => None,
            Self::Tags { prefix, matches } => {
                matches.first().map(|t| Self::suffix_after(t, prefix.len()))
            }
            Self::Commands { prefix, matches } => matches
                .first()
                .map(|c| Self::suffix_after(c.name, prefix.len())),
            Self::SubArgs {
                prefix, matches, ..
            } => matches.first().map(|o| Self::suffix_after(o, prefix.len())),
            Self::FilterTypes { prefix, matches } => matches
                .first()
                .map(|f| Self::suffix_after(f.syntax, 1 + prefix.len())),
            Self::DateOps { prefix, matches } => matches
                .first()
                .map(|f| Self::suffix_after(f.syntax, 1 + prefix.len())),
            Self::DateValues {
                prefix, matches, ..
            } => matches
                .first()
                .map(|(s, _)| Self::suffix_after(s, prefix.len())),
            Self::SavedFilters { prefix, matches } => {
                matches.first().map(|f| Self::suffix_after(f, prefix.len()))
            }
            Self::Negation { inner } => inner.first_completion(),
        }
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        !matches!(self, Self::Inactive)
    }
}
