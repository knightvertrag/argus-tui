mod breakpoints;
mod code;
mod header;
mod help;
mod layout;
mod memory;
mod prompt;
mod registers;
mod stack;
mod tabs;
mod terminal_pane;
mod theme;
mod threads;
mod variables;
mod watch;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::Frame;
use ratatui::style::Style;
use ratatui::widgets::Block;
use tokio::sync::mpsc;

use crate::command::{self, Command};
use crate::view::ViewModel;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Code,
    Vars,
    Stack,
    Threads,
    Memory,
    Terminal,
}

impl Tab {
    fn from_digit(ch: char) -> Option<Self> {
        Some(match ch {
            '1' => Self::Code,
            '2' => Self::Vars,
            '3' => Self::Stack,
            '4' => Self::Threads,
            '5' => Self::Memory,
            '6' => Self::Terminal,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus {
    #[default]
    View,
    Prompt,
}

#[derive(Debug)]
pub struct UiState {
    pub tab: Tab,
    pub focus: Focus,
    pub prompt: String,
    pub cursor: usize,
    pub help: bool,
    pub quitting: bool,
    pub message: Option<String>,
    pub code_scroll: usize,
    pub code_stick: bool,
    pub code_view: usize,
    pub code_len: usize,
    pub list_scroll: usize,
    pub list_stick: bool,
    pub list_view: usize,
    pub list_len: usize,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            tab: Tab::Code,
            focus: Focus::View,
            prompt: String::new(),
            cursor: 0,
            help: false,
            quitting: false,
            message: None,
            code_scroll: 0,
            code_stick: false,
            code_view: 1,
            code_len: 0,
            list_scroll: 0,
            list_stick: false,
            list_view: 1,
            list_len: 0,
        }
    }
}

pub struct ListWindow<'a> {
    pub scroll: &'a mut usize,
    pub stick: &'a mut bool,
    pub view: &'a mut usize,
    pub len: &'a mut usize,
    pub stick_to_end: bool,
}

impl ListWindow<'_> {
    pub fn visible_start(&mut self, len: usize, view: usize) -> usize {
        let view = view.max(1);
        *self.len = len;
        *self.view = view;
        if !*self.stick {
            *self.scroll = if self.stick_to_end {
                len.saturating_sub(view)
            } else {
                0
            };
        }
        let max = len.saturating_sub(view);
        *self.scroll = (*self.scroll).min(max);
        *self.scroll
    }
}

pub fn draw(frame: &mut Frame, model: &ViewModel, ui: &mut UiState) {
    frame.render_widget(Block::new().style(Style::new().bg(theme::BG)), frame.area());
    let chrome = layout::chrome(frame.area());
    header::render(frame, chrome.header, model);
    match ui.tab {
        Tab::Code => {
            let panes = layout::code_view(chrome.body);
            breakpoints::render(frame, panes.breakpoints, model);
            watch::render(frame, panes.watch, model);
            stack::render(frame, panes.stack, model, None);
            code::render(frame, panes.code, model, ui);
            variables::render(frame, panes.variables, model, None);
            registers::render(frame, panes.registers, model);
        }
        Tab::Vars => {
            let mut window = list_window(ui, false);
            variables::render(frame, chrome.body, model, Some(&mut window));
        }
        Tab::Stack => {
            let mut window = list_window(ui, false);
            stack::render(frame, chrome.body, model, Some(&mut window));
        }
        Tab::Threads => {
            let mut window = list_window(ui, false);
            threads::render(frame, chrome.body, model, &mut window);
        }
        Tab::Memory => memory::render(frame, chrome.body),
        Tab::Terminal => {
            let mut window = list_window(ui, true);
            terminal_pane::render(frame, chrome.body, model, &mut window);
        }
    }
    tabs::render(frame, chrome.tabs, ui.tab);
    prompt::render(frame, chrome.prompt, model, ui);
    if ui.help {
        help::render(frame, frame.area());
    }
}

