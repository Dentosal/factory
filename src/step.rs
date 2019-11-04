use pyo3::types::*;
use std::collections::HashSet;
use std::fmt;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    /// Step id
    pub id: StepId,
    /// Dependencies
    pub requires: HashSet<StepId>,
    /// Associated Python object, if any
    pub py_obj: Option<&'a PyAny>,
    /// Name used to specify which target to build.
    /// Only available if this step can be used as a build target.
    pub target_name: Option<String>,
    /// Display name, for debugging / visualization only
    pub name: String,
}
