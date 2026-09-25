use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::Block;

pub const BG: Color = Color::Rgb(20, 20, 20);
pub const BORDER: Color = Color::Rgb(48, 54, 61);
pub const TITLE: Color = Color::Rgb(88, 196, 214);
pub const MAGENTA: Color = Color::Rgb(157, 124, 216);
pub const GREEN: Color = Color::Rgb(152, 195, 121);
pub const YELLOW: Color = Color::Rgb(229, 192, 123);
pub const CURRENT: Color = Color::Rgb(42, 46, 56);
pub const DIM: Color = Color::Rgb(106, 114, 128);
pub const FG: Color = Color::Rgb(220, 223, 228);
pub const CYAN: Color = Color::Rgb(86, 182, 194);
pub const RED: Color = Color::Rgb(224, 108, 117);

pub fn base() -> Style {
    Style::new().fg(FG).bg(BG)
}

pub fn title() -> Style {
    Style::new().fg(TITLE)
}

pub fn border() -> Style {
    Style::new().fg(BORDER)
}

pub fn dim() -> Style {
    Style::new().fg(DIM).bg(BG)
}

pub fn green() -> Style {
    Style::new().fg(GREEN).bg(BG)
}

pub fn magenta() -> Style {
    Style::new().fg(MAGENTA).bg(BG)
}

pub fn cyan() -> Style {
    Style::new().fg(CYAN).bg(BG)
}

pub fn yellow() -> Style {
    Style::new().fg(YELLOW).bg(BG)
}

pub fn pane(heading: &str) -> Block<'static> {
    Block::bordered()
        .border_style(border())
        .style(base())
        .title(Span::styled(format!(" {heading} "), title()))
}
