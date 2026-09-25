use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::ui::ListWindow;
use crate::ui::theme;
use crate::view::ViewModel;

pub fn render(frame: &mut Frame, area: Rect, model: &ViewModel, window: Option<&mut ListWindow>) {
    let block = theme::pane("Call Stack");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let start = match window {
        Some(window) => window.visible_start(model.frames.len(), inner.height as usize),
        None => 0,
    };
    for (row, frame_row) in model.frames.iter().enumerate().skip(start) {
        let visible = row - start;
        if visible >= inner.height as usize {
            break;
        }
        let style = if frame_row.focused {
            theme::magenta()
        } else {
            theme::base()
        };
        Line::from(Span::styled(frame_row.label.as_str(), style)).render(
            Rect::new(inner.x, inner.y + visible as u16, inner.width, 1),
            frame.buffer_mut(),
        );
    }
}
