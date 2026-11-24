//! What is known of the nodes of a pipeline: their wiring, and what a Check and the Runs found.

use std::collections::{BTreeMap, HashMap, HashSet};

use super::Columns;
use super::gallery::Gallery;
use crate::polars::prelude::{DataFrame, DataType, Field, Schema};
use crate::{Catalog, Node, Pipeline, Registry, is_sink};

/// What is known of a node, which colours the border of its card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Nothing, but that it is wired well.
    Unknown,
    /// Its columns resolve.
    Checked,
    /// It is wired badly, or its columns do not resolve.
    Rejected,
    /// A Run computed it.
    Ran,
    /// A Run of it failed.
    Failed,
}

/// What is known of each node of a pipeline, by its id.
#[derive(Default)]
pub struct Known {
    /// What is wrong with the wiring of each node, as of the latest edit.
    wiring: BTreeMap<String, String>,
    /// The nodes that write their frame, as to a file, which every Run does.
    writes: Vec<String>,
    found: BTreeMap<String, Found>,
}

/// What a Check and the Runs found of a node, until it or one it reads is edited.
#[derive(Default)]
struct Found {
    columns: Option<Columns>,
    /// Its frame, if a Run saved it, and the images in that.
    frame: Option<(DataFrame, Gallery)>,
    /// Why the latest Run failed, if it was to save this node or have it write.
    failed: Option<String>,
    /// Whether a Run computed it.
    is_run: bool,
}

impl Known {
    /// Notes how each node is wired and which nodes write; returns what is wrong with the whole.
    pub fn wire(
        &mut self,
        pipeline: &Pipeline,
        registry: &Registry,
        catalog: &Catalog,
    ) -> Option<String> {
        self.wiring.clear();
        self.writes.clear();
        // a node that reads no other reads the disk, which is left to a Check: it is not built
        let sources = pipeline
            .nodes
            .iter()
            .filter(|node| {
                catalog
                    .blocks
                    .get(node.block.as_str())
                    .is_some_and(|block| {
                        block
                            .inputs
                            .is_empty()
                    })
            })
            .map(|node| (node.id.clone(), DataFrame::empty()))
            .collect();
        let frames = match pipeline.frames_from(registry, &sources) {
            Ok(frames) => frames,
            Err(e) => return Some(e.to_string()),
        };
        for (id, frame) in frames {
            match frame {
                Ok(frame) if is_sink(&frame) => self
                    .writes
                    .push(id.to_owned()),
                Ok(_) => {}
                Err(e) => {
                    self.wiring
                        .insert(id.to_owned(), e.to_string());
                }
            }
        }
        None
    }

    /// Forgets what was found of the nodes that were edited and of all that read them.
    pub fn forget(
        &mut self,
        pipeline: &Pipeline,
        is_edited: impl Fn(&Node) -> bool,
    ) {
        let edited = pipeline
            .nodes
            .iter()
            .filter(|node| is_edited(node))
            .map(|node| node.id.as_str());
        let stale = pipeline.downstream(edited);
        self.found
            .retain(|id, _| !stale.contains(id.as_str()));
    }

    /// Forgets what was found of the node of an id, which it no longer has.
    pub fn remove(
        &mut self,
        id: &str,
    ) {
        self.found
            .remove(id);
    }

    /// Lets the frame of a node go, which is no longer to be saved.
    pub fn unsave(
        &mut self,
        id: &str,
    ) {
        if let Some(found) = self
            .found
            .get_mut(id)
        {
            found.frame = None;
        }
    }

    /// Notes the columns that a Check found of each node, or why they do not resolve.
    pub fn checked(
        &mut self,
        checks: BTreeMap<String, Columns>,
    ) {
        for (id, columns) in checks {
            let found = self.of(id);
            found.columns = Some(columns);
            // why a Run failed before may no longer hold
            found.failed = None;
        }
    }

    /// An empty frame of the columns found of each node, to stand in for it in a Check.
    ///
    /// What was found holds until the node or one it reads is edited, so only those are looked at.
    pub fn found(&self) -> HashMap<String, DataFrame> {
        self.found
            .iter()
            .filter_map(|(id, found)| {
                let columns = found
                    .columns
                    .as_ref()?
                    .as_ref()
                    .ok()?;
                let schema: Schema = columns
                    .iter()
                    .cloned()
                    .map(|(name, dtype)| Field::new(name.into(), dtype))
                    .collect();
                Some((id.clone(), DataFrame::empty_with_schema(&schema)))
            })
            .collect()
    }

    /// What a Run is to compute: the pipeline to run, its targets, and the saved frames it reads.
    ///
    /// A Reload first lets go of every frame, so that it computes everything.
    pub fn plan(
        &mut self,
        pipeline: &Pipeline,
        is_reload: bool,
    ) -> (Pipeline, Vec<String>, HashMap<String, DataFrame>) {
        for found in self
            .found
            .values_mut()
        {
            found.failed = None;
            if is_reload {
                found.frame = None;
                found.is_run = false;
            }
        }
        let mut pipeline = pipeline.clone();
        // a frame that is saved is not collected again
        pipeline
            .outputs
            .retain(|id| {
                self.frame(id)
                    .is_none()
            });
        // but every write is done again, as its file may be gone
        let targets = pipeline
            .outputs
            .iter()
            .chain(&self.writes)
            .cloned()
            .collect();
        // a read frame goes along, but not a write's, which it would stand in for
        let read = pipeline
            .nodes
            .iter()
            .flat_map(|node| &node.inputs)
            .filter(|id| {
                !self
                    .writes
                    .contains(id)
            })
            .filter_map(|id| {
                Some((
                    id.clone(),
                    self.frame(id)?
                        .clone(),
                ))
            })
            .collect();
        (pipeline, targets, read)
    }

