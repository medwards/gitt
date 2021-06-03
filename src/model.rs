use git2::{Commit, Repository};
use tui::widgets::ListState;

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
    revision_window_index: ListState,
    revision_window_length: usize,
    revision_max: usize,
}

impl AppModel {
    pub fn new(app_state: AppState, repository: Repository, revspec: Option<String>) -> Self {
        let mut model = Self {
            app_state,
            repository,
            revspec: None,
            revision_index: 0,
            revision_window_index: ListState::default(),
            revision_window_length: 1,
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
        self.revision_window_index.select(Some(0));
        self.revision_window_length = 1;
        self.revision_max = self.walker().count();
    }

    // Returns commits from revision_index to revision_index + revision_window_length
    // This means something has to initialize revision_window_length
    // (otherwise it defaults to 1)
    pub fn commits(&self) -> Vec<Commit> {
        self.walker()
            .flat_map(|oid| {
                self.repository
                    .find_commit(oid.expect("Revwalk unable to get oid"))
            })
            .skip(self.revision_index)
            .take(self.revision_window_length)
            .collect()
    }

    pub fn commit(&self) -> Commit {
        // TODO: reuse commits?
        self.walker()
            .flat_map(|oid| {
                self.repository
                    .find_commit(oid.expect("Revwalk unable to get oid"))
            })
            .skip(self.revision_index)
            .nth(self.revision_window_index.selected().unwrap_or(0))
            .expect("Unexpected missing commit")
    }

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

    pub fn revision_window(&self) -> (&ListState, usize) {
        (&self.revision_window_index, self.revision_window_length)
    }
    pub fn resize_revision_window(&mut self, length: usize) {
        // TODO: On resize check that revision_window_index is still within the window
        self.revision_window_length = length;
    }

    pub fn go_to_first_revision(&mut self) {
        self.revision_index = 0;
        self.revision_window_index.select(Some(0))
    }

    pub fn go_to_last_revision(&mut self) {
        self.revision_index = self.revision_max - self.revision_window_length;
        self.revision_window_index
            .select(Some(self.revision_window_length - 1))
    }

    pub fn increment_revision(&mut self) {
        if self.revision_window_index.selected().unwrap_or(0) < self.revision_window_length - 1 {
            // Increment the position in the window
            self.revision_window_index
                .select(Some(self.revision_window_index.selected().unwrap_or(0) + 1));
        } else if self.revision_window_index.selected().unwrap_or(0)
            == self.revision_window_length - 1
            && self.revision_index < self.revision_max - self.revision_window_length
        {
            // Increment the entire window
            self.revision_index = self.revision_index + 1;
        }
    }

    pub fn decrement_revision(&mut self) {
        if self.revision_window_index.selected().unwrap_or(0) > 0 {
            self.revision_window_index
                .select(self.revision_window_index.selected().map(|s| s - 1));
        } else {
            self.revision_index = self.revision_index.saturating_sub(1);
        }
    }
}
