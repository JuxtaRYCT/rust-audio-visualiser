use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{BarChart, Block, Borders},
    Terminal,
};
use rodio::{Decoder, OutputStream, Sink, Source};
use std::{
    fs::File,
    io::BufReader,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

const WINDOW_SIZE: usize = 100; // Number of bars to display on the graph

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Setup audio
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;
    let file = File::open("src/pink.mp3")?;
    let source = Decoder::new(BufReader::new(file))?;
    let sample_rate = source.sample_rate();
    let channels = source.channels();
    let samples: Arc<Vec<i16>> = Arc::new(source.collect());

    let audio_levels = Arc::new(Mutex::new(Vec::new()));
    let audio_levels_clone = Arc::clone(&audio_levels);
    let samples_clone = Arc::clone(&samples);

    // Spawn a thread for audio processing
    thread::spawn(move || {
        let chunk_size = sample_rate as usize / 20; // 50ms chunks
        for chunk in samples_clone.chunks(chunk_size) {
            let rms: f32 = (chunk
                .iter()
                .map(|&s| (s as f32 / i16::MAX as f32).powi(2))
                .sum::<f32>()
                / chunk.len() as f32)
                .sqrt();
            let db = 20.0 * rms.log10();
            let normalized_db = ((db + 60.0) / 60.0).clamp(0.0, 1.0);

            let mut levels = audio_levels_clone.lock().unwrap();
            levels.push(normalized_db);
            if levels.len() > WINDOW_SIZE {
                levels.remove(0);
            }

            thread::sleep(Duration::from_millis(50));
        }
    });

    sink.append(rodio::buffer::SamplesBuffer::new(
        channels,
        sample_rate,
        samples.to_vec(),
    ));
    sink.play();

    // Create a vector of static strings for labels
    let labels: Vec<String> = (0..WINDOW_SIZE).map(|i| i.to_string()).collect();

    // Main loop
    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([Constraint::Percentage(100)].as_ref())
                .split(f.size());

            let levels = audio_levels.lock().unwrap();
            let bar_data: Vec<(&str, u64)> = levels
                .iter()
                .enumerate()
                .map(|(i, &level)| (labels[i].as_str(), (level * 100.0) as u64))
                .collect();

            let barchart = BarChart::default()
                .block(
                    Block::default()
                        .title("Audio Visualization")
                        .borders(Borders::ALL),
                )
                .data(&bar_data)
                .bar_width(1)
                .bar_gap(0)
                .bar_style(Style::default().fg(Color::Yellow))
                .value_style(Style::default().fg(Color::Black).bg(Color::Yellow));

            f.render_widget(barchart, chunks[0]);
        })?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if let KeyCode::Char('q') = key.code {
                    break;
                }
            }
        }

        if sink.empty() {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
