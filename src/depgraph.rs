use std::collections::{HashMap, HashSet};

use super::{RunStatistics, Step, StepId};

#[derive(Debug, Clone)]
pub struct IdGraph(HashMap<StepId, HashSet<StepId>>);
impl IdGraph {
    pub fn from_steps(steps: &[Step]) -> Self {
        Self(
            steps
                .iter()
                .map(|step| (step.id, step.requires.clone()))
                .collect(),
        )
    }

    pub fn nodes(&self) -> HashSet<StepId> {
        self.0.keys().copied().collect()
    }

    pub fn dependencies_of<'a>(&'a self, step: StepId) -> &'a HashSet<StepId> {
        &self.0[&step]
    }

    pub fn targets(&self) -> HashSet<StepId> {
        let mut keys = self.nodes();
        for ids in self.0.values() {
            for id in ids.iter() {
                keys.remove(id);
            }
        }
        keys
    }
}
/// Produce graphviz dot representation of the dependency graph
pub fn to_dot(steps: &[Step], stats: RunStatistics) -> String {
    let mut dot = String::new();
    dot.push_str("digraph D {\n");
    for s in steps.iter() {
        let stat = stats.commands.get(&s.id);
        dot.push_str(&format!(
            "node{} [shape=box,peripheries={},label=\"{}: {}\n{}\"]\n",
            s.id,
            if s.target_name.is_some() { 2 } else { 1 },
            s.id,
            s.name,
            stat.map(|st| format!("{:?} {}", st.time, if st.fresh() { "[fresh]" } else { "" }))
                .unwrap_or_else(String::new)
        ));
    }
    for s in steps.iter() {
        for r in s.requires.iter() {
            dot.push_str(&format!("node{} -> node{}\n", r, s.id));
        }
    }
    dot.push_str("}\n");
    dot
}

/// Remove redundant dependencies
/// TODO: a fast algorithm instead of brute force
pub fn linearize(steps: &mut Vec<Step>) {
    let original = steps.clone();
    for step in steps.iter_mut() {
        if step.requires.len() > 1 {
            // Paths without current step
            let ps: Vec<Vec<StepId>> = paths_from(&original, step.id)
                .into_iter()
                .map(|mut p| {
                    p.pop();
                    p
                })
                .collect();

            // Redudant paths have same prefix as one of the longer paths
            for (i0, p0) in ps.iter().enumerate() {
                for (i1, p1) in ps.iter().enumerate() {
                    if i0 != i1 && is_prefix_of(p0, p1) {
                        step.requires.remove(p0.last().unwrap());
                        break;
                    }
                }
            }
        }
    }
}

/// Is a prefix of other slice
fn is_prefix_of<T: PartialEq>(prefix: &[T], full: &[T]) -> bool {
    if prefix.len() > full.len() {
        false
    } else {
        prefix.iter().zip(full.iter()).all(|(a, b)| a == b)
    }
}

pub fn step_index_by_id(steps: &[Step], cursor: StepId) -> usize {
    for (i, step) in steps.iter().enumerate() {
        if step.id == cursor {
            return i;
        }
    }
    panic!("Step {:?} doesn't exist", cursor);
}

fn paths_from(steps: &[Step], cursor: StepId) -> Vec<Vec<StepId>> {
    let i = step_index_by_id(steps, cursor);
    if steps[i].requires.is_empty() {
        return vec![vec![cursor]];
    } else {
        steps[i]
            .requires
            .iter()
            .cloned()
            .flat_map(|req| {
                paths_from(steps, req).into_iter().map(|mut path| {
                    path.push(cursor);
                    path
                })
            })
            .collect()
    }
}
