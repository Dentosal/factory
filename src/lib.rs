#![feature(option_flattening)]
#![deny(unused_must_use)]
#![warn(clippy::all)]
#![warn(clippy::cargo)]

use crossbeam_channel::{unbounded, Receiver, Sender};
use pyo3::{prelude::*, types::*};
use std::collections::HashMap;
use std::fs;
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

pub fn run(
    _py: Python,
    steps: &[Step],
    cfg_dict: &PyDict,
    toml_config: &TomlConfig,
    _py_factory: &PyModule,
) -> PyResult<RunStatistics> {
    let step_by_id: HashMap<StepId, &Step> = steps.iter().map(|s| (s.id, s)).collect();
    let mut p = parallelize::Parallelizer::from_graph(depgraph::IdGraph::from_steps(&steps));

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

    let pb = ProgressBar::new(p.total_count());
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:20} {pos:>4}/{len:4} {msg}")
            .progress_chars("#>-"),
    );

    loop {
        pb.set_position(p.completed_count());
        pb.set_message(&format!("{} running", p.running_count()));
        while let Some(step_id) = p.get_task() {
            let step = step_by_id[&step_id];
            if let Some(py_obj) = step.py_obj {
                let start = std::time::Instant::now();

                let mut cmd = py_obj.getattr("cmd")?;

                let py_env = py_obj.getattr("env")?.downcast_ref::<PyDict>()?;
                let env: HashMap<String, String> = py_env
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();

                let mut ty: String = cmd.getattr("__class__")?.getattr("__name__")?.to_string();
                if ty.as_str() == "function" {
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
                    }
                    "Expr" => {
                        let expr = cmd.getattr("expr")?;
                        let name: String = cmd.getattr("name")?.extract()?;
                        cfg_dict.set_item(name, expr)?;
                        p.mark_complete(step_id);
                        statistics.commands.insert(
                            step_id,
                            CommandResult {
                                step_id,
                                time: start.elapsed(),
                                data: CommandResultData::Virtual,
                            },
                        );
                    }
                    "Assert" => {
                        let expr: bool = cmd.getattr("expr")?.extract()?;
                        let msg = cmd.getattr("error_msg")?.to_string();
                        if !expr {
                            // TODO: proper error handling
                            panic!("STOP {}", msg);
                        }
                        p.mark_complete(step_id);
                        statistics.commands.insert(
                            step_id,
                            CommandResult {
                                step_id,
                                time: start.elapsed(),
                                data: CommandResultData::Virtual,
                            },
                        );
                    }
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
                println!("ERROR: {:?}", result);
                break;
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
    let py_f_code = String::from_utf8(fs::read("src/python/factory.py").unwrap()).unwrap();
    let py_factory = PyModule::from_code(py, &py_f_code, "factory.py", "factory")?;
    Ok(py_factory)
}
