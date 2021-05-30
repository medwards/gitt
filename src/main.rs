use chrono::offset::TimeZone;
use std::str::FromStr;
use tui::style::Color;
use tui::style::Style;
use tui::text::Span;
use tui::text::Spans;

mod model;

enum Event<I> {
    Input(I),
    Tick,
}

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

    let mut app_model = model::AppModel::new(repository, None);

    // ui stuff
    let (tx, rx) = std::sync::mpsc::channel();
    let tick_rate = std::time::Duration::from_millis(200);
    std::thread::spawn(move || {
        let mut last_tick = std::time::Instant::now();
        loop {
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| std::time::Duration::from_secs(0));

            if crossterm::event::poll(timeout).expect("poll works") {
                if let crossterm::event::Event::Key(key) =
                    crossterm::event::read().expect("can read events")
                {
                    tx.send(Event::Input(key)).expect("can send events");
                }
            }

            if last_tick.elapsed() >= tick_rate {
                if let Ok(_) = tx.send(Event::Tick) {
                    last_tick = std::time::Instant::now();
                }
            }
        }
    });

    crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen)?;
    crossterm::terminal::enable_raw_mode().expect("can run in raw mode");
    let stdout = std::io::stdout();
    let backend = tui::backend::CrosstermBackend::new(stdout);
    let mut terminal = tui::Terminal::new(backend)?;
    terminal.clear()?;

    let mut commit_list_state = tui::widgets::ListState::default();
    commit_list_state.select(Some(0));

    loop {
        let mut commit_list_height = 0;
        terminal.draw(|rect| {
            let size = rect.size();
            let chunks = tui::layout::Layout::default()
                .direction(tui::layout::Direction::Vertical)
                .margin(2)
                .constraints(
                    [
                        tui::layout::Constraint::Length(7),
                        tui::layout::Constraint::Length(1),
                        tui::layout::Constraint::Min(2),
                    ]
                    .as_ref(),
                )
                .split(size);

            let chunk_commit = chunks[0];
            let chunk_details = chunks[2];
            let commits_block = tui::widgets::Block::default();
            commit_list_height = commits_block.inner(chunk_commit).height as usize;

            let commits = app_model.commits(commit_list_height);
            let commit_items: Vec<_> = commits.iter().map(commit_list_item).collect();

            let list = tui::widgets::List::new(commit_items)
                .block(commits_block)
                .highlight_style(
                    tui::style::Style::default().add_modifier(tui::style::Modifier::BOLD),
                );

            let details_block = commit_details(
                app_model.repository(),
                &commits
                    .iter()
                    .nth(commit_list_state.selected().unwrap_or(0))
                    .expect("Could not find selected commit"),
            )
            .block(tui::widgets::Block::default());

            rect.render_stateful_widget(list, chunk_commit, &mut commit_list_state);
            rect.render_widget(details_block, chunk_details)
        })?;

        // TODO: what if someone commits while this is blocking?
        match rx.recv()? {
            Event::Input(event) => match event.code {
                crossterm::event::KeyCode::Char('q') => {
                    crossterm::terminal::disable_raw_mode()?;
                    terminal.show_cursor()?;
                    crossterm::execute!(
                        std::io::stdout(),
                        crossterm::terminal::LeaveAlternateScreen
                    )?;
                    break;
                }
                crossterm::event::KeyCode::Char('g') => {
                    commit_list_state.select(Some(0));
                    app_model.go_to_first();
                }
                crossterm::event::KeyCode::Char('G') => {
                    commit_list_state.select(Some(commit_list_height - 1));
                    app_model.go_to_last();
                    (0..commit_list_height)
                        .into_iter()
                        .for_each(|_| app_model.decrement());
                }
                crossterm::event::KeyCode::Down | crossterm::event::KeyCode::Char('j') => {
                    if app_model.remaining(commit_list_height) == 0 {
                        continue;
                    }
                    match commit_list_state.selected() {
                        Some(index) => commit_list_state.select(Some(index + 1)),
                        None => commit_list_state.select(Some(0)),
                    };
                    if commit_list_state.selected().unwrap_or(0) >= commit_list_height {
                        commit_list_state.select(Some(commit_list_height - 1));
                        app_model.increment();
                    }
                }
                crossterm::event::KeyCode::Up | crossterm::event::KeyCode::Char('k') => {
                    if commit_list_state.selected().unwrap_or(commit_list_height) == 0 {
                        app_model.decrement();
                    }
                    match commit_list_state.selected() {
                        Some(index) => commit_list_state.select(Some(index.saturating_sub(1))),
                        None => {
                            commit_list_state.select(Some(commit_list_height.saturating_sub(1)))
                        }
                    };
                }
                _ => {}
            },
            Event::Tick => {}
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

fn commit_details(
    repo: &git2::Repository,
    commit: &git2::Commit,
) -> tui::widgets::Paragraph<'static> {
    let mut text = vec![Spans::from(vec![Span::raw(commit.id().to_string())])];
    text.append(
        &mut commit
            .message()
            .unwrap_or_else(|| "INVALID MESSAGE")
            .split("\n")
            .map(|s| s.to_string())
            .map(|s| Spans::from(vec![Span::raw(s)]))
            .collect(),
    );

    if commit.parents().len() <= 1 {
        let parent_tree = commit.parent(0).ok().map(|p| p.tree().ok()).flatten();
        let diff = repo
            .diff_tree_to_tree(parent_tree.as_ref(), commit.tree().ok().as_ref(), None)
            .expect("Unable to create diff");
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            let (origin, style) = match line.origin() {
                'F' => {
                    text.append(
                        &mut std::str::from_utf8(line.content())
                            .unwrap()
                            .split("\n")
                            .map(|s| s.to_string())
                            .map(|s| {
                                Spans::from(vec![Span::styled(s, Style::default().fg(Color::Gray))])
                            })
                            .collect(),
                    );
                    return true;
                }
                'H' => (None, Style::default().fg(Color::Cyan)),
                ' ' => (None, Style::default()),
                '+' => (Some(line.origin()), Style::default().fg(Color::Green)),
                '-' => (Some(line.origin()), Style::default().fg(Color::Red)),
                _ => (None, Style::default()),
            };

            let spans = vec![
                Span::styled(origin.unwrap_or(' ').to_string(), style),
                Span::styled(
                    std::str::from_utf8(line.content()).unwrap().to_string(),
                    style,
                ),
            ];
            text.push(Spans::from(spans));
            true
        })
        .expect("Unable to format diff");
    }
    tui::widgets::Paragraph::new(text)
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
}
