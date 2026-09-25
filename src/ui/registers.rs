use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::ui::theme;
use crate::view::ViewModel;

pub fn render(frame: &mut Frame, area: Rect, model: &ViewModel) {
    let block = theme::pane("Register");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    if model.registers_unavailable {
        Line::from(Span::styled("(unavailable)", theme::dim())).render(inner, frame.buffer_mut());
        return;
    }

    let value_width = (inner.width / 2).max(8).min(inner.width);
    let name_width = inner.width.saturating_sub(value_width);
    for (index, register) in model.registers.iter().enumerate() {
        if index as u16 >= inner.height {
            break;
        }
        let y = inner.y + index as u16;
        Line::from(Span::styled(
            fit(&register.name, name_width as usize),
            theme::base(),
        ))
        .render(Rect::new(inner.x, y, name_width, 1), frame.buffer_mut());
        Paragraph::new(fit(&register.value, value_width as usize))
            .style(theme::yellow())
            .alignment(Alignment::Right)
            .render(
                Rect::new(inner.x + name_width, y, value_width, 1),
                frame.buffer_mut(),
            );
    }
}

fn fit(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        text.to_string()
    } else {
        text.chars().take(width).collect()
    }
}
