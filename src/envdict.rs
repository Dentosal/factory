use pyo3::{prelude::*, types::*};
use std::collections::{HashMap, HashSet};

/// None in value position means that this key must be deleted when merging
#[derive(Debug, Default)]
pub struct EnvDict(HashMap<String, Option<String>>);
impl EnvDict {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn from_pydict(py_obj: &PyAny) -> Self {
        let py_env = py_obj.downcast_ref::<PyDict>().expect("Dictionary expected");

        Self(
            py_env
                .iter()
                .map(|(k, v)| {
                    (
                        k.to_string(),
                        if v.is_none() { None } else { Some(v.to_string()) },
                    )
                })
                .collect(),
        )
    }

    /// Combine self with other, preferring values in self
    pub fn merge(self, other: Self) -> Self {
        let keys: HashSet<&String> = self.0.keys().chain(other.0.keys()).collect();
        Self(
            keys.into_iter()
                .map(|key| {
                    if let Some(value) = self.0.get(key) {
                        (key.clone(), value.clone())
                    } else {
                        (key.clone(), other.0.get(key).unwrap().clone())
                    }
                })
                .collect(),
        )
    }

    /// Remove nonexistent keys and convert to HashMap
    pub fn finalize(self) -> HashMap<String, String> {
        self.0
            .into_iter()
            .flat_map(
                |(key, value)| {
                    if let Some(v) = value { Some((key, v)) } else { None }
                },
            )
            .collect()
    }
}
