use std::sync::{Arc, Mutex};
use ratatui::{
    buffer::Buffer,
    layout::{Rect, Constraint},
    style::{Stylize, Style, Color},
    widgets::{Block, Table, Row, Cell, Widget, BorderType},
};

use crate::core::monitor::ConnectionMonitor;
use crate::core::filters::ConnectionFilter;
use crate::app::SortBy;

pub struct ProcessTableWidget {
    monitor: Arc<Mutex<ConnectionMonitor>>,
    filter: ConnectionFilter,
    sort_by: SortBy,
    scroll_offset: usize,
}

impl ProcessTableWidget {
    pub fn new(monitor: Arc<Mutex<ConnectionMonitor>>) -> Self {
        Self {
            monitor,
            filter: ConnectionFilter::default(),
            sort_by: SortBy::Total,
            scroll_offset: 0,
        }
    }

    pub fn set_filter(&mut self, filter: ConnectionFilter) {
        self.filter = filter;
        self.scroll_offset = 0;
    }

    pub fn set_sort_by(&mut self, sort_by: SortBy) {
        self.sort_by = sort_by;
        self.scroll_offset = 0;
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: usize, total_rows: usize, visible_rows: usize) {
        let max_scroll = total_rows.saturating_sub(visible_rows);
        self.scroll_offset = (self.scroll_offset + amount).min(max_scroll);
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn scroll_to_bottom(&mut self, total_rows: usize, visible_rows: usize) {
        let max_scroll = total_rows.saturating_sub(visible_rows);
        self.scroll_offset = max_scroll;
    }
}

impl Widget for &ProcessTableWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let monitor_guard = match self.monitor.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        let mut process_metrics = monitor_guard.get_process_metrics(&self.filter);
        
        match self.sort_by {
            SortBy::Total => {
                process_metrics.sort_by(|a, b| b.total_connections.cmp(&a.total_connections)
                    .then_with(|| a.pid.cmp(&b.pid)));
            },
            SortBy::Active => {
                process_metrics.sort_by(|a, b| b.current_connections.cmp(&a.current_connections)
                    .then_with(|| a.pid.cmp(&b.pid)));
            }, 
            SortBy::Max => {
                process_metrics.sort_by(|a, b| b.max_concurrent.cmp(&a.max_concurrent)
                    .then_with(|| a.pid.cmp(&b.pid)));
            }
        }

        let content_height = area.height.saturating_sub(3);
        let visible_rows = content_height as usize;
        let total_rows = process_metrics.len();
        
        let start_idx = self.scroll_offset;
        let end_idx = (start_idx + visible_rows).min(total_rows);
        let visible_metrics = &process_metrics[start_idx..end_idx];
        
        let rows: Vec<Row> = visible_metrics.iter().map(|metrics| {
            let pid_style = if metrics.is_alive {
                Style::new().fg(Color::Green)
            } else {
                Style::new().fg(Color::Red)
            };
            
            Row::new(vec![
                Cell::from(metrics.pid.to_string()).style(pid_style),
                Cell::from(metrics.name.clone()),
                Cell::from(metrics.current_connections.to_string()),
                Cell::from(metrics.total_connections.to_string()),
                Cell::from(metrics.max_concurrent.to_string()),
            ])
        }).collect();
        
        let widths = [
            Constraint::Percentage(10),  // PID
            Constraint::Percentage(60),  // Name
            Constraint::Percentage(10),  // Current Connections
            Constraint::Percentage(10),  // Total Connections
            Constraint::Percentage(10),  // Max Connections
        ];
        
        let table = Table::new(rows, widths)
            .header(
                Row::new(vec![
                    "PID",
                    "Process Name",
                    "Active",
                    "Total",
                    "Max",
                ])
                .style(Style::new().bold().fg(Color::White))
                .bottom_margin(1)
            )
            .block(
                Block::bordered()
                    .title("Connections by Process")
                    .title_style(Style::new().bold().fg(Color::Cyan))
                    .border_type(BorderType::Plain)
                    .border_style(Style::new().fg(Color::Blue))
            );
        
        table.render(area, buf);
    }
}