use std::collections::HashSet;
use std::path::PathBuf;

use git2::{Commit, Oid, Repository};
use tui::style::{Color, Style};
use tui::text::{Span, Spans};
use tui::widgets::TableState;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Commits,
    Details,
    Finished,
}

#[derive(Clone, PartialEq, Eq)]
pub enum CommitFilter {
    Path((PathBuf, HashSet<Oid>)),
    Ids(HashSet<Oid>),
    Text(String), // TODO: author? time?
}

impl CommitFilter {
    pub fn apply<'a>(&self, commit: &'a Commit<'a>, repository: &'a Repository) -> bool {
        match self {
            Self::Path((path_match, tree_diff)) => {
                if tree_diff.contains(&commit.id()) {
                    return false;
                }
                let tree = commit.tree().ok();
                let parent_tree = commit.parent(0).ok().map(|p| p.tree().ok()).flatten();
                let diff = repository
                    .diff_tree_to_tree(parent_tree.as_ref(), tree.as_ref(), None)
                    .expect("Unable to create diff");
                diff.deltas().any(|delta| {
                    let old_file_matches = delta
                        .old_file()
                        .path()
                        .map(|p| p.starts_with(path_match))
                        .unwrap_or(false);
                    let new_file_matches = delta
                        .new_file()
                        .path()
                        .map(|p| p.starts_with(path_match))
                        .unwrap_or(false);
                    old_file_matches || new_file_matches
                })
            }
            Self::Ids(oids) => oids.contains(&commit.id()),
            _ => unimplemented!(),
        }
    }
}

pub struct CommitView<'a> {
    repository: &'a Repository,
    filters: &'a [CommitFilter],
    walker: Box<dyn Iterator<Item = Result<Oid, git2::Error>> + 'a>,
}

impl<'a> CommitView<'a> {
    pub fn new(
        repository: &'a Repository,
        revision: Option<&String>,
        filters: &'a [CommitFilter],
    ) -> Self {
        let revision = revision.map(|revspec| {
            repository
                .revparse(revspec.as_str())
                .expect("Invalid revision specifier")
        });

        let mut walker: git2::Revwalk<'a> =
            repository.revwalk().expect("Unable to initialize revwalk");
        if let Some(rev) = revision.as_ref() {
            walker
                .push(
                    rev.from()
                        .expect("revision specifier not converted into oid")
                        .id(),
                )
                .expect("Unable to push ref onto revwalk");
        } else {
            walker
                .push_head()
                .expect("Unable to push head onto revwalk");
        }

        let walker: Box<dyn Iterator<Item = Result<Oid, git2::Error>>> = if filters.is_empty() {
            Box::new(walker)
        } else {
            Box::new(walker.filter(move |result| {
                let oid = result.as_ref().copied().expect("blah");
                repository
                    .find_commit(oid)
                    .and_then(|commit| {
                        Ok(filters
                            .iter()
                            .any(|filter| filter.apply(&commit, repository)))
                    })
                    .unwrap_or(false)
            }))
        };

        Self {
            repository,
            filters,
            walker,
        }
    }
}

impl<'a> Iterator for CommitView<'a> {
    type Item = Commit<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.walker.next() {
            Some(oid) => self
                .repository
                .find_commit(oid.expect("Revwalk unable to get oid"))
                .ok(),
            None => None,
        }
    }
}
pub struct AppModel {
    pub app_state: AppState,
    repository: Repository,
    revspec: Option<String>,
    filters: Vec<CommitFilter>,
    revision_index: usize,
    revision_window_index: TableState,
    revision_window_length: usize,
    revision_max: usize,
    diff_index: usize,
    diff_window_length: usize,
    diff_length: usize,
}

impl AppModel {
    pub fn new(
        app_state: AppState,
        repository: Repository,
        revspec: Option<String>,
        filters: Vec<CommitFilter>,
    ) -> Result<Self, git2::Error> {
        let mut model = Self {
            app_state,
            repository,
            revspec: None,
            filters,
            revision_index: 0,
            revision_window_index: TableState::default(),
            revision_window_length: 0,
            revision_max: 0,
            diff_index: 0,
            diff_window_length: 1,
            diff_length: 1,
        };
        model.set_revision(revspec)?;
        Ok(model)
    }

    pub fn set_revision(&mut self, revision: Option<String>) -> Result<(), git2::Error> {
        // TODO: replace hacky rev validation
        // Can't store the raw Revspec because it contains a reference to the repository
        if let Some(ref rev) = revision {
            let _ = self.repository.revparse(rev)?;
        };
        self.revspec = revision;
        self.revision_index = 0;
        self.revision_window_index.select(Some(0));
        self.revision_window_length = self.walker().count();
        self.revision_max = self.walker().count();
        if self.revision_max == 0 {
            return Err(git2::Error::from_str("No commits found"));
        }
        self.diff_index = 0;
        self.diff_window_length = 1;
        self.diff_length = self.diff().len();
        Ok(())
    }

