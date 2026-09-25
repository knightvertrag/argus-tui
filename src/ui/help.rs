use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::text::Span;
use ratatui::widgets::{Block, Clear, Paragraph, Widget, Wrap};

use crate::ui::theme;

const HELP: &str = "\
View
  c continue    n next    s step in    f finish
  b toggle breakpoint on the current line
  1 Code  2 Vars  3 Stack  4 Threads  5 Memory  6 Terminal
  j/k or arrows scroll     Enter or : command line
  q, Esc, Ctrl-C quit

Prompt
  continue, next, step, finish, quit, help
  break <file>:<line> [if <condition>]
  delete <id>
  watch <expression>
  unwatch <index>
  thread <id>
  frame <index>
  Esc closes the prompt

Esc closes this help.";

pub fn render(frame: &mut Frame, area: Rect) {
    let width = area.width.min(64);
    let height = area.height.clamp(8, 22);
    if width < 8 || area.height < 8 {
        return;
    }
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let rect = Rect::new(x, y, width, height);
    frame.render_widget(Clear, rect);
    let block = Block::bordered()
        .border_style(theme::magenta())
        .style(theme::base())
        .title(Span::styled(" Help ", theme::title()))
        .title_alignment(Alignment::Center);
    let paragraph = Paragraph::new(HELP)
        .block(block)
        .style(theme::base())
        .wrap(Wrap { trim: false });
    paragraph.render(rect, frame.buffer_mut());
}
