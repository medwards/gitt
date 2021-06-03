use chrono::offset::TimeZone;
use std::str::FromStr;
use tui::style::Color;
use tui::text::Span;

mod controller;
mod model;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = app_args();
    let matches = app.get_matches_from(std::env::args_os());
    dbg!(&matches);
    let repository_dir = matches
        .value_of("working-directory")
        .map(|p| std::path::PathBuf::from_str(p).expect("Invalid path provided"))
        .unwrap_or_else(|| std::env::current_dir().expect("invoked from an invalid directory"));
    let repository = git2::Repository::discover(&repository_dir).expect(
        format!(
            "Unable to load repository at {}",
            &repository_dir.to_str().unwrap_or_else(|| "").to_owned()
        )
        .as_str(),
    );

    let mut app_model = model::AppModel::new(
        model::AppState::Commits,
        repository,
        matches.value_of("COMMITTISH").map(|s| s.to_string()),
    );

    let tick_rate = std::time::Duration::from_millis(200);
    let mut handler = controller::EventHandler::new(tick_rate);

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
            let commits_block = tui::widgets::Block::default();
            app_model.resize_revision_window(commits_block.inner(chunk_commit).height as usize);

            // TODO: something awkward happening with the borrow checker here
            // commits depends on the lifetime of app_model for some reason which means
            // there is an immutable borrow that conflices with the mutable borrow of
            // resize_revision_window
            let commits = app_model.commits();
            let commit_items: Vec<_> = commits.iter().map(commit_list_item).collect();
            let length = commits.len();
            drop(commits);
            app_model.resize_revision_window(length);

            let list = tui::widgets::List::new(commit_items)
                .block(commits_block)
                .highlight_style(
                    tui::style::Style::default().add_modifier(tui::style::Modifier::BOLD),
                );

            let details_block = tui::widgets::Paragraph::new(app_model.diff())
                .scroll((app_model.diff_line_scroll() as u16, 0))
                .block(tui::widgets::Block::default());

            let (list_state, _) = app_model.revision_window();
            rect.render_stateful_widget(list, chunk_commit, &mut list_state.clone());
            rect.render_widget(details_block, chunk_details)
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

fn commit_list_item(commit: &git2::Commit) -> tui::widgets::ListItem<'static> {
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
    tui::widgets::ListItem::new(tui::text::Spans::from(vec![
        Span::styled(time, tui::style::Style::default()),
        Span::raw(" "),
        Span::styled(title, tui::style::Style::default().fg(Color::White)),
        Span::raw(" "),
        Span::styled(author, tui::style::Style::default()),
    ]))
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
            clap::Arg::new("COMMITTISH")
                .index(1)
                .about("Git ref to view"),
        )
}
