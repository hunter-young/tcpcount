use ratatui::{
    buffer::Buffer,
    layout::{Rect, Layout, Direction, Constraint, Alignment},
    style::{Stylize, Style, Color},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Paragraph, Widget, Clear},
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

use crate::core::filters::ConnectionFilter;

#[derive(PartialEq)]
pub enum FilterField {
    Pid,
    ProcessName,
    RemoteHost,
    RemotePort,
}

impl FilterField {
    pub fn as_str(&self) -> &'static str {
        match self {
            FilterField::Pid => "PID",
            FilterField::ProcessName => "Process Name",
            FilterField::RemoteHost => "Remote Host",
            FilterField::RemotePort => "Remote Port",
        }
    }
    
    pub fn next(&self) -> Self {
        match self {
            FilterField::Pid => FilterField::ProcessName,
            FilterField::ProcessName => FilterField::RemoteHost,
            FilterField::RemoteHost => FilterField::RemotePort,
            FilterField::RemotePort => FilterField::Pid,
        }
    }
    
    pub fn prev(&self) -> Self {
        match self {
            FilterField::Pid => FilterField::RemotePort,
            FilterField::ProcessName => FilterField::Pid,
            FilterField::RemoteHost => FilterField::ProcessName,
            FilterField::RemotePort => FilterField::RemoteHost,
        }
    }
}

pub struct FilterWidget {
    current_field: FilterField,
    pid_input: String,
    process_name_input: String,
    remote_host_input: String,
    remote_port_input: String,
    active: bool,
    error: Option<String>,
}

impl FilterWidget {
    pub fn new() -> Self {
        Self {
            current_field: FilterField::Pid,
            pid_input: String::new(),
            process_name_input: String::new(),
            remote_host_input: String::new(),
            remote_port_input: String::new(),
            active: false,
            error: None,
        }
    }
    
    pub fn show(&mut self, current_filter: &ConnectionFilter) {
        self.active = true;
        self.error = None;
        
        if let Some(pid) = current_filter.pid {
            self.pid_input = pid.to_string();
        } else {
            self.pid_input = String::new();
        }
        
        if let Some(ref name) = current_filter.process_name {
            self.process_name_input = name.clone();
        } else {
            self.process_name_input = String::new();
        }
        
        if let Some(ref host) = current_filter.remote_host {
            self.remote_host_input = host.clone();
        } else {
            self.remote_host_input = String::new();
        }
        
        if let Some(port) = current_filter.remote_port {
            self.remote_port_input = port.to_string();
        } else {
            self.remote_port_input = String::new();
        }
        
        self.current_field = FilterField::Pid;
    }
    
    pub fn hide(&mut self) {
        self.active = false;
    }
    
    pub fn is_active(&self) -> bool {
        self.active
    }
    
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Option<ConnectionFilter> {
        if !self.active {
            return None;
        }
        
        if key_event.kind != KeyEventKind::Press {
            return None;
        }
        
        match key_event.code {
            KeyCode::Esc => {
                self.hide();
                None
            },
            KeyCode::Enter => {
                match self.build_filter() {
                    Ok(filter) => {
                        self.hide();
                        Some(filter)
                    }
                    Err(msg) => {
                        self.error = Some(msg);
                        None
                    }
                }
            },
            KeyCode::Tab => {
                self.current_field = self.current_field.next();
                None
            },
            KeyCode::BackTab => {
                self.current_field = self.current_field.prev();
                None
            },
            KeyCode::Char(c) => {
                match self.current_field {
                    FilterField::Pid => self.pid_input.push(c),
                    FilterField::ProcessName => self.process_name_input.push(c),
                    FilterField::RemoteHost => self.remote_host_input.push(c),
                    FilterField::RemotePort => self.remote_port_input.push(c),
                }
                None
            },
            KeyCode::Backspace => {
                match self.current_field {
                    FilterField::Pid => { self.pid_input.pop(); },
                    FilterField::ProcessName => { self.process_name_input.pop(); },
                    FilterField::RemoteHost => { self.remote_host_input.pop(); },
                    FilterField::RemotePort => { self.remote_port_input.pop(); },
                }
                None
            },
            _ => None,
        }
    }
    