    // Returns commits from revision_index to revision_index + revision_window_length
    pub fn commits(&self) -> Vec<Commit> {
        self.walker()
            .skip(self.revision_index)
            .take(self.revision_window_length)
            .collect()
    }

    pub fn commit(&self) -> Commit {
        // TODO: reuse commits?
        // TODO: could be empty (or nth goes off the edge of the iterator)
        self.walker()
            .skip(self.revision_index)
            .nth(self.revision_window_index.selected().unwrap_or(0))
            .expect("Unexpected missing commit")
    }

    pub fn diff(&self) -> Vec<Spans> {
        let commit = self.commit();
        let mut text = vec![Spans::from(vec![
            Span::raw(
                commit
                    .as_object()
                    .short_id()
                    .expect("Unable to write short_id")
                    .as_str()
                    .expect("short_id was not valid utf8")
                    .to_string(),
            ),
            Span::raw(" - ".to_string()),
            Span::raw(commit.id().to_string()),
        ])];
        text.append(
            &mut commit
                .message()
                .unwrap_or_else(|| "INVALID MESSAGE")
                .split("\n")
                .map(|s| s.trim_end().to_string())
                .map(|s| Spans::from(vec![Span::raw(s)]))
                .collect(),
        );

        if commit.parents().len() <= 1 {
            let parent_tree = commit.parent(0).ok().map(|p| p.tree().ok()).flatten();
            let diff = self
                .repository
                .diff_tree_to_tree(parent_tree.as_ref(), commit.tree().ok().as_ref(), None)
                .expect("Unable to create diff");
            diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
                let (origin, style) = match line.origin() {
                    'F' => {
                        text.append(
                            &mut std::str::from_utf8(line.content())
                                .unwrap()
                                .split("\n")
                                .map(|s| s.trim_end().to_string())
                                .map(|s| {
                                    Spans::from(vec![Span::styled(
                                        s,
                                        Style::default().fg(Color::Gray),
                                    )])
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
                        std::str::from_utf8(line.content())
                            .unwrap()
                            .trim_end()
                            .to_string(),
                        style,
                    ),
                ];
                text.push(Spans::from(spans));
                true
            })
            .expect("Unable to format diff");
        }
        text
    }

    pub fn walker(&self) -> CommitView {
        CommitView::new(&self.repository, self.revspec.as_ref(), &self.filters)
    }

    pub fn revision_index(&self) -> usize {
        self.revision_index
    }

    pub fn revision_window(&self) -> (&TableState, usize) {
        (&self.revision_window_index, self.revision_window_length)
    }
    pub fn resize_revision_window(&mut self, length: usize) {
        assert!(self.revision_window_index.selected().unwrap_or(0) <= length);
        // TODO: just set the length and then check the count with self.commits().count()
        let commit_count = self.walker().skip(self.revision_index).take(length).count();
        // If there are not enough commits to fill the window, shrink it
        // This can happen if there are very few commits in the repository, or the window was
        // resized to be larger after scrolling to near the end of the list of commits
        self.revision_window_length = std::cmp::min(length, commit_count);
    }

    pub fn go_to_first_revision(&mut self) {
        self.revision_index = 0;
        self.revision_window_index.select(Some(0));
        self.diff_reset();
    }

    pub fn go_to_last_revision(&mut self) {
        self.revision_index = self.revision_max - self.revision_window_length;
        self.revision_window_index
            .select(Some(self.revision_window_length - 1));
        self.diff_reset();
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
        self.diff_reset();
    }

    pub fn decrement_revision(&mut self) {
        if self.revision_window_index.selected().unwrap_or(0) > 0 {
            self.revision_window_index
                .select(self.revision_window_index.selected().map(|s| s - 1));
        } else {
            self.revision_index = self.revision_index.saturating_sub(1);
        }
        self.diff_reset();
    }

    fn diff_reset(&mut self) {
        self.diff_index = 0;
        self.diff_window_length = 1;
        self.diff_length = self.diff().len();
    }

    pub fn resize_diff_window(&mut self, window_length: usize) {
        self.diff_window_length = window_length;
    }

    pub fn diff_line_scroll(&self) -> (usize, usize, usize) {
        (self.diff_index, self.diff_window_length, self.diff_length)
    }

    pub fn go_to_first_diff_line(&mut self) {
        self.diff_index = 0;
    }

    pub fn go_to_last_diff_line(&mut self) {
        self.diff_index = self.diff_length.saturating_sub(self.diff_window_length)
    }

    pub fn increment_diff_line(&mut self) {
        if self.diff_index < self.diff_length.saturating_sub(self.diff_window_length) {
            self.diff_index = self.diff_index + 1;
        }
    }

    pub fn decrement_diff_line(&mut self) {
        self.diff_index = self.diff_index.saturating_sub(1);
    }
}
