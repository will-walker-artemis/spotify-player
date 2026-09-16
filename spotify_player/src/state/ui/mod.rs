use crate::{
    config::{self, Theme},
    key,
    ui::{self, Orientation},
    utils::filtered_items_from_query,
};

#[cfg(feature = "image")]
use crate::ui::cover_image::CoverImage;
#[cfg(feature = "image")]
use ratatui_image::picker::Picker;

use ratatui::layout::Rect;

pub type UIStateGuard<'a> = parking_lot::MutexGuard<'a, UIState>;

mod page;
mod popup;

pub use page::*;
pub use popup::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseTarget {
    ResumePause,
    LibraryWindow {
        focus: LibraryFocusState,
        first_item: usize,
        item_count: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouseArea {
    pub rect: Rect,
    pub target: MouseTarget,
}

impl MouseArea {
    pub fn contains(&self, column: u16, row: u16) -> bool {
        column >= self.rect.x
            && column < self.rect.x.saturating_add(self.rect.width)
            && row >= self.rect.y
            && row < self.rect.y.saturating_add(self.rect.height)
    }

    pub fn item_at(&self, row: u16) -> Option<usize> {
        let MouseTarget::LibraryWindow {
            first_item,
            item_count,
            ..
        } = self.target
        else {
            return None;
        };

        let item = first_item + usize::from(row.saturating_sub(self.rect.y));
        (item < item_count).then_some(item)
    }
}

#[cfg(feature = "image")]
#[derive(Default)]
pub struct ImageRenderInfo {
    pub url: String,
    pub render_area: ratatui::layout::Rect,
    pub state: Option<CoverImage>,
}

#[cfg(feature = "image")]
impl std::fmt::Debug for ImageRenderInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageRenderInfo")
            .field("url", &self.url)
            .field("render_area", &self.render_area)
            .field("state", &self.state.is_some())
            .finish()
    }
}

/// Application's UI state
#[derive(Debug)]
pub struct UIState {
    pub is_running: bool,
    pub theme: config::Theme,
    pub input_key_sequence: key::KeySequence,
    pub orientation: ui::Orientation,

    pub history: Vec<PageState>,
    pub popup: Option<PopupState>,

    /// The rectangle representing the playback progress bar,
    /// which is mainly used to handle mouse click events (for seeking command)
    pub playback_progress_bar_rect: ratatui::layout::Rect,

    /// Interactive regions populated during the most recent render.
    pub mouse_areas: Vec<MouseArea>,

    /// Count prefix for vim-style navigation (e.g., 5j, 10k)
    pub count_prefix: Option<usize>,

    #[cfg(feature = "image")]
    pub last_cover_image_render_info: ImageRenderInfo,

    #[cfg(feature = "image")]
    pub picker: Picker,
}

impl UIState {
    pub fn current_page(&self) -> &PageState {
        self.history.last().expect("non-empty history")
    }

    pub fn current_page_mut(&mut self) -> &mut PageState {
        self.history.last_mut().expect("non-empty history")
    }

    pub fn new_search_popup(&mut self) {
        self.current_page_mut().select(0);
        self.popup = Some(PopupState::Search {
            query: String::new(),
        });
    }

    pub fn new_page(&mut self, page: PageState) {
        self.popup = None;
        if let Some(current_page) = self.history.last() {
            if &page == current_page {
                return;
            }
        }
        self.history.push(page);
    }

    /// Return whether there exists a focused popup.
    ///
    /// Currently, only search popup is not focused when it's opened.
    pub fn has_focused_popup(&self) -> bool {
        match self.popup.as_ref() {
            None => false,
            Some(popup) => !matches!(popup, PopupState::Search { .. }),
        }
    }

    /// Get a list of items possibly filtered by a search query if exists a search popup
    pub fn search_filtered_items<'a, T: std::fmt::Display>(&self, items: &'a [T]) -> Vec<&'a T> {
        match self.popup {
            Some(PopupState::Search { ref query }) => filtered_items_from_query(query, items),
            _ => items.iter().collect::<Vec<_>>(),
        }
    }
}

impl Default for UIState {
    fn default() -> Self {
        Self {
            is_running: true,
            theme: Theme::default(),
            input_key_sequence: key::KeySequence { keys: vec![] },
            orientation: match crossterm::terminal::size() {
                Ok((columns, rows)) => ui::Orientation::from_size(columns, rows),
                Err(err) => {
                    tracing::warn!("Unable to get terminal size, error: {err:#}");
                    Orientation::default()
                }
            },

            history: vec![PageState::Library {
                state: LibraryPageUIState::new(),
            }],
            popup: None,

            playback_progress_bar_rect: Rect::default(),

            mouse_areas: Vec::new(),

            count_prefix: None,

            #[cfg(feature = "image")]
            last_cover_image_render_info: ImageRenderInfo::default(),

            // Will be reinitialize later in ui/mod.rs after init_ui()
            #[cfg(feature = "image")]
            picker: Picker::halfblocks(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library_area(first_item: usize, item_count: usize) -> MouseArea {
        MouseArea {
            rect: Rect::new(10, 5, 20, 4),
            target: MouseTarget::LibraryWindow {
                focus: LibraryFocusState::Playlists,
                first_item,
                item_count,
            },
        }
    }

    #[test]
    fn mouse_area_contains_only_points_inside_rect() {
        let area = library_area(0, 4);

        assert!(area.contains(10, 5));
        assert!(area.contains(29, 8));
        assert!(!area.contains(9, 5));
        assert!(!area.contains(30, 8));
        assert!(!area.contains(10, 9));
    }

    #[test]
    fn mouse_area_maps_rows_to_scrolled_items() {
        let area = library_area(7, 20);

        assert_eq!(area.item_at(5), Some(7));
        assert_eq!(area.item_at(8), Some(10));
    }

    #[test]
    fn mouse_area_ignores_rows_after_last_item() {
        let area = library_area(7, 9);

        assert_eq!(area.item_at(6), Some(8));
        assert_eq!(area.item_at(7), None);
    }
}
