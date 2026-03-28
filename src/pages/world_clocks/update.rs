// SPDX-License-Identifier: MIT
//
// World clocks update logic: search, add, remove, detail navigation,
// edit mode, and drag-to-reorder.

use super::model::*;
use super::{tz_city_name, Message};

impl WorldClocksState {
    pub fn update(&mut self, message: Message) {
        match message {
            Message::SearchChanged(text) => {
                self.search_text = text.clone();
                if text.len() >= 2 {
                    let lower = text.to_lowercase();
                    self.filtered_timezones = chrono_tz::TZ_VARIANTS
                        .iter()
                        .filter(|tz| {
                            let name = tz.name().to_lowercase();
                            name.contains(&lower)
                                || tz_city_name(**tz).to_lowercase().contains(&lower)
                        })
                        .take(20)
                        .map(|tz| (tz_city_name(*tz), *tz))
                        .collect();
                } else {
                    self.filtered_timezones.clear();
                }
            }
            Message::AddClock(tz) => {
                if !self.clocks.iter().any(|c| c.timezone == tz) {
                    self.clocks.push(ClockEntry {
                        id: self.next_id,
                        timezone: tz,
                        city_name: tz_city_name(tz),
                        is_local: false,
                    });
                    self.next_id += 1;
                }
                self.search_text.clear();
                self.filtered_timezones.clear();
            }
            Message::RemoveClock(id) => {
                self.clocks.retain(|c| c.id != id);
                if self.selected_clock_id == Some(id) {
                    self.selected_clock_id = None;
                }
                // If we were dragging, reset drag state since indices changed
                self.dragging_index = None;
            }
            Message::OpenAddSidebar => {
                // Handled in app.rs
            }
            Message::SelectClock(id) => {
                self.selected_clock_id = Some(id);
            }
            Message::DeselectClock => {
                self.selected_clock_id = None;
            }
            Message::ToggleEditMode => {
                self.edit_mode = !self.edit_mode;
                self.dragging_index = None;
                self.pre_drag_order.clear();
            }
            Message::StartDrag(index) => {
                // Save current order for cancel/revert
                self.pre_drag_order = self.clocks.iter().map(|c| c.id).collect();
                self.dragging_index = Some(index);
            }
            Message::Reorder(from, to) => {
                if from < self.clocks.len() && to < self.clocks.len() && from != to {
                    let clock = self.clocks.remove(from);
                    self.clocks.insert(to, clock);
                    self.dragging_index = Some(to);
                }
            }
            Message::FinishDrag => {
                self.dragging_index = None;
                self.pre_drag_order.clear();
            }
            Message::CancelDrag => {
                // Revert to pre-drag order
                if !self.pre_drag_order.is_empty() {
                    let id_order = &self.pre_drag_order;
                    let mut restored = Vec::with_capacity(id_order.len());
                    for &id in id_order {
                        if let Some(pos) = self.clocks.iter().position(|c| c.id == id) {
                            restored.push(self.clocks.remove(pos));
                        }
                    }
                    // Append any clocks added during drag (shouldn't happen, but be safe)
                    restored.append(&mut self.clocks);
                    self.clocks = restored;
                }
                self.dragging_index = None;
                self.pre_drag_order.clear();
            }
        }
    }
}
