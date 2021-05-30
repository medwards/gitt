use git2::{Commit, Repository};

pub struct AppModel {
    repository: Repository,
    revision: Option<String>,
    revision_index: usize,
    revision_max: usize,
}

impl AppModel {
    pub fn new(repository: Repository, revision: Option<String>, revision_index: usize) -> Self {
        let mut model = Self {
            repository,
            revision: None,
            revision_index,
            revision_max: 0,
        };
        model.set_revision(revision);
        model
    }

    pub fn repository(&self) -> &Repository {
        &self.repository
    }

    pub fn set_revision(&mut self, revision: Option<String>) {
        self.revision = revision;
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
        if let Some(git_ref) = self.revision.as_ref() {
            walker
                .push_ref(git_ref)
                .expect("Unable to push ref onto revwalk");
        } else {
            walker
                .push_head()
                .expect("Unable to push head onto revwalk");
        }
        walker
    }

    pub fn remaining(&self, skip: usize) -> usize {
        self.revision_max - self.revision_index - skip
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
