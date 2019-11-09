use pyo3::{prelude::*, types::*};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use super::StepId;
use crate::config_file::ExecConfig;

#[derive(Debug)]
pub struct CommandResult {
    pub step_id: StepId,
    pub time: Duration,
    pub data: CommandResultData,
}
impl CommandResult {
    pub fn success(&self) -> bool {
        match self.data {
            CommandResultData::Fresh => true,
            CommandResultData::Output(ref out) => out.status.success(),
            CommandResultData::Virtual => true,
        }
    }

    pub fn fresh(&self) -> bool {
        match self.data {
            CommandResultData::Fresh => true,
            CommandResultData::Output(_) => false,
            CommandResultData::Virtual => false,
        }
    }

    /// Output (error) state to stderr
    pub fn show(self) {
        eprint!("Step {} [{:?}]: ", self.step_id, self.time);
        self.data.show();
    }
}

#[derive(Debug)]
pub enum CommandResultData {
    Fresh,
    Output(std::process::Output),
    Virtual,
}
impl CommandResultData {
    /// Output (error) state to stderr
    pub fn show(self) {
        match self {
            Self::Output(out) => {
                eprintln!("status = {:?}", out.status.code());
                eprint!("Command stdout:");
                if out.stdout.is_empty() {
                    eprintln!(" (empty)");
                } else {
                    eprintln!("");
                    io::stdout().write_all(&out.stdout).unwrap();
                }
                eprint!("Command stderr:");
                if out.stderr.is_empty() {
                    eprintln!(" (empty)");
                } else {
                    eprintln!("");
                    io::stderr().write_all(&out.stderr).unwrap();
                }
            },
            other => eprintln!("{:?}", other),
        }
    }
}

fn time_modified(p: &Path) -> Option<SystemTime> {
    let mt = p.metadata().ok()?.modified().unwrap();
    if p.is_file() {
        Some(mt)
    } else {
        fs::read_dir(p)
            .unwrap()
            .map(|entry| time_modified(&entry.unwrap().path()))
            .max()
            .unwrap_or(Some(mt))
    }
}

#[derive(Debug)]
pub struct Command {
    step_id: StepId,
    cmd: Vec<String>,
    inputs: Option<Vec<PathBuf>>,
    output: Option<PathBuf>,
    cwd: PathBuf,
    stdout_pass: bool,
    stderr_pass: bool,
    stdout_file: Option<PathBuf>,
    stderr_file: Option<PathBuf>,
    env: HashMap<String, String>,
}
impl Command {
    #[must_use]
    pub fn run(&self) -> CommandResult {
        use std::process::Command;

        let start = Instant::now();

        log::info!("[step {:>4}] Running: {:?}", self.step_id, self.cmd);

        // Check if outputs are already fresh
        if let Some(output) = &self.output {
            if let Some(inputs) = &self.inputs {
                let output_modified = time_modified(output.as_ref());
                let inputs_modified = inputs.iter().map(|p| time_modified(p.as_ref())).max().flatten();

                log::trace!(
                    "[step {:>4}] output [{:?}] ({:?})",
                    self.step_id,
                    output_modified,
                    output
                );
                log::trace!(
                    "[step {:>4}] inputs [{:?}] ({:?})",
                    self.step_id,
                    inputs_modified,
                    inputs
                );

                if let Some(output_m) = output_modified {
                    if let Some(inputs_m) = inputs_modified {
                        if output_m >= inputs_m {
                            log::info!("[step {:>4}] Fresh", self.step_id);
                            return CommandResult {
                                step_id: self.step_id,
                                time: start.elapsed(),
                                data: CommandResultData::Fresh,
                            };
                        }
                    }
                }
            }
        }

        let (program, args) = self.cmd.split_first().expect("Empty command");

        let output = Command::new(program)
            .args(args)
            .envs(&self.env.clone())
            .current_dir(self.cwd.clone())
            .output()
            .expect("failed to execute process");

        if self.stdout_pass {
            io::stdout().write_all(&output.stdout).unwrap();
        }

        if self.stderr_pass {
            io::stderr().write_all(&output.stderr).unwrap();
        }

        if let Some(f) = &self.stdout_file {
            fs::write(f, &output.stdout).unwrap();
        }

        if let Some(f) = &self.stderr_file {
            fs::write(f, &output.stderr).unwrap();
        }

        log::info!("[step {:>4}] Result: {:?}", self.step_id, output.status.code());

        CommandResult {
            step_id: self.step_id,
            time: start.elapsed(),
            data: CommandResultData::Output(output),
        }
    }

    pub fn new(
        step_id: StepId, cmd_obj: &PyAny, exec_config: &ExecConfig, env: HashMap<String, String>,
    ) -> PyResult<Self> {
        let cmd: Vec<String> = cmd_obj
            .getattr("cmd")?
            .iter()?
            .flat_map(|c| {
                let v = c.ok()?;
                if v.is_none() {
                    return None;
                }
                Some(v.to_string())
            })
            .collect();

        let py_inputs = cmd_obj.getattr("inputs")?;
        let inputs: Option<Vec<PathBuf>> = if py_inputs.is_none() {
            None
        } else {
            Some(
                py_inputs
                    .iter()?
                    .flat_map(|c| {
                        let v = c.ok()?;
                        if v.is_none() {
                            return None;
                        }
                        Some(Path::new(&v.to_string()).to_owned())
                    })
                    .collect(),
            )
        };

        let py_output = cmd_obj.getattr("output")?;
        let output: Option<PathBuf> = if py_output.is_none() {
            None
        } else {
            Some(Path::new(&py_output.to_string()).to_owned())
        };

        let py_cwd = cmd_obj.getattr("cwd")?;
        let cwd: PathBuf = if py_cwd.is_none() {
            exec_config.root_dir.clone().unwrap()
        } else {
            Path::new(&py_cwd.to_string()).to_owned()
        };

        let py_stdout_file = cmd_obj.getattr("stdout_file")?;
        let stdout_file: Option<PathBuf> = if py_stdout_file.is_none() {
            None
        } else {
            Some(Path::new(&py_stdout_file.to_string()).to_owned())
        };

        let py_stderr_file = cmd_obj.getattr("stderr_file")?;
        let stderr_file: Option<PathBuf> = if py_stderr_file.is_none() {
            None
        } else {
            Some(Path::new(&py_stderr_file.to_string()).to_owned())
        };

        Ok(Self {
            step_id,
            cmd,
            inputs,
            output,
            cwd,
            stdout_pass: exec_config.transparent,
            stderr_pass: exec_config.transparent,
            stdout_file,
            stderr_file,
            env,
        })
    }
}