fn list_window(ui: &mut UiState, stick_to_end: bool) -> ListWindow<'_> {
    ListWindow {
        scroll: &mut ui.list_scroll,
        stick: &mut ui.list_stick,
        view: &mut ui.list_view,
        len: &mut ui.list_len,
        stick_to_end,
    }
}

pub fn handle_key(key: KeyEvent, ui: &mut UiState, cmds: &mpsc::UnboundedSender<Command>) {
    if key.kind == KeyEventKind::Release || ui.quitting {
        return;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        quit(ui, cmds);
        return;
    }
    if ui.help {
        if matches!(key.code, KeyCode::Esc | KeyCode::Char('?')) {
            ui.help = false;
        } else if matches!(key.code, KeyCode::Char('q')) {
            quit(ui, cmds);
        }
        return;
    }
    if ui.focus == Focus::Prompt {
        handle_prompt(key, ui, cmds);
        return;
    }
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => quit(ui, cmds),
        KeyCode::Char('?') => ui.help = true,
        KeyCode::Char('c') => send(ui, cmds, Command::Resume),
        KeyCode::Char('n') => send(ui, cmds, Command::StepOver),
        KeyCode::Char('s') => send(ui, cmds, Command::StepIn),
        KeyCode::Char('f') => send(ui, cmds, Command::StepOut),
        KeyCode::Char('b') => send(ui, cmds, Command::ToggleBreakpoint),
        KeyCode::Char('j') | KeyCode::Down => scroll(ui, 1),
        KeyCode::Char('k') | KeyCode::Up => scroll(ui, -1),
        KeyCode::Char(ch @ '1'..='6') => {
            let Some(tab) = Tab::from_digit(ch) else {
                return;
            };
            if ui.tab != tab {
                ui.list_scroll = 0;
                ui.list_stick = false;
                ui.tab = tab;
            }
        }
        KeyCode::Enter | KeyCode::Char(':') => {
            ui.focus = Focus::Prompt;
            ui.message = None;
        }
        _ => {}
    }
}

fn handle_prompt(key: KeyEvent, ui: &mut UiState, cmds: &mpsc::UnboundedSender<Command>) {
    match key.code {
        KeyCode::Esc => {
            ui.focus = Focus::View;
            prompt::clear(ui);
        }
        KeyCode::Enter => submit(ui, cmds),
        KeyCode::Backspace => prompt::backspace(ui),
        KeyCode::Left => prompt::move_left(ui),
        KeyCode::Right => prompt::move_right(ui),
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => prompt::clear(ui),
        KeyCode::Char(ch) => prompt::insert(ui, ch),
        _ => {}
    }
}

fn submit(ui: &mut UiState, cmds: &mpsc::UnboundedSender<Command>) {
    match command::parse(&ui.prompt) {
        Ok(None) => prompt::clear(ui),
        Ok(Some(Command::Help)) => {
            ui.help = true;
            prompt::clear(ui);
        }
        Ok(Some(Command::Quit)) => quit(ui, cmds),
        Ok(Some(command)) => {
            ui.message = None;
            let _ = cmds.send(command);
            prompt::clear(ui);
        }
        Err(err) => ui.message = Some(err),
    }
}

fn send(ui: &mut UiState, cmds: &mpsc::UnboundedSender<Command>, command: Command) {
    ui.message = None;
    let _ = cmds.send(command);
}

fn quit(ui: &mut UiState, cmds: &mpsc::UnboundedSender<Command>) {
    if ui.quitting {
        return;
    }
    ui.quitting = true;
    let _ = cmds.send(Command::Quit);
}

fn scroll(ui: &mut UiState, delta: isize) {
    if ui.tab == Tab::Code {
        ui.code_stick = true;
        ui.code_scroll = nudge(ui.code_scroll, delta, ui.code_len, ui.code_view);
    } else {
        ui.list_stick = true;
        ui.list_scroll = nudge(ui.list_scroll, delta, ui.list_len, ui.list_view);
    }
}

