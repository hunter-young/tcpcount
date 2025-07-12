use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, MouseEvent, MouseEventKind};
use crossterm::{execute, event::EnableMouseCapture, event::DisableMouseCapture};
use ratatui::{DefaultTerminal, Frame};

use crate::core::monitor::ConnectionMonitor;
use crate::core::filters::ConnectionFilter;
use crate::widgets::{
    HostTableWidget, 
    ProcessHostTableWidget,
    ProcessTableWidget,
    SummaryWidget,
    ActiveConnectionsGraphWidget,
    FilterWidget
};

use ratatui::layout::{Layout, Direction, Constraint};
use ratatui::widgets::Paragraph;
use ratatui::style::{Style, Color};
use ratatui::text::{Span, Line};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortBy {
    Total,
    Active,
    Max,
}

impl SortBy {
    pub fn as_str(&self) -> &'static str {
        match self {
            SortBy::Total => "Total",
            SortBy::Active => "Active",
            SortBy::Max => "Max",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FocusedTable {
    ProcessHost,
    Process,
    Host,
}

pub struct App {
    pub host_table_widget: HostTableWidget,
    pub process_host_table_widget: ProcessHostTableWidget,
    pub process_table_widget: ProcessTableWidget,
    pub summary_widget: SummaryWidget,
    pub active_connections_graph_widget: ActiveConnectionsGraphWidget,
    pub filter_widget: FilterWidget,
    pub monitor: Arc<Mutex<ConnectionMonitor>>,
    pub current_filter: ConnectionFilter,
    pub exit: bool,
    pub last_tick: Instant,
    pub tick_rate: Duration,
    pub mouse_enabled: bool,
    pub focused_table: FocusedTable,
}

impl App {
    pub fn new() -> Self {
        let monitor = Arc::new(Mutex::new(ConnectionMonitor::new()));
        let current_filter = ConnectionFilter::default();
        
        App {
            host_table_widget: HostTableWidget::new(Arc::clone(&monitor)),
            process_host_table_widget: ProcessHostTableWidget::new(Arc::clone(&monitor)),
            process_table_widget: ProcessTableWidget::new(Arc::clone(&monitor)),
            summary_widget: SummaryWidget::new(Arc::clone(&monitor)),
            active_connections_graph_widget: ActiveConnectionsGraphWidget::new(Arc::clone(&monitor))
                .with_max_points(300),
            filter_widget: FilterWidget::new(),
            monitor,
            current_filter,
            exit: false,
            last_tick: Instant::now(),
            tick_rate: Duration::from_millis(250),
            mouse_enabled: false,
            focused_table: FocusedTable::ProcessHost,
        }
    }
    
    pub fn with_filter(mut self, filter: ConnectionFilter) -> Self {
        self.current_filter = filter.clone();
        self.apply_filter(filter);
        self
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        if let Ok(()) = execute!(
            std::io::stdout(),
            EnableMouseCapture
        ) {
            self.mouse_enabled = true;
        }

        let result = self.run_loop(terminal);

        if self.mouse_enabled {
            let _ = execute!(
                std::io::stdout(),
                DisableMouseCapture
            );
        }

        result
    }

    fn run_loop(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            let timeout = self.tick_rate
                .checked_sub(self.last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));
            
            if crossterm::event::poll(timeout)? {
                self.handle_events()?;
            }
            
            if self.last_tick.elapsed() >= self.tick_rate {
                self.tick();
                self.last_tick = Instant::now();
            }
            
            terminal.draw(|frame| self.draw(frame))?;
        }
        Ok(())
    }

    fn tick(&mut self) {
        self.update_monitor();
        self.active_connections_graph_widget.update();
    }

    fn update_monitor(&mut self) {
        if let Ok(mut monitor) = self.monitor.lock() {
            monitor.refresh().ok();
        }
    }

    fn reset_monitor(&mut self) {
        if let Ok(mut monitor) = self.monitor.lock() {
            monitor.reset();
        }
    }