    fn build_filter(&self) -> Result<ConnectionFilter, String> {
        let mut filter = ConnectionFilter::default();
        
        if !self.pid_input.is_empty() {
            match self.pid_input.parse::<u32>() {
                Ok(pid) => filter.pid = Some(pid),
                Err(_) => return Err(format!("Invalid PID: {}", self.pid_input)),
            }
        }
        
        if !self.process_name_input.is_empty() {
            filter.process_name = Some(self.process_name_input.clone());
        }
        
        if !self.remote_host_input.is_empty() {
            filter.remote_host = Some(self.remote_host_input.clone());
        }
        
        if !self.remote_port_input.is_empty() {
            match self.remote_port_input.parse::<u16>() {
                Ok(port) => filter.remote_port = Some(port),
                Err(_) => return Err(format!("Invalid port: {}", self.remote_port_input)),
            }
        }
        
        Ok(filter)
    }
    
    pub fn get_input_for_current_field(&self) -> &str {
        match self.current_field {
            FilterField::Pid => &self.pid_input,
            FilterField::ProcessName => &self.process_name_input,
            FilterField::RemoteHost => &self.remote_host_input,
            FilterField::RemotePort => &self.remote_port_input,
        }
    }
}

impl Widget for &FilterWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if !self.active {
            return;
        }
        
        let popup_width = area.width.min(60);
        let popup_height = 12;
        
        let hmargin = (area.width.saturating_sub(popup_width)) / 2;
        let vmargin = (area.height.saturating_sub(popup_height)) / 2;
        
        let popup_area = Rect {
            x: area.x + hmargin,
            y: area.y + vmargin,
            width: popup_width,
            height: popup_height,
        };
        
        Clear.render(popup_area, buf);
        
        let block = Block::bordered()
            .title("Filter Connections")
            .title_style(Style::new().bold().fg(Color::Yellow))
            .border_type(BorderType::Plain)
            .border_style(Style::new().fg(Color::Yellow));
            
        let inner_area = block.inner(popup_area);
        
        block.render(popup_area, buf);
        
        let field_layout = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(1),  // PID
                Constraint::Length(1),  // Process Name
                Constraint::Length(1),  // Remote Host
                Constraint::Length(1),  // Remote Port
                Constraint::Length(1),  // Empty space
                Constraint::Length(1),  // Instructions
                Constraint::Length(2),  // Error message (2 lines for wrapping)
            ])
            .split(inner_area);
        
        self.render_field(buf, field_layout[0], FilterField::Pid, &self.pid_input);
        self.render_field(buf, field_layout[1], FilterField::ProcessName, &self.process_name_input);
        self.render_field(buf, field_layout[2], FilterField::RemoteHost, &self.remote_host_input);
        self.render_field(buf, field_layout[3], FilterField::RemotePort, &self.remote_port_input);
        
        let instructions = Paragraph::new("Tab: Next field  |  Shift+Tab: Previous field  |  Enter: Apply  |  Esc: Cancel")
            .style(Style::new().fg(Color::Gray))
            .alignment(Alignment::Center);
        instructions.render(field_layout[5], buf);
        
        if let Some(ref error) = self.error {
            let error_msg = Paragraph::new(error.as_str())
                .style(Style::new().fg(Color::Red))
                .alignment(Alignment::Left);
            error_msg.render(field_layout[6], buf);
        }
    }
}

impl FilterWidget {
    fn render_field(&self, buf: &mut Buffer, area: Rect, field: FilterField, value: &str) {
        let is_active = self.current_field == field;
        
        let label_style = Style::new().fg(Color::White);
        let value_style = if is_active {
            Style::new().fg(Color::Yellow)
        } else {
            Style::new().fg(Color::Gray)
        };
        
        let value_text = if is_active {
            format!("{}_", value)
        } else {
            value.to_string()
        };
        
        let text = Text::from(vec![
            Line::from(vec![
                Span::styled(format!("{}: ", field.as_str()), label_style),
                Span::styled(value_text, value_style),
            ]),
        ]);
        
        let paragraph = Paragraph::new(text);
        paragraph.render(area, buf);
    }
}