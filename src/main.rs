use std::str::FromStr;

fn main() {
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
    walker.for_each(|oid| {
        println!("{:?}", oid);
        println!(
            "{:?}",
            repository
                .find_commit(oid.expect("Unable to get object id from repository"))
                .ok()
        );
    });
    // TODO: might want to wrap walker in a double ended iterator implementation to let us walk
    // backwards (ie when we paginate backwards) alternatively need to track depth into the walk
    println!("Hello, world!");
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
