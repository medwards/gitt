use chrono::offset::TimeZone;
use std::str::FromStr;

mod controller;
mod model;
mod widgets;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = app_args();
    let matches = app.get_matches_from(std::env::args_os());
    if matches.is_present("verbose") {
        dbg!(&matches);
    }
    let repository_dir = matches
        .value_of("working-directory")
        // If present, use working-directory
        .map(|p| std::path::PathBuf::from_str(p).map_err(|_| "infallible".to_string()))
        // otherwise use the current dir
        .or_else(|| Some(std::env::current_dir().map_err(|e| format!("{}", e))))
        .expect("Missing value AND default for working-directory")?;
    let repository = git2::Repository::discover(&repository_dir)?;

    let filters: Vec<_> = matches
        .values_of("path")
        .map(|paths| {
            paths
                .map(|path| model::CommitFilter::Path(path.to_string()))
                .collect()
        })
        .unwrap_or_else(|| Vec::new());

    let mut app_model = model::AppModel::new(
        model::AppState::Commits,
        repository,
        matches.value_of("COMMITTISH").map(|s| s.to_string()),
        filters,
    )?;

    let tick_rate = std::time::Duration::from_millis(200);
    let mut handler = controller::EventHandler::new(tick_rate);

    let bounds: Vec<_> = (0..6).map(|_| cassowary::Variable::new()).collect();
    let window_width = cassowary::Variable::new();
    let mut column_solver = widgets::commit_list_column_width_solver(&bounds, &window_width);

    // TODO: use RAII for this somehow
    crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen)?;
    crossterm::terminal::enable_raw_mode().expect("can run in raw mode");
    let stdout = std::io::stdout();
    let backend = tui::backend::CrosstermBackend::new(stdout);
    let mut terminal = tui::Terminal::new(backend)?;
    terminal.clear()?;

    loop {
        terminal.draw(|rect| {
            let size = rect.size();
            let chunks = tui::layout::Layout::default()
                .direction(tui::layout::Direction::Vertical)
                .margin(2)
                .constraints(
                    [
                        tui::layout::Constraint::Percentage(20),
                        tui::layout::Constraint::Length(1),
                        tui::layout::Constraint::Min(2),
                    ]
                    .as_ref(),
                )
                .split(size);

            let chunk_commit = chunks[0];
            let chunk_details = chunks[2];
            let chunk_details = tui::layout::Layout::default()
                .direction(tui::layout::Direction::Horizontal)
                .constraints(
                    [
                        tui::layout::Constraint::Min(10),
                        tui::layout::Constraint::Length(1),
                    ]
                    .as_ref(),
                )
                .split(chunk_details);
            let chunk_details_pane = chunk_details[0];
            let chunk_details_scroll = chunk_details[1];
            let commits_block = tui::widgets::Block::default();
            let details_block = tui::widgets::Block::default();

            app_model.resize_revision_window(commits_block.inner(chunk_commit).height as usize);
            let commit_items: Vec<_> = app_model.commits().iter().map(commit_list_item).collect();

            app_model.resize_diff_window(details_block.inner(chunk_details_pane).height as usize);

            // TODO: https://github.com/fdehau/tui-rs/issues/499
            column_solver
                .suggest_value(window_width, chunk_commit.width as f64)
                .expect("constraints solver failed");
            let column_widths = widgets::solver_changes_to_lengths(&column_solver, &bounds);

            let list = tui::widgets::Table::new(commit_items)
                .block(commits_block)
                .highlight_style(
                    tui::style::Style::default().add_modifier(tui::style::Modifier::BOLD),
                )
                .widths(column_widths.as_slice());

            let (details_index, details_window, details_length) = app_model.diff_line_scroll();
            let details_scroll = widgets::VerticalBar {
                window_index: details_index,
                window_length: details_window,
                total_length: details_length,
                style: tui::style::Style::default().bg(
                    if app_model.app_state == model::AppState::Details {
                        tui::style::Color::Gray
                    } else {
                        tui::style::Color::Black
                    },
                ),
            };
            let details_block = tui::widgets::Paragraph::new(app_model.diff())
                .scroll((details_index as u16, 0))
                .block(details_block);

            let (list_state, _) = app_model.revision_window();
            rect.render_stateful_widget(list, chunk_commit, &mut list_state.clone());
            rect.render_widget(details_block, chunk_details_pane);
            rect.render_widget(details_scroll, chunk_details_scroll);
        })?;

        if handler.update_model(&mut app_model).is_err()
            || app_model.app_state == model::AppState::Finished
        {
            crossterm::terminal::disable_raw_mode()?;
            terminal.show_cursor()?;
            crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen)?;
            break;
        }
    }
    Ok(())
}

fn commit_list_item(commit: &git2::Commit) -> tui::widgets::Row<'static> {
    let time = format_time(&commit.time());
    // TODO: If this needs to be length limited include unicode_segmentation
    let title = commit
        .message()
        .unwrap_or_else(|| "INVALID UTF8 IN COMMIT MESSAGE")
        .split("\n")
        .nth(0)
        .expect("message body was bad")
        .to_owned();
    let author = commit.author().to_string();
    tui::widgets::Row::new(vec![title, author, time])
}

fn format_time(time: &git2::Time) -> String {
    let tz = chrono::FixedOffset::east_opt(time.offset_minutes() * 60)
        .expect("timezone offset was too big");
    let dt = tz.timestamp(time.seconds(), 0);
    dt.to_rfc3339()
}

fn app_args() -> clap::App<'static> {
    clap::App::new("gitt")
        .about("Git repository viewer in your terminal")
        .arg(
            clap::Arg::new("working-directory")
                .long("working-directory")
                .value_name("PATH")
                .about("Use PATH as the working directory of gitt"),
        )
        .arg(
            clap::Arg::new("verbose")
                .long("verbose")
                .required(false)
                .takes_value(false)
                .about("Emit processing messages"),
        )
        .arg(clap::Arg::new("COMMITTISH").about("Git ref to view"))
        .arg(
            clap::Arg::new("path")
                .multiple(true)
                .last(true)
                .about("Limit commits to the ones touching files in the given paths"),
        )
}
