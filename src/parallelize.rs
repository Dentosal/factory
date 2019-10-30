use std::collections::HashSet;

use super::depgraph::IdGraph;
use super::StepId;

pub struct Parallelizer {
    graph: IdGraph,
    pending: HashSet<StepId>,
    running: HashSet<StepId>,
}
impl Parallelizer {
    pub fn from_graph(graph: IdGraph) -> Self {
        // let target: StepId = graph.targets().into_iter().collect::<Vec<_>>()[0];
        // println!("{}", target);

        let pending = graph.nodes();
        Self {
            graph,
            pending,
            running: HashSet::new(),
        }
    }

    pub fn is_done(&self) -> bool {
        self.pending.is_empty() && self.running.is_empty()
    }

    pub fn total_count(&self) -> u64 {
        self.graph.nodes().len() as u64
    }

    pub fn pending_count(&self) -> u64 {
        self.pending.len() as u64
    }

    pub fn running_count(&self) -> u64 {
        self.running.len() as u64
    }

    pub fn completed_count(&self) -> u64 {
        self.total_count() - self.pending_count() - self.running_count()
    }

    pub fn get_task(&mut self) -> Option<StepId> {
        'outer: for p in self.pending.clone().into_iter() {
            for dep in self.graph.dependencies_of(p).iter() {
                if self.running.contains(&dep) || self.pending.contains(&dep) {
                    continue 'outer;
                }
            }
            self.running.insert(p);
            self.pending.remove(&p);
            return Some(p);
        }
        None
    }

    pub fn mark_complete(&mut self, step: StepId) {
        assert!(self.running.contains(&step));
        self.running.remove(&step);
    }
}