    /// Notes that a Run computed `targets` and all they read, and keeps the frames it saved.
    pub fn ran(
        &mut self,
        pipeline: &Pipeline,
        registry: &Registry,
        targets: &[String],
        frames: Vec<(String, DataFrame)>,
    ) {
        let targets = targets
            .iter()
            .map(String::as_str);
        for id in pipeline.upstream(targets) {
            self.of(id.to_owned())
                .is_run = true;
        }
        for (id, mut frame) in frames {
            // one unticked while the Run was under way is not saved
            let is_saved = pipeline
                .outputs
                .contains(&id);
            if !is_saved {
                continue;
            }
            // in one chunk, a cell is found in constant time
            frame.rechunk_mut();
            let node = pipeline
                .nodes
                .iter()
                .find(|node| node.id == id);
            // the images of the column that its block has shown, and of no other
            let gallery = node
                .and_then(|node| registry.images(&node.block, node.args.clone()))
                .map(|column| Gallery::of(&frame, &column))
                .unwrap_or_default();
            // a plot draws the figure of a row over again, to zoom and pan
            #[cfg(feature = "plot")]
            let gallery =
                match node.and_then(|node| registry.drawer(&node.block, node.args.clone())) {
                    Some(Ok(drawer)) => gallery.live(&frame, drawer),
                    _ => gallery,
                };
            self.of(id).frame = Some((frame, gallery));
        }
    }

    /// Notes why a Run of `targets` failed; Polars does not say which node it was.
    pub fn failed(
        &mut self,
        targets: Vec<String>,
        error: &str,
    ) {
        for id in targets {
            self.of(id).failed = Some(error.to_owned());
        }
    }

    /// What was found of node `id`, to add to.
    fn of(
        &mut self,
        id: String,
    ) -> &mut Found {
        self.found
            .entry(id)
            .or_default()
    }

    /// The frame of node `id`, if a Run saved it.
    pub fn frame(
        &self,
        id: &str,
    ) -> Option<&DataFrame> {
        let (frame, _) = self.saved(id)?;
        Some(frame)
    }

    /// The images in the frame of node `id`, if a Run saved it.
    pub fn gallery(
        &self,
        id: &str,
    ) -> Option<&Gallery> {
        let (_, gallery) = self.saved(id)?;
        Some(gallery)
    }

    fn saved(
        &self,
        id: &str,
    ) -> Option<&(DataFrame, Gallery)> {
        self.found
            .get(id)?
            .frame
            .as_ref()
    }

    /// The columns of node `id`, or what is wrong with its wiring or its columns.
    pub fn columns(
        &self,
        id: &str,
    ) -> Option<Result<&Vec<(String, DataType)>, &String>> {
        match self.wiring.get(id) {
            Some(e) => Some(Err(e)),
            None => self
                .found
                .get(id)?
                .columns
                .as_ref()
                .map(Result::as_ref),
        }
    }

    /// Why the latest Run failed, if it was to save node `id` or have it write.
    pub fn failure(
        &self,
        id: &str,
    ) -> Option<&str> {
        self.found
            .get(id)?
            .failed
            .as_deref()
    }

    /// What colours the card of node `id`: a Run before a Check.
    pub fn status(
        &self,
        id: &str,
    ) -> Status {
        let found = self.found.get(id);
        let is_run = found.is_some_and(|found| found.is_run);
        match self.columns(id) {
            _ if self
                .failure(id)
                .is_some() =>
            {
                Status::Failed
            }
            Some(Err(_)) => Status::Rejected,
            _ if is_run => Status::Ran,
            Some(Ok(_)) => Status::Checked,
            None => Status::Unknown,
        }
    }

    /// The nodes that a Run computes: those it saves or has write, and all that they read.
    fn reached<'a>(
        &'a self,
        pipeline: &'a Pipeline,
    ) -> HashSet<&'a str> {
        let targets = pipeline
            .outputs
            .iter()
            .chain(&self.writes)
            .map(String::as_str);
        pipeline.upstream(targets)
    }

    /// Whether every node that a Run computes is wired well, with args that its block takes.
    pub fn is_wired(
        &self,
        pipeline: &Pipeline,
    ) -> bool {
        let reached = self.reached(pipeline);
        self.wiring
            .keys()
            .all(|id| !reached.contains(id.as_str()))
    }

    /// Whether a Check is behind every node that a Run computes: until then it goes before a Run.
    pub fn is_checked(
        &self,
        pipeline: &Pipeline,
    ) -> bool {
        let reached = self.reached(pipeline);
        pipeline
            .nodes
            .iter()
            .all(|node| {
                let is_sound = matches!(self.status(&node.id), Status::Checked | Status::Ran);
                // no Check mends bad wiring, which is no matter in a node that no Run computes
                let is_aside = self
                    .wiring
                    .contains_key(&node.id)
                    && !reached.contains(node.id.as_str());
                is_sound || is_aside
            })
    }

    /// The names of every column that a Check found of the nodes of these ids, once each.
    pub fn names<'a>(
        &self,
        ids: impl IntoIterator<Item = &'a String>,
    ) -> Vec<String> {
        let mut names = Vec::new();
        let found = ids
            .into_iter()
            .filter_map(|id| {
                self.columns(id)?
                    .ok()
            })
            .flatten();
        for (name, _) in found {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
        names
    }
}