fn nudge(current: usize, delta: isize, len: usize, view: usize) -> usize {
    let max = len.saturating_sub(view.max(1));
    if delta < 0 {
        current.saturating_sub(delta.unsigned_abs())
    } else {
        current.saturating_add(delta as usize).min(max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debugger::Phase;
    use crate::view::{BreakpointRow, LocalRow, RegisterRow, ViewModel, Watch};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    fn fixture() -> ViewModel {
        let mut source_lines = vec![String::new(); 11];
        source_lines[0] = "#include <stdio.h>".into();
        source_lines[6] = "for(int i = 0; i < 10; i++) {".into();
        ViewModel {
            phase: Phase::Stopped,
            exit_code: None,
            function: Some("main".into()),
            line: Some(7),
            column: Some(13),
            has_source: true,
            file_name: Some("main.c".into()),
            source_lines,
            locals: vec![LocalRow {
                head: "int i = ".into(),
                value: "10".into(),
            }],
            source_error: None,
            breakpoint_lines: vec![8],
            watches: vec![Watch {
                expression: "i".into(),
                value: "10".into(),
            }],
            breakpoints: vec![BreakpointRow {
                id: Some(1),
                location: "main.c:8".into(),
                condition: "i == 10".into(),
                verified: true,
            }],
            registers: vec![RegisterRow {
                name: "rax".into(),
                value: "0x0a".into(),
            }],
            registers_unavailable: false,
            threads: Vec::new(),
            frames: vec![crate::view::FrameRow {
                label: "#0 main at main.c:8".into(),
                focused: true,
            }],
            terminal: Vec::new(),
            status: String::new(),
        }
    }

    fn buffer_text(buffer: &Buffer) -> String {
        let mut out = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    /// Foreground of the first cell of `needle`, matched per cell so multi-byte
    /// symbols are not treated as byte offsets.
    fn fg_of(buffer: &Buffer, needle: &str) -> Option<ratatui::style::Color> {
        let chars: Vec<String> = needle.chars().map(|ch| ch.to_string()).collect();
        if chars.is_empty() {
            return None;
        }
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                let fits = chars.iter().enumerate().all(|(index, symbol)| {
                    let column = x.saturating_add(index as u16);
                    column < buffer.area.width && buffer[(column, y)].symbol() == symbol
                });
                if fits {
                    return buffer[(x, y)].style().fg;
                }
            }
        }
        None
    }

    #[test]
    fn code_view_matches_the_prototype_panes() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let model = fixture();
        let mut ui = UiState::default();
        terminal.draw(|frame| draw(frame, &model, &mut ui)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        let text = buffer_text(&buffer);
        assert!(text.contains("Code"), "{text}");
        assert!(text.contains("Watch"), "{text}");
        assert!(text.contains("Breakpoints"), "{text}");
        assert!(text.contains("Variables"), "{text}");
        assert!(text.contains("Register"), "{text}");
        assert!(text.contains("Call Stack"), "{text}");
        assert!(text.contains("main.c"), "{text}");
        assert!(text.contains("main.c:8"), "{text}");
        assert!(text.contains("0x0a"), "{text}");
        assert!(text.contains('→'), "{text}");
        assert!(text.contains("fn main"), "{text}");
        assert_eq!(fg_of(&buffer, "→"), Some(theme::MAGENTA));
        assert_eq!(fg_of(&buffer, "0x0a"), Some(theme::YELLOW));

        let tab_top = buffer.area.height.saturating_sub(6);
        let tab_bottom = buffer.area.height.saturating_sub(3);
        let mut magenta_tab = false;
        for y in tab_top..tab_bottom {
            for x in 0..buffer.area.width {
                if buffer[(x, y)].style().fg == Some(theme::MAGENTA) {
                    magenta_tab = true;
                }
            }
        }
        assert!(magenta_tab, "active tab border should be magenta");
    }
}
