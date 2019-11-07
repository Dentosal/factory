#![feature(option_flattening)]
#![deny(unused_must_use)]
#![warn(clippy::all)]
#![warn(clippy::cargo)]

use crossbeam_channel::{unbounded, Receiver, Sender};
use pyo3::{prelude::*, types::*};
use std::collections::HashMap;
use std::io;
use std::thread::{self, JoinHandle};

use indicatif::{ProgressBar, ProgressStyle};

pub mod command;
pub mod config;
pub mod depgraph;
pub mod parallelize;
pub mod step;

use self::command::{Command, CommandResult, CommandResultData};
use self::config::TomlConfig;
use self::step::{Step, StepId};

#[derive(Debug, Default)]
pub struct RunStatistics {
    pub commands: HashMap<StepId, CommandResult>,
}
impl RunStatistics {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }
}

/// Assumes ascii-only text, but uses unicode ellipsis
fn truncate_ellipisis(max_len: usize, s: &str) -> String {
    assert!(max_len >= 1);
    if s.len() > max_len {
        let mut res = s[..(max_len - 1)].to_owned();
        res.push('\u{2026}');
        res
    } else {
        s.to_owned()
    }
}

fn find_target_id(steps: &[Step], target_name: &str) -> StepId {
    for step in steps {
        if step.target_name == Some(target_name.to_owned()) {
            return step.id;
        }
    }
    panic!("No step named {}", target_name);
}

#[derive(Debug)]
pub enum RunError {
    Python(PyErr),
    Command(CommandResult),
    Io(io::Error),
}
impl RunError {
    /// Output error state to stderr
    pub fn show(self, py: Python) {
        match self {
            Self::Python(e) => {
                e.print_and_set_sys_last_vars(py);
            },
            Self::Command(c) => {
                c.show();
            },
            other => {
                eprintln!("{:?}", other);
            },
        }
    }
}
impl From<io::Error> for RunError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
impl From<PyErr> for RunError {
    fn from(error: PyErr) -> Self {
        Self::Python(error)
    }
}

pub fn run(
    _py: Python, steps: &[Step], target_name: &str, cfg_dict: &PyDict, toml_config: &TomlConfig, quiet: bool,
    _py_factory: &PyModule,
) -> Result<RunStatistics, RunError>
{
    let target = find_target_id(steps, target_name);
    let step_by_id: HashMap<StepId, &Step> = steps.iter().map(|s| (s.id, s)).collect();
    let mut dep_graph = depgraph::IdGraph::from_steps(&steps);
    dep_graph = dep_graph.focus(target);
    let mut p = parallelize::Parallelizer::from_graph(dep_graph);

    let (to_thread, t_recv) = unbounded::<Option<Command>>();
    let (t_send, from_thread) = unbounded::<CommandResult>();

    let parallel = toml_config.threads();

    let threads: Vec<JoinHandle<()>> = (0..parallel)
        .map(|_| {
            let tx = t_send.clone();
            let rx = t_recv.clone();
            thread::spawn(move || runner(rx, tx))
        })
        .collect();

    let mut statistics = RunStatistics::new();

    let pb = if quiet {
        ProgressBar::hidden()
    } else {
        let p = ProgressBar::new(p.total_count());
        p.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:20} [{pos:>4}/{len:4}] {msg}")
                .progress_chars("##-"),
        );
        p.enable_steady_tick(100);
        p
    };

    let size = terminal_size::terminal_size();
    let term_w = if let Some((terminal_size::Width(w), _)) = size {
        w as usize
    } else {
        80
    };
    let avail_w = if term_w < 44 { 0 } else { term_w - 44 };

    loop {
        while let Some(step_id) = p.get_task() {
            pb.set_position(p.completed_count());
            let mut ids: Vec<StepId> = p.running_ids().into_iter().collect();
            ids.sort();
            pb.set_message(&truncate_ellipisis(
                avail_w,
                &format!(
                    "{}: {:}",
                    ids.len(),
                    ids.into_iter()
                        .map(|id| { step_by_id.get(&id).unwrap().name.clone() })
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
            let step = step_by_id[&step_id];
            if let Some(py_obj) = step.py_obj {
                let start = std::time::Instant::now();

                let mut cmd = py_obj.getattr("cmd")?;

                let py_env = py_obj.getattr("env")?.downcast_ref::<PyDict>().unwrap();
                let env: HashMap<String, String> = py_env
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();

                let mut ty: String = cmd.getattr("__class__")?.getattr("__name__")?.to_string();
                // While to support recursive functions
                while ty.as_str() == "function" {
                    cmd = cmd.call1((cfg_dict,))?;
                    ty = cmd.getattr("__class__")?.getattr("__name__")?.to_string();
                }
                match ty.as_str() {
                    "Cmd" => {
                        to_thread
                            .send(Some(Command::new(
                                step_id,
                                cmd,
                                toml_config.root_dir.clone().unwrap().as_ref(),
                                env,
                            )?))
                            .unwrap();
                    },
                    "Expr" => {
                        let expr = cmd.getattr("expr")?;
                        let name: String = cmd.getattr("name")?.extract()?;
                        cfg_dict.set_item(name, expr)?;
                        p.mark_complete(step_id);
                        statistics.commands.insert(step_id, CommandResult {
                            step_id,
                            time: start.elapsed(),
                            data: CommandResultData::Virtual,
                        });
                    },
                    "Assert" => {
                        let expr: bool = cmd.getattr("expr")?.extract()?;
                        let msg = cmd.getattr("error_msg")?.to_string();
                        if !expr {
                            // TODO: proper error handling
                            panic!("STOP {}", msg);
                        }
                        p.mark_complete(step_id);
                        statistics.commands.insert(step_id, CommandResult {
                            step_id,
                            time: start.elapsed(),
                            data: CommandResultData::Virtual,
                        });
                    },
                    _ => unimplemented!("??"),
                }
            } else {
                p.mark_complete(step_id);
            }
        }

        if p.is_done() {
            break;
        } else {
            let result = from_thread.recv().unwrap();

            if !result.success() {
                pb.abandon_with_message("error");
                return Err(RunError::Command(result));
            }

            p.mark_complete(result.step_id);
            statistics.commands.insert(result.step_id, result);
        }
    }

    for _ in 0..parallel {
        to_thread.send(None).unwrap();
    }

    for t in threads.into_iter() {
        t.join().unwrap();
    }

    pb.finish_with_message("done");

    Ok(statistics)
}

fn runner(rx: Receiver<Option<Command>>, tx: Sender<CommandResult>) {
    while let Ok(Some(cmd)) = rx.recv() {
        let result = cmd.run();
        tx.send(result).unwrap();
    }
}

/// Verify Python version
pub fn check_python(py: Python) -> PyResult<()> {
    let sys = py.import("sys")?;
    let hexversion: u32 = sys.get("hexversion")?.extract()?;
    assert!(hexversion >= 0x0307_0000); // at least 3.7.0
    Ok(())
}

/// Verify Python version
pub fn get_py_factory(py: Python) -> PyResult<&PyModule> {
    let py_f_code = include_str!("python/factory.py");
    let py_factory = PyModule::from_code(py, &py_f_code, "factory.py", "factory")?;
    Ok(py_factory)
}
