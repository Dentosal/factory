use pyo3::types::*;
use std::collections::HashSet;
use std::fmt;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct StepId(u64);
impl StepId {
    pub fn first() -> Self {
        Self(0)
    }

    pub fn take(&mut self) -> Self {
        let old = self.0;
        self.0 += 1;
        Self(old)
    }
}
impl fmt::Debug for StepId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl fmt::Display for StepId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct Step<'a> {
    pub id: StepId,
    pub requires: HashSet<StepId>,
    pub py_obj: Option<&'a PyAny>,
    /// Display name, for debugging / visualization only
    pub name: String,
}
