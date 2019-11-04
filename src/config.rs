use pyo3::{prelude::*, types::*};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::depgraph;
use super::step::{Step, StepId};

#[derive(Debug, Serialize, Deserialize)]
pub struct TomlConfig {
    /// Root directory override.
    /// Set to correct value after reading this file if None.
    pub root_dir: Option<PathBuf>,
    /// Python config file
    pub config: PathBuf,
    /// Default target to execute
    pub default_target: Option<String>,
    /// Number of threads, logical CPU core count is used by default
    pub threads: Option<usize>,
    /// Output file for graphviz dot file containing build metadata
    pub stats_dot: Option<PathBuf>,
}
impl TomlConfig {
    pub fn root_dir(&self) -> PathBuf {
        self.root_dir.clone().unwrap()
    }

    pub fn threads(&self) -> usize {
        self.threads.unwrap_or_else(num_cpus::get)
    }
}

/// Read toml and python config files
pub fn read<'py>(
    py: Python<'py>,
    dir_path: &Path,
) -> PyResult<(Vec<Step<'py>>, &'py PyDict, TomlConfig)> {
    let contents = fs::read(dir_path.join("Factory.toml")).expect("Factory.toml missing");
    let mut toml_config: TomlConfig = toml::from_slice(&contents).expect("Invalid toml");

    toml_config.root_dir = Some(if let Some(cfg_rd) = toml_config.root_dir {
        cfg_rd.canonicalize().unwrap()
    } else {
        dir_path.canonicalize().unwrap()
    });

    // TODO: Validate configuration options

    // Import the python configuration
    let py_code_path = toml_config.root_dir().join(&toml_config.config);
    let py_config = if py_code_path.is_dir() {
        let sys = py.import("sys")?;
        let sys_path: &PyList = sys.get("path")?.downcast_ref::<PyList>()?;
        sys_path.insert(0, toml_config.root_dir().to_str().unwrap())?;
        PyModule::import(py, "config")?
    } else {
        let py_code_path = toml_config.root_dir().join(&toml_config.config);
        let py_code =
            String::from_utf8(fs::read(&py_code_path).expect("Python config file not found"))
                .unwrap();
        PyModule::from_code(py, &py_code, py_code_path.to_str().unwrap(), "config")?
    };

    // Create objects for calling functions
    let py_pathlib = py.import("pathlib")?;
    let py_path: &PyAny = py_pathlib.get("Path")?.extract()?;
    let root_path = py_path.call1((toml_config.root_dir().to_str().unwrap(),))?;
    let cfg_dict = PyDict::new(py);
    cfg_dict.set_item("root_dir", toml_config.root_dir().to_str().unwrap())?;
    cfg_dict.set_item("threads", toml_config.threads())?;

    // Call initialization function
    if let Ok(py_init) = py_config.get("init") {
        py_init.call1((cfg_dict,))?;
    }

    // Call filesystem initialization function
    if let Ok(py_init_fs) = py_config.get("init_fs") {
        py_init_fs.call1((root_path, cfg_dict))?;
    }

    // Get step data from the configuration
    let mut steps: Vec<Step> = Vec::new();
    let mut next_id = StepId::first();
    let mut step_fn_to_id: Vec<(_, StepId)> = Vec::new();

    let start_id = next_id.take();
    steps.push(Step {
        id: start_id,
        requires: HashSet::new(),
        py_obj: None,
        target_name: None,
        name: "start".to_owned(),
    });

    for (name, value) in py_config.dict().into_iter() {
        let name_str = name.downcast_ref::<PyString>().unwrap().to_string()?;
        if name_str.starts_with("step_") {
            let py_step = value.call1((root_path,))?;
            let n = name_str.splitn(2, '_').last().unwrap();
            let new_steps = create_steps(py, py_step, &mut next_id, start_id, &n, true)?;
            assert!(!new_steps.is_empty());
            step_fn_to_id.push((value, new_steps.last().unwrap().id));
            steps.extend(new_steps);
        }
    }
    // Insert step dependencies (`Step::requires`)
    for step in steps.iter_mut() {
        if let Some(py_obj) = step.py_obj {
            let py_req = py_obj.getattr("requires")?;
            let set: &PySet = py_req.downcast_ref().unwrap();
            let mut required_ids = Vec::new();
            for item in set.iter()? {
                let item = item?;
                // Resolve item
                let mut found = false;
                for (sfn, sid) in step_fn_to_id.iter() {
                    if *sfn == item {
                        required_ids.push(sid);
                        found = true;
                        break;
                    }
                }
                assert!(found);
            }
            assert_eq!(set.len(), required_ids.len());
            step.requires.extend(required_ids);
        }
    }

    depgraph::linearize(&mut steps);
    Ok((steps, &cfg_dict, toml_config))
}

