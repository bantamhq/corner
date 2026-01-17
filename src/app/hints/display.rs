use ratatui::style::Color;

use crate::registry::DateScope;
use crate::ui::theme;

use super::compute::date_display_items;
use super::types::{HintContext, HintItem};

fn suffix_after(s: &str, prefix_len: usize) -> String {
    s.get(prefix_len..).unwrap_or_default().to_string()
}

impl HintContext {
    #[must_use]
    pub fn first_completion(&self) -> Option<String> {
        match self {
            Self::Inactive | Self::GuidanceMessage { .. } => None,
            Self::Tags {
                prefix,
                matches,
                selected,
                ..
            } => matches
                .get(*selected)
                .map(|t| suffix_after(t, prefix.len())),
            Self::Commands { prefix, matches } => matches
                .first()
                .map(|c| suffix_after(c.name, prefix.len())),
            Self::FilterTypes {
                prefix,
                matches,
                selected,
                ..
            }
            | Self::DateOps {
                prefix,
                matches,
                selected,
                ..
            } => matches
                .get(*selected)
                .map(|f| suffix_after(f.syntax, 1 + prefix.len())),
            Self::DateValues {
                prefix,
                matches,
                selected,
                scope,
                ..
            } => {
                let options = date_display_items(scope, matches);
                options.get(*selected).and_then(|item| {
                    if !item.selectable {
                        return None;
                    }
                    let trimmed = item.label.trim_start_matches('@');
                    Some(suffix_after(trimmed, prefix.len()))
                })
            }
            Self::SavedFilters {
                prefix,
                matches,
                selected,
                ..
            } => matches
                .get(*selected)
                .map(|f| suffix_after(f, prefix.len())),
            Self::Negation { inner } => inner.first_completion(),
        }
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        !matches!(self, Self::Inactive)
    }

    /// Get help text/description for the current hint context
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        let effective = match self {
            Self::Negation { inner } => inner.as_ref(),
            other => other,
        };

        match effective {
            Self::Commands { prefix, matches } if !prefix.is_empty() => {
                matches.first().map(|c| c.help)
            }
            Self::FilterTypes {
                prefix,
                matches,
                selected,
                ..
            } if !prefix.is_empty() => matches.get(*selected).map(|f| f.help),
            Self::DateOps {
                prefix,
                matches,
                selected,
                ..
            } if !prefix.is_empty() => matches.get(*selected).map(|f| f.help),
            Self::DateValues {
                prefix,
                matches,
                selected,
                ..
            } if !prefix.is_empty() => matches.get(*selected).map(|dv| dv.completion_hint),
            Self::DateValues {
                scope: DateScope::Filter,
                ..
            } => Some("Dates default to past. Append + for future."),
            _ => None,
        }
    }

    /// Get the display color for this hint context
    #[must_use]
    pub fn color(&self) -> Color {
        let effective = match self {
            Self::Negation { inner } => inner.as_ref(),
            other => other,
        };

        match effective {
            Self::Tags { .. } => theme::TAG,
            Self::Commands { .. } => theme::HUB_PRIMARY,
            Self::FilterTypes { .. } | Self::DateOps { .. } => theme::HINT_FILTER_TYPE,
            Self::DateValues { .. } => theme::PROJECTED_DATE,
            Self::SavedFilters { .. } => theme::HINT_FILTER_TYPE,
            Self::Inactive | Self::GuidanceMessage { .. } | Self::Negation { .. } => {
                theme::HINT_INACTIVE
            }
        }
    }

    /// Get formatted display items for rendering
    #[must_use]
    pub fn display_items(&self, negation_prefix: &str) -> Vec<HintItem> {
        let effective = match self {
            Self::Negation { inner } => inner.as_ref(),
            other => other,
        };

        match effective {
            Self::Inactive | Self::GuidanceMessage { .. } | Self::Negation { .. } => vec![],
            Self::Tags { matches, .. } => matches
                .iter()
                .map(|t| HintItem {
                    label: format!("{}#{t}", negation_prefix),
                    selectable: true,
                })
                .collect(),
            Self::Commands { matches, .. } => matches
                .iter()
                .map(|cmd| HintItem {
                    label: format!(":{}", cmd.name),
                    selectable: true,
                })
                .collect(),
            Self::FilterTypes { matches, .. } => matches
                .iter()
                .map(|f| HintItem {
                    label: format!("{}{}", negation_prefix, f.syntax),
                    selectable: true,
                })
                .collect(),
            Self::DateOps { matches, .. } => matches
                .iter()
                .map(|f| HintItem {
                    label: format!("{}{}", negation_prefix, f.syntax),
                    selectable: true,
                })
                .collect(),
            Self::DateValues { matches, scope, .. } => date_display_items(scope, matches),
            Self::SavedFilters { matches, .. } => matches
                .iter()
                .map(|f| HintItem {
                    label: format!("{}${f}", negation_prefix),
                    selectable: true,
                })
                .collect(),
        }
    }
}

pub(super) fn first_selectable_index(items: &[HintItem]) -> usize {
    items.iter().position(|item| item.selectable).unwrap_or(0)
}
