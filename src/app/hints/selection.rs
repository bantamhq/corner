use super::compute::date_display_items;
use super::display::first_selectable_index;
use super::types::HintContext;

const VISIBLE_HINTS: usize = 5;

fn adjust_scroll_offset(selected: usize, scroll_offset: &mut usize) {
    if selected >= *scroll_offset + VISIBLE_HINTS {
        *scroll_offset = selected + 1 - VISIBLE_HINTS;
    }
    if selected < *scroll_offset {
        *scroll_offset = selected;
    }
}

fn advance_selection(selected: &mut usize, count: usize, scroll_offset: &mut usize) {
    if count > 0 {
        *selected = (*selected + 1).min(count.saturating_sub(1));
        adjust_scroll_offset(*selected, scroll_offset);
    }
}

impl HintContext {
    pub fn select_next(&mut self) {
        match self {
            Self::Tags {
                matches,
                selected,
                scroll_offset,
                ..
            }
            | Self::SavedFilters {
                matches,
                selected,
                scroll_offset,
                ..
            } => advance_selection(selected, matches.len(), scroll_offset),
            Self::FilterTypes {
                matches,
                selected,
                scroll_offset,
                ..
            }
            | Self::DateOps {
                matches,
                selected,
                scroll_offset,
                ..
            } => advance_selection(selected, matches.len(), scroll_offset),
            Self::DateValues {
                matches,
                scope,
                selected,
                scroll_offset,
                ..
            } => {
                let items = date_display_items(scope, matches);
                if let Some((index, _)) = items
                    .iter()
                    .enumerate()
                    .skip((*selected).saturating_add(1))
                    .find(|(_, item)| item.selectable)
                {
                    *selected = index;
                    adjust_scroll_offset(*selected, scroll_offset);
                }
            }
            _ => {}
        }
    }

    pub fn select_prev(&mut self) {
        match self {
            Self::Tags {
                selected,
                scroll_offset,
                ..
            }
            | Self::FilterTypes {
                selected,
                scroll_offset,
                ..
            }
            | Self::DateOps {
                selected,
                scroll_offset,
                ..
            }
            | Self::SavedFilters {
                selected,
                scroll_offset,
                ..
            } => {
                *selected = selected.saturating_sub(1);
                adjust_scroll_offset(*selected, scroll_offset);
            }
            Self::DateValues {
                matches,
                scope,
                selected,
                scroll_offset,
                ..
            } => {
                if *selected == 0 {
                    return;
                }
                let items = date_display_items(scope, matches);
                if let Some((index, _)) = items
                    .iter()
                    .enumerate()
                    .take(*selected)
                    .rev()
                    .find(|(_, item)| item.selectable)
                {
                    *selected = index;
                    adjust_scroll_offset(*selected, scroll_offset);
                }
            }
            _ => {}
        }
    }

    #[must_use]
    pub fn with_previous_selection(self, previous: &HintContext) -> Self {
        match (self, previous) {
            (
                HintContext::Tags {
                    prefix,
                    matches,
                    selected: _,
                    ..
                },
                HintContext::Tags {
                    selected,
                    scroll_offset,
                    ..
                },
            ) => {
                let selected = (*selected).min(matches.len().saturating_sub(1));
                HintContext::Tags {
                    prefix,
                    matches,
                    selected,
                    scroll_offset: *scroll_offset,
                }
            }
            (
                HintContext::FilterTypes {
                    prefix,
                    matches,
                    selected: _,
                    ..
                },
                HintContext::FilterTypes {
                    selected,
                    scroll_offset,
                    ..
                },
            ) => {
                let selected = (*selected).min(matches.len().saturating_sub(1));
                HintContext::FilterTypes {
                    prefix,
                    matches,
                    selected,
                    scroll_offset: *scroll_offset,
                }
            }
            (
                HintContext::DateOps {
                    prefix,
                    matches,
                    selected: _,
                    ..
                },
                HintContext::DateOps {
                    selected,
                    scroll_offset,
                    ..
                },
            ) => {
                let selected = (*selected).min(matches.len().saturating_sub(1));
                HintContext::DateOps {
                    prefix,
                    matches,
                    selected,
                    scroll_offset: *scroll_offset,
                }
            }
            (
                HintContext::DateValues {
                    prefix,
                    scope,
                    matches,
                    selected: _,
                    ..
                },
                HintContext::DateValues {
                    selected,
                    scroll_offset,
                    ..
                },
            ) => {
                let items = date_display_items(&scope, &matches);
                let mut selected = (*selected).min(items.len().saturating_sub(1));
                if !items.get(selected).is_some_and(|item| item.selectable) {
                    selected = first_selectable_index(&items);
                }
                HintContext::DateValues {
                    prefix,
                    scope,
                    matches,
                    selected,
                    scroll_offset: *scroll_offset,
                }
            }
            (
                HintContext::SavedFilters {
                    prefix,
                    matches,
                    selected: _,
                    ..
                },
                HintContext::SavedFilters {
                    selected,
                    scroll_offset,
                    ..
                },
            ) => {
                let selected = (*selected).min(matches.len().saturating_sub(1));
                HintContext::SavedFilters {
                    prefix,
                    matches,
                    selected,
                    scroll_offset: *scroll_offset,
                }
            }
            (next, _) => next,
        }
    }

    #[must_use]
    pub fn selected_index(&self) -> usize {
        match self {
            Self::Tags { selected, .. }
            | Self::FilterTypes { selected, .. }
            | Self::DateOps { selected, .. }
            | Self::DateValues { selected, .. }
            | Self::SavedFilters { selected, .. } => *selected,
            _ => 0,
        }
    }

    #[must_use]
    pub fn scroll_offset(&self) -> usize {
        match self {
            Self::Tags { scroll_offset, .. }
            | Self::FilterTypes { scroll_offset, .. }
            | Self::DateOps { scroll_offset, .. }
            | Self::DateValues { scroll_offset, .. }
            | Self::SavedFilters { scroll_offset, .. } => *scroll_offset,
            _ => 0,
        }
    }
}