/// `py_step` can be either: FactoryStep, Tuple[FactoryStep], Set[FactoryStep],
/// or any combination of Tuple and Sets of FactoryStep
///
/// This function doesn't fill `requires` field of the step struct from,
/// as not all necessary information is available yet. However, tuples
/// and sets already use `requires` field to mark their internal order.
fn create_steps<'a>(
    py: Python,           // Python GIL
    py_step: &'a PyAny,   // Python `Step` object from src/python/factory.py
    next_id: &mut StepId, // Id for the next step
    requires_id: StepId,  // Requirement for the next step
    step_name: &str,      // Name of the current Python step function
    last_part: bool,      // Is the `step_name` completed after this step
) -> PyResult<Vec<Step<'a>>> {
    if py.is_instance::<PyTuple, _>(py_step)? {
        // Tuple, i.e. a sequence of steps
        let tuple: &PyTuple = py_step.downcast_ref::<PyTuple>().unwrap();
        let t_len = tuple.len();
        let mut steps: Vec<Step> = Vec::new();
        let mut next_requires = requires_id;
        for i in 0..t_len {
            let new_steps = create_steps(
                py,
                tuple.get_item(i),
                next_id,
                next_requires,
                step_name,
                i + 1 == t_len,
            )?;
            assert!(!new_steps.is_empty());
            next_requires = new_steps.last().unwrap().id; // FIXME: last?
            steps.extend(new_steps);
        }
        Ok(steps)
    } else if py.is_instance::<PySet, _>(py_step)? {
        // Set, i.e. steps ran in arbitrary order
        let set: &PySet = py_step.downcast_ref::<PySet>().unwrap();

        // End "synchronization" step
        let mut end_step = Step {
            id: next_id.take(),
            requires: HashSet::new(),
            py_obj: None,
            target_name: Some(step_name.to_owned()),
            name: format!("collect {}", step_name),
        };

        // Go through all items
        let mut steps: Vec<Step> = Vec::new();
        for item in set.iter()? {
            let new_steps = create_steps(py, item?, next_id, requires_id, step_name, false)?;
            assert!(!new_steps.is_empty());

            // Insert `requires` fields
            end_step.requires.insert(new_steps.last().unwrap().id); // FIXME: Last or all?

            steps.extend(new_steps);
        }
        steps.push(end_step);
        Ok(steps)
    } else {
        // A single step
        let sub_name = if let Ok(q_cmd) = py_step.getattr("cmd")?.getattr("cmd") {
            let sn = q_cmd.get_item(0).unwrap().to_string();
            sn.split('/').last().unwrap().to_owned()
        } else if let Ok(q_name) = py_step.getattr("cmd")?.getattr("name") {
            format!("expr {}", q_name)
        } else {
            "<dynamic>".to_owned()
        };

        let mut requires = HashSet::new();
        requires.insert(requires_id);
        Ok(vec![Step {
            id: next_id.take(),
            requires,
            py_obj: Some(py_step),
            target_name: if last_part {
                Some(step_name.to_owned())
            } else {
                None
            },
            name: format!("{}: {}", step_name, sub_name),
        }])
    }
}
