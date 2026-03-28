// SPDX-License-Identifier: MIT
//
// A reusable drag-to-reorder list widget, modelled on the `AppletReorderList`
// in cosmic-settings (cosmic-settings/src/pages/desktop/panel/applets_inner.rs).
//
// The widget wraps a pre-built inner `Element` (typically a column of card rows)
// and intercepts mouse / Wayland-DnD events to implement press → drag → drop
// reordering with position-based insertion zones.
//
// # Usage
//
// ```rust,ignore
// ReorderList::new(inner_column, item_count, dragging_index)
//     .on_start_drag(|idx| Msg::StartDrag(idx))
//     .on_reorder(|from, to| Msg::Reorder(from, to))
//     .on_finish(Msg::FinishDrag)
//     .on_cancel(Msg::CancelDrag)
// ```

use std::borrow::Cow;
use std::mem;

use cosmic::iced::clipboard::dnd::{DndAction, DndDestinationRectangle, DndEvent, OfferEvent, SourceEvent};
use cosmic::iced::clipboard::mime::AsMimeTypes;
use cosmic::iced::id::Internal;
use cosmic::iced::{mouse, overlay, touch, Length, Point, Rectangle, Size, Vector};
use cosmic::iced_core::clipboard::IconSurface;
use cosmic::iced_core::widget::{tree, Operation, Tree};
use cosmic::iced_core::{self, layout, renderer, Clipboard, Shell};
use cosmic::iced_runtime::core::id::Id;
use cosmic::prelude::*;
use cosmic::{theme, widget};

const MIME_TYPE: &str = "application/x-reorder-index";
const DRAG_START_DISTANCE_SQUARED: f32 = 64.0;

/// Data transferred during a DnD operation — just the source index as bytes.
#[derive(Debug, Clone)]
struct DndIndex(usize);

impl AsMimeTypes for DndIndex {
    fn available(&self) -> Cow<'static, [String]> {
        Cow::Owned(vec![MIME_TYPE.to_string()])
    }

    fn as_bytes(&self, mime_type: &str) -> Option<Cow<'static, [u8]>> {
        if mime_type == MIME_TYPE {
            Some(Cow::Owned(self.0.to_string().into_bytes()))
        } else {
            None
        }
    }
}

/// A drag-to-reorder list widget.
///
/// Wraps a pre-built inner element (column of cards) and handles drag events
/// to reorder the items.  The caller is responsible for:
/// - building card elements (collapsing the dragged card to a placeholder),
/// - tracking `dragging_index` in external state,
/// - applying reorder operations when `on_reorder(from, to)` fires.
pub struct ReorderList<'a, Message> {
    id: Id,
    item_count: usize,
    inner: Element<'a, Message>,
    dragging_index: Option<usize>,
    on_start_drag: Option<Box<dyn Fn(usize) -> Message + 'a>>,
    on_reorder: Option<Box<dyn Fn(usize, usize) -> Message + 'a>>,
    on_finish: Option<Message>,
    on_cancel: Option<Message>,
    drag_icon_builder: Option<Box<dyn Fn(usize, Vector) -> (Element<'static, ()>, tree::State, Vector) + 'a>>,
}