    fn draw(&self, frame: &mut Frame) {
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(7),   // First row: Graph + Summary
                Constraint::Percentage(38), // Second row: Process-Host Table
                Constraint::Percentage(38), // Third row: Host Table + Process Table
                Constraint::Length(1),   // Fourth row: Status bar
            ])
            .margin(1)
            .split(frame.area());
            
        let top_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(75), // Graph (75% of width)
                Constraint::Percentage(25), // Summary count (25% of width)
            ])
            .split(main_chunks[0]);
            
        let bottom_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50), // Host Table
                Constraint::Percentage(50), // Process Table
            ])
            .split(main_chunks[2]);
        
        frame.render_widget(&self.active_connections_graph_widget, top_chunks[0]);
        frame.render_widget(&self.summary_widget, top_chunks[1]);
        
        frame.render_widget(&self.process_host_table_widget, main_chunks[1]);
        
        frame.render_widget(&self.host_table_widget, bottom_chunks[0]);
        frame.render_widget(&self.process_table_widget, bottom_chunks[1]);
        
        let mut status_text = Vec::new();
        
        let filter_str = if self.current_filter.is_empty() {
            "No filters active".to_string()
        } else {
            format!("Filter: {}", self.current_filter.to_string())
        };
        
        status_text.push(Span::styled(filter_str, Style::default().fg(Color::Yellow)));
        
        // Add spacer
        status_text.push(Span::raw(" | "));

        // Show focused table
        let focused_table_str = match self.focused_table {
            FocusedTable::ProcessHost => "Focus: Process-Host",
            FocusedTable::Process => "Focus: Process",
            FocusedTable::Host => "Focus: Host",
        };
        status_text.push(Span::styled(focused_table_str, Style::default().fg(Color::Cyan)));
        status_text.push(Span::raw(" | "));
        
        // Add key bindings
        status_text.push(Span::styled("1-3", Style::default().fg(Color::Green)));
        status_text.push(Span::raw(": Switch Table "));

        status_text.push(Span::styled("↑↓", Style::default().fg(Color::Green)));
        status_text.push(Span::raw(": Scroll "));

        status_text.push(Span::styled("f", Style::default().fg(Color::Green)));
        status_text.push(Span::raw(": Filter "));
        
        status_text.push(Span::styled("c", Style::default().fg(Color::Green)));
        status_text.push(Span::raw(": Clear "));
        
        status_text.push(Span::styled("r", Style::default().fg(Color::Green)));
        status_text.push(Span::raw(": Reset "));

        status_text.push(Span::styled("t/a/m", Style::default().fg(Color::Green)));
        status_text.push(Span::raw(": Sort "));
        
        status_text.push(Span::styled("q", Style::default().fg(Color::Green)));
        status_text.push(Span::raw(": Quit"));
        
        let status_bar = Paragraph::new(Line::from(status_text));
        frame.render_widget(status_bar, main_chunks[3]);
        
        if self.filter_widget.is_active() {
            frame.render_widget(&self.filter_widget, frame.area());
        }
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            Event::Mouse(mouse_event) => {
                self.handle_mouse_event(mouse_event)
            }
            _ => {}
        };
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if self.filter_widget.is_active() {
            if let Some(new_filter) = self.filter_widget.handle_key_event(key_event) {
                self.apply_filter(new_filter);
            }
            return;
        }
        
        match key_event.code {
            KeyCode::Char('q') => self.exit(),
            KeyCode::Char('r') => self.reset_monitor(),
            KeyCode::Char('c') => self.clear_all_filters(),
            KeyCode::Char('f') => self.enter_filter_mode(),
            KeyCode::Char('t') => self.set_sort_by(SortBy::Total),
            KeyCode::Char('a') => self.set_sort_by(SortBy::Active),
            KeyCode::Char('m') => self.set_sort_by(SortBy::Max),
            KeyCode::Char('1') => self.focused_table = FocusedTable::ProcessHost,
            KeyCode::Char('2') => self.focused_table = FocusedTable::Host,
            KeyCode::Char('3') => self.focused_table = FocusedTable::Process,
            KeyCode::Up => self.scroll_focused_table_up(1),
            KeyCode::Down => self.scroll_focused_table_down(1),
            KeyCode::PageUp => self.scroll_focused_table_up(10),
            KeyCode::PageDown => self.scroll_focused_table_down(10),
            KeyCode::Home => self.scroll_focused_table_to_top(),
            KeyCode::End => self.scroll_focused_table_to_bottom(),
            _ => {}
        }
    }

    fn handle_mouse_event(&mut self, mouse_event: MouseEvent) {
        if !self.mouse_enabled {
            return;
        }

        match mouse_event.kind {
            MouseEventKind::ScrollUp => {
                self.scroll_focused_table_up(3);
            }
            MouseEventKind::ScrollDown => {
                self.scroll_focused_table_down(3);
            }
            _ => {}
        }
    }

    fn scroll_focused_table_up(&mut self, amount: usize) {
        match self.focused_table {
            FocusedTable::ProcessHost => self.process_host_table_widget.scroll_up(amount),
            FocusedTable::Process => self.process_table_widget.scroll_up(amount),
            FocusedTable::Host => self.host_table_widget.scroll_up(amount),
        }
    }

    fn scroll_focused_table_down(&mut self, amount: usize) {
        match self.focused_table {
            FocusedTable::ProcessHost => {
                if let Ok(monitor) = self.monitor.lock() {
                    let metrics = monitor.get_process_host_metrics(&self.current_filter);
                    let total_rows = metrics.len();
                    let visible_rows = 15; // Approximate
                    self.process_host_table_widget.scroll_down(amount, total_rows, visible_rows);
                }
            }
            FocusedTable::Process => {
                if let Ok(monitor) = self.monitor.lock() {
                    let metrics = monitor.get_process_metrics(&self.current_filter);
                    let total_rows = metrics.len();
                    let visible_rows = 15; // Approximate
                    self.process_table_widget.scroll_down(amount, total_rows, visible_rows);
                }
            }
            FocusedTable::Host => {
                if let Ok(monitor) = self.monitor.lock() {
                    let metrics = monitor.get_host_metrics(&self.current_filter);
                    let total_rows = metrics.len();
                    let visible_rows = 15; // Approximate
                    self.host_table_widget.scroll_down(amount, total_rows, visible_rows);
                }
            }
        }
    }

    fn scroll_focused_table_to_top(&mut self) {
        match self.focused_table {
            FocusedTable::ProcessHost => self.process_host_table_widget.scroll_to_top(),
            FocusedTable::Process => self.process_table_widget.scroll_to_top(),
            FocusedTable::Host => self.host_table_widget.scroll_to_top(),
        }
    }

    fn scroll_focused_table_to_bottom(&mut self) {
        match self.focused_table {
            FocusedTable::ProcessHost => {
                if let Ok(monitor) = self.monitor.lock() {
                    let metrics = monitor.get_process_host_metrics(&self.current_filter);
                    let total_rows = metrics.len();
                    let visible_rows = 15; // Approximate
                    self.process_host_table_widget.scroll_to_bottom(total_rows, visible_rows);
                }
            }
            FocusedTable::Process => {
                if let Ok(monitor) = self.monitor.lock() {
                    let metrics = monitor.get_process_metrics(&self.current_filter);
                    let total_rows = metrics.len();
                    let visible_rows = 15; // Approximate
                    self.process_table_widget.scroll_to_bottom(total_rows, visible_rows);
                }
            }
            FocusedTable::Host => {
                if let Ok(monitor) = self.monitor.lock() {
                    let metrics = monitor.get_host_metrics(&self.current_filter);
                    let total_rows = metrics.len();
                    let visible_rows = 15; // Approximate
                    self.host_table_widget.scroll_to_bottom(total_rows, visible_rows);
                }
            }
        }
    }
    
    fn clear_all_filters(&mut self) {
        let filter = ConnectionFilter::default();
        self.current_filter = filter.clone();
        self.apply_filter(filter);
    }
    
    fn enter_filter_mode(&mut self) {
        self.filter_widget.show(&self.current_filter);
    }
    
    fn apply_filter(&mut self, filter: ConnectionFilter) {
        self.current_filter = filter.clone();
        
        self.host_table_widget.set_filter(filter.clone());
        self.process_host_table_widget.set_filter(filter.clone());
        self.process_table_widget.set_filter(filter.clone());
        self.summary_widget.set_filter(filter.clone());
        self.active_connections_graph_widget.set_filter(filter);
    }

    fn set_sort_by(&mut self, sort_by: SortBy) {
        self.host_table_widget.set_sort_by(sort_by);
        self.process_host_table_widget.set_sort_by(sort_by);
        self.process_table_widget.set_sort_by(sort_by);
    }

    fn exit(&mut self) {
        self.exit = true
    }
}