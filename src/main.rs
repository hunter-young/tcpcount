mod app;
mod core;
mod widgets;
mod cli;

use app::App;
use cli::parse_args;

use ratatui;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let initial_filter = parse_args();
    
    let mut terminal = ratatui::init();
    
    let app_result = App::new()
        .with_filter(initial_filter)
        .run(&mut terminal);
    
    ratatui::restore();
    
    app_result?;
    
    Ok(())
}