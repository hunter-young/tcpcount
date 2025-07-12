use std::sync::{Arc, Mutex};
use std::time::{SystemTime, Duration};
use std::cmp;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Stylize, Style, Color},
    widgets::{Block, Widget, Sparkline, BorderType},
    text::Span,
    symbols,
};

use crate::core::monitor::ConnectionMonitor;
use crate::core::filters::ConnectionFilter;

pub struct ActiveConnectionsGraphWidget {
    monitor: Arc<Mutex<ConnectionMonitor>>,
    filter: ConnectionFilter,
    max_points: usize,
    history_data: Vec<u64>,
    last_sample_time: SystemTime,
    sample_interval: Duration,
    last_filter_hash: u64, // To detect filter changes
}

impl ActiveConnectionsGraphWidget {
    pub fn new(monitor: Arc<Mutex<ConnectionMonitor>>) -> Self {
        let filter = ConnectionFilter::default();
        let filter_hash = Self::hash_filter(&filter);
        
        Self {
            monitor,
            filter,
            max_points: 100, // Default to 100 data points
            history_data: Vec::new(),
            last_sample_time: SystemTime::now(),
            sample_interval: Duration::from_secs(1), // 1 second per bar
            last_filter_hash: filter_hash,
        }
    }

    fn hash_filter(filter: &ConnectionFilter) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        
        if let Some(pid) = filter.pid {
            pid.hash(&mut hasher);
        }
        
        if let Some(ref name) = filter.process_name {
            name.hash(&mut hasher);
        }
        
        if let Some(ref host) = filter.remote_host {
            host.hash(&mut hasher);
        }
        
        if let Some(port) = filter.remote_port {
            port.hash(&mut hasher);
        }
        
        hasher.finish()
    }

    pub fn set_filter(&mut self, filter: ConnectionFilter) {
        self.filter = filter;
        self.last_filter_hash = Self::hash_filter(&self.filter);
        
        self.rebuild_history_data();
    }
    
    pub fn with_max_points(mut self, points: usize) -> Self {
        self.max_points = points;
        self
    }
    
    fn rebuild_history_data(&mut self) {
        if let Ok(monitor_guard) = self.monitor.lock() {
            let history = monitor_guard.get_connection_history_filtered(
                &self.filter,
                None,
                None  // No end time limit
            );
            
            self.history_data = history.iter()
                .map(|(_, count)| *count as u64)
                .collect();
            
            if self.history_data.len() > self.max_points {
                let skip = self.history_data.len() - self.max_points;
                self.history_data = self.history_data.iter().skip(skip).cloned().collect();
            }
        }
    }

    pub fn update(&mut self) {
        let now = SystemTime::now();
        
        let current_hash = Self::hash_filter(&self.filter);
        if current_hash != self.last_filter_hash {
            self.last_filter_hash = current_hash;
            self.rebuild_history_data();
            return;
        }
        
        if let Ok(elapsed) = now.duration_since(self.last_sample_time) {
            if elapsed >= self.sample_interval {
                if let Ok(monitor_guard) = self.monitor.lock() {
                    let active_connections = monitor_guard.get_filtered_active_connections(&self.filter).len() as u64;
                    
                    self.history_data.push(active_connections);
                    
                    if self.history_data.len() > self.max_points {
                        self.history_data.remove(0);
                    }
                    
                    self.last_sample_time = now;
                }
            }
        }
    }
    
    /// Find the maximum value in the history data
    fn get_max_value(&self) -> u64 {
        self.history_data.iter().fold(0, |max, &val| cmp::max(max, val))
    }
}

impl Widget for &ActiveConnectionsGraphWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.history_data.is_empty() {
            let block = Block::bordered()
                .title("Active Connections (1s interval)")
                .title_style(Style::new().bold().fg(Color::Cyan))
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(Color::Blue));
            
            block.render(area, buf);
            return;
        }
        
        let max_value = self.get_max_value();
        let max_value_rounded = if max_value == 0 { 
            1
        } else {
            let magnitude = (max_value as f64).log10().floor() as u32;
            let base = 10u64.pow(magnitude);
            ((max_value as f64 / base as f64).ceil() as u64) * base
        };
        
        let block = Block::bordered()
            .title("Active Connections (1s interval)")
            .title_style(Style::new().bold().fg(Color::Cyan))
            .border_type(BorderType::Plain)
            .border_style(Style::new().fg(Color::Blue));
        
        let inner_area = block.inner(area);
        block.render(area, buf);
        
        if inner_area.width < 1 || inner_area.height < 1 {
            return;
        }
        
        if inner_area.height > 2 {
            let scale_area = Rect {
                x: inner_area.x,
                y: inner_area.y,
                width: 6,
                height: inner_area.height,
            };
            
            let max_marker = Span::styled(
                format!("{:4}", max_value_rounded),
                Style::default().fg(Color::Gray)
            );
            buf.set_span(scale_area.x, scale_area.y, &max_marker, 4);
            
            if scale_area.height > 1 {
                let min_marker = Span::styled(
                    format!("{:4}", 0),
                    Style::default().fg(Color::Gray)
                );
                buf.set_span(scale_area.x, scale_area.bottom() - 1, &min_marker, 4);
            }
        }
        
        let sparkline_area = Rect {
            x: inner_area.x + 6,
            y: inner_area.y,
            width: inner_area.width.saturating_sub(6),
            height: inner_area.height,
        };
        
        let available_points = sparkline_area.width as usize;
        let data_slice = if self.history_data.len() <= available_points {
            let mut padded = vec![0; available_points - self.history_data.len()];
            padded.extend(&self.history_data);
            padded
        } else {
            self.history_data.iter()
                .skip(self.history_data.len() - available_points)
                .cloned()
                .collect()
        };
        
        let sparkline = Sparkline::default()
            .data(&data_slice)
            .max(max_value_rounded)
            .style(Style::default().fg(Color::Cyan))
            .bar_set(symbols::bar::NINE_LEVELS);
            
        sparkline.render(sparkline_area, buf);
    }
}