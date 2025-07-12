use std::sync::{Arc, Mutex};
use ratatui::{
    buffer::Buffer,
    layout::{Rect, Alignment},
    style::{Stylize, Style, Color},
    text::{Line, Span, Text},
    widgets::{Block, Paragraph, Widget, BorderType},
};

use crate::core::monitor::ConnectionMonitor;
use crate::core::filters::ConnectionFilter;

pub struct SummaryWidget {
    monitor: Arc<Mutex<ConnectionMonitor>>,
    filter: ConnectionFilter,
}

impl SummaryWidget {
    pub fn new(monitor: Arc<Mutex<ConnectionMonitor>>) -> Self {
        Self {
            monitor,
            filter: ConnectionFilter::default(),
        }
    }

    pub fn set_filter(&mut self, filter: ConnectionFilter) {
        self.filter = filter;
    }
}

impl Widget for &SummaryWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let monitor_guard = match self.monitor.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        let current_connections = monitor_guard.get_filtered_active_connections(&self.filter).len();
        
        let historical_connections = monitor_guard.get_filtered_historical_connections(&self.filter).len();
        let total_opened = historical_connections + current_connections;
        
        let history = monitor_guard.get_connection_history_filtered(&self.filter, None, None);
        let max_concurrent = history.iter().map(|(_, count)| *count).max().unwrap_or(0);
        
        let text = Text::from(vec![
            Line::from(vec![
                Span::raw("Active: "),
                Span::styled(
                    format!("{}", current_connections), 
                    Style::default().fg(Color::Green).bold()
                ),
            ]),
            Line::from(vec![
                Span::raw("Total: "),
                Span::styled(
                    format!("{}", total_opened),
                    Style::default().fg(Color::Green).bold()
                ),
            ]),
            Line::from(vec![
                Span::raw("Max: "),
                Span::styled(
                    format!("{}", max_concurrent),
                    Style::default().fg(Color::Green).bold()
                ),
            ]),
        ]);
        
        let paragraph = Paragraph::new(text)
            .block(
                Block::bordered()
                    .title("Overall connections")
                    .title_style(Style::new().bold().fg(Color::Cyan))
                    .border_type(BorderType::Plain)
                    .border_style(Style::new().fg(Color::Blue))
            )
            .alignment(Alignment::Left);
            
        paragraph.render(area, buf);
    }
}