impl<'a, Message: Clone + 'static> ReorderList<'a, Message> {
    /// Create a new reorder list.
    ///
    /// - `inner`: The pre-built visual content (e.g. a `Column` of card rows).
    /// - `item_count`: Number of items in the list.
    /// - `dragging_index`: Which item is currently being dragged (`None` if idle).
    pub fn new(
        inner: impl Into<Element<'a, Message>>,
        item_count: usize,
        dragging_index: Option<usize>,
    ) -> Self {
        Self {
            id: Id::unique(),
            item_count,
            inner: inner.into(),
            dragging_index,
            on_start_drag: None,
            on_reorder: None,
            on_finish: None,
            on_cancel: None,
            drag_icon_builder: None,
        }
    }

    /// Callback when a drag starts. Receives the index of the item being dragged.
    #[must_use]
    pub fn on_start_drag(mut self, f: impl Fn(usize) -> Message + 'a) -> Self {
        self.on_start_drag = Some(Box::new(f));
        self
    }

    /// Callback during drag motion. Receives `(from, to)` indices for the reorder.
    #[must_use]
    pub fn on_reorder(mut self, f: impl Fn(usize, usize) -> Message + 'a) -> Self {
        self.on_reorder = Some(Box::new(f));
        self
    }

    /// Message emitted when the drag completes successfully.
    #[must_use]
    pub fn on_finish(mut self, msg: Message) -> Self {
        self.on_finish = Some(msg);
        self
    }

    /// Message emitted when the drag is cancelled.
    #[must_use]
    pub fn on_cancel(mut self, msg: Message) -> Self {
        self.on_cancel = Some(msg);
        self
    }

    /// Optional builder for the floating DnD icon.
    /// Receives the dragged item index and the cursor offset, returns
    /// `(element, tree_state, offset)` for the icon surface.
    #[must_use]
    pub fn drag_icon(
        mut self,
        f: impl Fn(usize, Vector) -> (Element<'static, ()>, tree::State, Vector) + 'a,
    ) -> Self {
        self.drag_icon_builder = Some(Box::new(f));
        self
    }

    /// Returns the drag id used for DnD destination matching.
    fn get_drag_id(&self) -> u128 {
        match &self.id.0 {
            Internal::Unique(id) | Internal::Custom(id, _) => *id as u128,
            Internal::Set(_) => panic!("Invalid Id assigned to ReorderList."),
        }
    }

    /// Given a cursor y-position, compute which insertion slot it falls into.
    /// Returns an index in `0..=item_count`.
    fn insertion_index(&self, layout: &layout::Layout, y: f32) -> usize {
        if self.item_count == 0 {
            return 0;
        }
        let spacing = theme::spacing().space_xxs as f32;
        let total_spacing = spacing * (self.item_count.saturating_sub(1)) as f32;
        let item_height =
            (layout.bounds().height - total_spacing) / self.item_count as f32;

        let mut threshold = layout.bounds().y;
        for i in 0..self.item_count {
            threshold += if i == 0 {
                item_height / 2.0
            } else {
                item_height + spacing
            };
            if y <= threshold {
                return i;
            }
        }
        self.item_count.saturating_sub(1)
    }

    /// Given `start_y` (where the press occurred), determine which item index
    /// was pressed.
    fn pressed_item_index(&self, layout: &layout::Layout, start_y: f32) -> Option<usize> {
        if self.item_count == 0 {
            return None;
        }
        let spacing = theme::spacing().space_xxs as f32;
        let total_spacing = spacing * (self.item_count.saturating_sub(1)) as f32;
        let item_height =
            (layout.bounds().height - total_spacing) / self.item_count as f32;

        let relative = start_y - layout.bounds().y;
        if relative < 0.0 {
            return None;
        }
        for i in 0..self.item_count {
            let top = i as f32 * (item_height + spacing);
            if relative < top + item_height {
                return Some(i);
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Widget implementation
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone)]
enum DraggingState {
    #[default]
    None,
    Pressed(Point),
    Dragging {
        index: usize,
    },
}

#[derive(Debug, Default, Clone)]
enum DndOfferState {
    #[default]
    None,
    HandlingOffer,
}

#[derive(Debug, Default, Clone)]
struct ReorderWidgetState {
    dragging_state: DraggingState,
    dnd_offer: DndOfferState,
    cached_size: Option<Size>,
}

impl<Message: Clone + 'static> cosmic::iced_core::Widget<Message, cosmic::Theme, cosmic::Renderer>
    for ReorderList<'_, Message>
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<ReorderWidgetState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(ReorderWidgetState::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.inner)]
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(&mut [&mut self.inner]);
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Shrink)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &cosmic::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let inner_layout =
            self.inner
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, limits);
        let state = tree.state.downcast_mut::<ReorderWidgetState>();
        state.cached_size = Some(inner_layout.size());
        layout::Node::with_children(inner_layout.size(), vec![inner_layout])
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: layout::Layout<'_>,
        renderer: &cosmic::Renderer,
        operation: &mut dyn Operation<()>,
    ) {
        operation.container(Some(&self.id), layout.bounds());
        self.inner.as_widget_mut().operate(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            operation,
        );
    }

    #[allow(clippy::too_many_lines)]
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &cosmic::iced::Event,
        layout: layout::Layout<'_>,
        cursor_position: mouse::Cursor,
        renderer: &cosmic::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        // Forward to inner element first (so delete buttons etc. still work).
        self.inner.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout.children().next().unwrap(),
            cursor_position,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        if shell.is_event_captured() {
            return;
        }

        let state = tree.state.downcast_mut::<ReorderWidgetState>();

        // --- Drag source state machine (press → threshold → dragging) ---
        state.dragging_state = match mem::take(&mut state.dragging_state) {
            DraggingState::None => match &event {
                cosmic::iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                | cosmic::iced::Event::Touch(touch::Event::FingerPressed { .. })
                    if cursor_position.is_over(layout.bounds()) =>
                {
                    shell.capture_event();
                    DraggingState::Pressed(cursor_position.position().unwrap_or_default())
                }
                _ => DraggingState::None,
            },

            DraggingState::Pressed(start) => match &event {
                cosmic::iced::Event::Mouse(mouse::Event::CursorMoved { .. })
                | cosmic::iced::Event::Touch(touch::Event::FingerMoved { .. }) => {
                    let pos = cursor_position.position().unwrap_or_default();
                    let dx = pos.x - start.x;
                    let dy = pos.y - start.y;
                    if dx * dx + dy * dy > DRAG_START_DISTANCE_SQUARED {
                        if let Some(index) = self.pressed_item_index(&layout, start.y) {
                            // Notify caller
                            if let Some(ref on_start) = self.on_start_drag {
                                shell.publish(on_start(index));
                            }

                            // Start Wayland DnD
                            let bounds = state.cached_size.map_or(layout.bounds(), |s| {
                                Rectangle::new(layout.bounds().position(), s)
                            });
                            // Compute offset relative to the dragged item's top-left,
                            // not the whole list widget.
                            let spacing = theme::spacing().space_xxs as f32;
                            let total_spacing =
                                spacing * self.item_count.saturating_sub(1) as f32;
                            let item_height = (layout.bounds().height - total_spacing)
                                / self.item_count as f32;
                            let item_top = layout.bounds().y
                                + index as f32 * (item_height + spacing);
                            let offset = Vector::new(
                                start.x - layout.bounds().x,
                                start.y - item_top,
                            );
                            const DRAG_SCALE: f32 = 0.92;
                            let icon_surface = self.drag_icon_builder.as_ref().map(|builder| {
                                let scaled_width = bounds.width * DRAG_SCALE;
                                // Scale the offset so the cursor stays at the same
                                // relative position on the shrunken card.
                                let scaled_offset = Vector::new(
                                    offset.x * DRAG_SCALE,
                                    offset.y * DRAG_SCALE,
                                );
                                let (icon_el, icon_state, _) = builder(index, scaled_offset);
                                IconSurface::new(
                                    widget::container(icon_el)
                                        .width(Length::Fixed(scaled_width))
                                        .into(),
                                    icon_state,
                                    scaled_offset,
                                )
                            });
                            iced_core::clipboard::start_dnd::<
                                cosmic::Theme,
                                cosmic::Renderer,
                            >(
                                clipboard,
                                false,
                                Some(iced_core::clipboard::DndSource::Widget(self.id.clone())),
                                icon_surface,
                                Box::new(DndIndex(index)),
                                DndAction::Move,
                            );

                            shell.capture_event();
                            DraggingState::Dragging { index }
                        } else {
                            DraggingState::Pressed(start)
                        }
                    } else {
                        DraggingState::Pressed(start)
                    }
                }
                cosmic::iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                | cosmic::iced::Event::Touch(
                    touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. },
                ) => {
                    shell.capture_event();
                    DraggingState::None
                }
                _ => DraggingState::Pressed(start),
            },

            DraggingState::Dragging { index } => match &event {
                cosmic::iced::Event::Dnd(DndEvent::Source(SourceEvent::Cancelled)) => {
                    if let Some(ref on_cancel) = self.on_cancel {
                        shell.publish(on_cancel.clone());
                    }
                    shell.capture_event();
                    DraggingState::None
                }
                cosmic::iced::Event::Dnd(DndEvent::Source(SourceEvent::Finished)) => {
                    shell.capture_event();
                    DraggingState::None
                }
                _ => DraggingState::Dragging { index },
            },
        };

        // --- DnD destination state machine (offer events) ---
        let my_drag_id = self.get_drag_id();
        state.dnd_offer = match mem::take(&mut state.dnd_offer) {
            DndOfferState::None => match &event {
                cosmic::iced::Event::Dnd(DndEvent::Offer(
                    rectangle,
                    OfferEvent::Enter { x, y, .. },
                )) if *rectangle == Some(my_drag_id) => {
                    self.handle_drag_motion(shell, &layout, *y as f32);
                    DndOfferState::HandlingOffer
                }
                _ => DndOfferState::None,
            },

            DndOfferState::HandlingOffer => match &event {
                cosmic::iced::Event::Dnd(DndEvent::Offer(
                    rectangle,
                    OfferEvent::Motion { y, .. },
                )) if *rectangle == Some(my_drag_id) => {
                    self.handle_drag_motion(shell, &layout, *y as f32);
                    DndOfferState::HandlingOffer
                }
                cosmic::iced::Event::Dnd(DndEvent::Offer(
                    rectangle,
                    OfferEvent::LeaveDestination | OfferEvent::Leave,
                )) if *rectangle == Some(my_drag_id) => {
                    DndOfferState::None
                }
                cosmic::iced::Event::Dnd(DndEvent::Offer(
                    rectangle,
                    OfferEvent::Data { .. },
                )) if *rectangle == Some(my_drag_id) => {
                    if let Some(ref on_finish) = self.on_finish {
                        shell.publish(on_finish.clone());
                    }
                    DndOfferState::None
                }
                _ => DndOfferState::HandlingOffer,
            },
        };
    }

    fn draw(
        &self,
        state: &Tree,
        renderer: &mut cosmic::Renderer,
        theme: &cosmic::Theme,
        style: &renderer::Style,
        layout: layout::Layout<'_>,
        cursor_position: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.inner.as_widget().draw(
            &state.children[0],
            renderer,
            theme,
            style,
            layout.children().next().unwrap(),
            cursor_position,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: layout::Layout<'b>,
        renderer: &cosmic::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, cosmic::Theme, cosmic::Renderer>> {
        self.inner.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }

    fn mouse_interaction(
        &self,
        state: &Tree,
        layout: layout::Layout<'_>,
        cursor_position: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &cosmic::Renderer,
    ) -> mouse::Interaction {
        let inner_interaction = self.inner.as_widget().mouse_interaction(
            &state.children[0],
            layout.children().next().unwrap(),
            cursor_position,
            viewport,
            renderer,
        );
        match inner_interaction {
            mouse::Interaction::Idle => {
                let state = state.state.downcast_ref::<ReorderWidgetState>();
                if matches!(state.dragging_state, DraggingState::Dragging { .. }) {
                    mouse::Interaction::Grabbing
                } else if cursor_position.is_over(layout.bounds()) {
                    mouse::Interaction::Grab
                } else {
                    mouse::Interaction::default()
                }
            }
            other => other,
        }
    }

    fn id(&self) -> Option<Id> {
        Some(self.id.clone())
    }

    fn set_id(&mut self, id: Id) {
        self.id = id;
    }

    fn drag_destinations(
        &self,
        _state: &Tree,
        layout: layout::Layout<'_>,
        _renderer: &cosmic::Renderer,
        dnd_rectangles: &mut iced_core::clipboard::DndDestinationRectangles,
    ) {
        let bounds = layout.bounds();
        dnd_rectangles.push(DndDestinationRectangle {
            id: self.get_drag_id(),
            rectangle: cosmic::iced::clipboard::dnd::Rectangle {
                x: bounds.x as f64,
                y: bounds.y as f64,
                width: bounds.width as f64,
                height: bounds.height as f64,
            },
            mime_types: vec![Cow::Owned(MIME_TYPE.to_string())],
            actions: DndAction::Move,
            preferred: DndAction::Move,
        });
    }
}

impl<'a, Message: Clone + 'static> ReorderList<'a, Message> {
    /// Handle a drag motion event: compute the target insertion position and
    /// emit `on_reorder(from, to)` if the dragging index is known.
    fn handle_drag_motion(
        &self,
        shell: &mut Shell<'_, Message>,
        layout: &layout::Layout,
        y: f32,
    ) {
        if let (Some(from), Some(on_reorder)) = (self.dragging_index, &self.on_reorder) {
            let to = self.insertion_index(layout, y);
            if from != to {
                shell.publish(on_reorder(from, to));
            }
        }
    }
}

impl<'a, Message: Clone + 'static> From<ReorderList<'a, Message>> for Element<'a, Message> {
    fn from(list: ReorderList<'a, Message>) -> Self {
        Element::new(list)
    }
}
