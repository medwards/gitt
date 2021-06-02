use git2::{Commit, Repository, Revspec};

#[derive(PartialEq, Eq)]
pub enum AppState {
    Commits,
    Finished,
}

pub struct AppModel {
    pub app_state: AppState,
    repository: Repository,
    revspec: Option<String>,
    revision_index: usize,
    revision_max: usize,
}

impl AppModel {
    pub fn new(app_state: AppState, repository: Repository, revspec: Option<String>) -> Self {
        let mut model = Self {
            app_state,
            repository,
            revspec: None,
            revision_index: 0,
            revision_max: 0,
        };
        model.set_revision(revspec);
        model
    }

    pub fn repository(&self) -> &Repository {
        &self.repository
    }

    pub fn set_revision(&mut self, revision: Option<String>) {
        // TODO: replace hacky rev validation
        // Can't store the raw Revspec because it contains a reference to the repository
        if let Some(ref rev) = revision {
            self.repository
                .revparse(rev)
                .expect("Invalid revision specifier");
        };
        self.revspec = revision;
        self.revision_index = 0;
        self.revision_max = self.walker().count();
    }

    pub fn commits(&self, max: usize) -> Vec<Commit> {
        self.walker()
            .flat_map(|oid| {
                self.repository
                    .find_commit(oid.expect("Revwalk unable to get oid"))
            })
            .skip(self.revision_index)
            .take(max)
            .collect()
    }

    // walker needs to be initialized see https://github.com/rust-lang/git2-rs/blob/master/examples/log.rs#L120
    // TODO: accept a revision identifier (ie branch name, commit id, etc.) and initialize revwalk
    // with this instead
    fn walker(&self) -> git2::Revwalk {
        let mut walker = self
            .repository
            .revwalk()
            .expect("Unable to initialize revwalk");
        if let Some(rev) = self.revspec.as_ref() {
            let rev = self
                .repository
                .revparse(rev.as_str())
                .expect("Invalid revision specifier");
            walker
                .push(rev.from().expect("missing spec").id())
                .expect("Unable to push ref onto revwalk");
        } else {
            walker
                .push_head()
                .expect("Unable to push head onto revwalk");
        }
        walker
    }

    pub fn go_to_first(&mut self) {
        self.revision_index = 0;
    }

    pub fn go_to_last(&mut self) {
        self.revision_index = self.revision_max;
    }

    pub fn remaining(&self, skip: usize) -> usize {
        (self.revision_max - self.revision_index).saturating_sub(skip)
    }

    pub fn increment(&mut self) {
        if self.revision_index < self.revision_max - 1 {
            self.revision_index = self.revision_index + 1;
        }
    }

    pub fn decrement(&mut self) {
        self.revision_index = self.revision_index.saturating_sub(1);
    }
}
