use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::ui::theme;
use crate::view::ViewModel;

/// One breakpoint as label/value rows: ID, Location, Condition.
pub fn render(frame: &mut Frame, area: Rect, model: &ViewModel) {
    let block = theme::pane("Breakpoints");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let mut row = 0u16;
    if model.breakpoints.is_empty() {
        return;
    }
    for (index, bp) in model.breakpoints.iter().enumerate() {
        if index > 0 {
            row = row.saturating_add(1);
        }
        let id = bp.id.map(|id| id.to_string()).unwrap_or_else(|| "-".into());
        let location_style = if bp.verified {
            theme::base()
        } else {
            theme::dim()
        };
        for (label, value, style) in [
            ("ID", id, theme::magenta()),
            ("Location", bp.location.clone(), location_style),
            ("Condition", bp.condition.clone(), theme::green()),
        ] {
            if row >= inner.height {
                return;
            }
            paint_pair(frame, inner, row, label, &value, style);
            row = row.saturating_add(1);
        }
    }
}

fn paint_pair(
    frame: &mut Frame,
    inner: Rect,
    row: u16,
    label: &str,
    value: &str,
    value_style: ratatui::style::Style,
) {
    let label_width = (inner.width / 2).max(1);
    let value_width = inner.width.saturating_sub(label_width);
    Line::from(Span::styled(label, theme::dim())).render(
        Rect::new(inner.x, inner.y + row, label_width, 1),
        frame.buffer_mut(),
    );
    let shown: String = value.chars().take(value_width as usize).collect();
    Line::from(Span::styled(shown, value_style)).render(
        Rect::new(inner.x + label_width, inner.y + row, value_width, 1),
        frame.buffer_mut(),
    );
}
