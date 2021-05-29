use chrono::offset::TimeZone;
use std::str::FromStr;

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

    let mut walker = repository.revwalk().expect("Unable to walk revisions");

    // walker needs to be initialized see https://github.com/rust-lang/git2-rs/blob/master/examples/log.rs#L120
    // TODO: accept a revision identifier (ie branch name, commit id, etc.) and initialize revwalk
    // with this instead
    walker
        .push_head()
        .expect("Unable to select HEAD for revwalker");

    // Some things in the walk are not commits
    let commits: Vec<_> = walker
        .flat_map(|oid| {
            repository.find_commit(oid.expect("Unable to get object id from repository"))
        })
        .collect();

    // TODO: might want to wrap walker in a double ended iterator implementation to let us walk
    // backwards (ie when we paginate backwards) alternatively need to track depth into the walk
    println!("Hello, world!");

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

    let mut revwalk_index: usize = 0;

    loop {
        terminal.draw(|rect| {
            let size = rect.size();
            let chunks = tui::layout::Layout::default()
                .direction(tui::layout::Direction::Vertical)
                .margin(2)
                .constraints(
                    [
                        tui::layout::Constraint::Length(7),
                        tui::layout::Constraint::Min(2),
                    ]
                    .as_ref(),
                )
                .split(size);

            let commit_items: Vec<_> = commits
                .iter()
                .skip(revwalk_index)
                .take(chunks[0].height as usize)
                .map(commit_list_item)
                .collect();

            let commits_block = tui::widgets::Block::default().title("Commits");
            let list = tui::widgets::List::new(commit_items)
                .block(commits_block)
                .highlight_style(tui::style::Style::default());

            let details_block = commit_details(
                &repository,
                &commits
                    .iter()
                    .nth(revwalk_index)
                    .expect("unexpected missing commit"),
            )
            .block(tui::widgets::Block::default().title("Details"));

            rect.render_widget(list, chunks[0]);
            rect.render_widget(details_block, chunks[1])
        })?;

        // TODO: what if someone commits while this is blocking?
        {
            use crossterm::event::KeyCode::*;
            match rx.recv()? {
                Event::Input(event) => match event.code {
                    Char('q') => {
                        crossterm::terminal::disable_raw_mode()?;
                        terminal.show_cursor()?;
                        crossterm::execute!(
                            std::io::stdout(),
                            crossterm::terminal::LeaveAlternateScreen
                        )?;
                        break;
                    }
                    Down | Char('j') => {
                        // TODO: don't go past the end of the revwalk
                        revwalk_index = revwalk_index + 1;
                    }
                    Up | Char('k') => {
                        revwalk_index = revwalk_index.saturating_sub(1);
                    }
                    _ => {}
                },
                Event::Tick => {}
            }
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
        tui::text::Span::styled(time, tui::style::Style::default()),
        tui::text::Span::raw(" "),
        tui::text::Span::styled(title, tui::style::Style::default()),
        tui::text::Span::raw(" "),
        tui::text::Span::styled(author, tui::style::Style::default()),
    ]))
}

fn commit_details(
    repo: &git2::Repository,
    commit: &git2::Commit,
) -> tui::widgets::Paragraph<'static> {
    let mut details = format!(
        "{}\n{}\n",
        commit.id(),
        commit
            .message()
            .unwrap_or_else(|| "INVALID UTF8 IN COMMIT MESSAGE\n")
    );
    if commit.parents().len() <= 1 {
        let parent_tree = commit.parent(0).ok().map(|p| p.tree().ok()).flatten();
        let diff = repo
            .diff_tree_to_tree(parent_tree.as_ref(), commit.tree().ok().as_ref(), None)
            .expect("Unable to create diff");
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            match line.origin() {
                ' ' | '+' | '-' => details.push_str(line.origin().to_string().as_str()),
                _ => {}
            }
            details.push_str(std::str::from_utf8(line.content()).unwrap());
            true
        })
        .expect("Unable to format diff");
    }
    tui::widgets::Paragraph::new(details)
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
