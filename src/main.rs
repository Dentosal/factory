#![deny(unused_must_use)]

use factory;

use pyo3::prelude::*;
use std::env;
use std::fs;
use std::path::PathBuf;

use structopt::{self, StructOpt};

#[derive(Debug, StructOpt)]
#[structopt(author, about)]
#[structopt(rename_all = "kebab-case")]
struct Args {
    /// Directory to look for Factory.toml
    #[structopt(short, long, parse(from_os_str))]
    directory: Option<PathBuf>,

    #[structopt(flatten)]
    exec: factory::ExecConfig,
}

#[paw::main]
fn main(args: Args) {
    pretty_env_logger::init();
    let code = inner_main(args);
    std::process::exit(code);
}

fn inner_main(mut args: Args) -> i32 {
    let init_dir = args
        .directory
        .unwrap_or_else(|| env::current_dir().expect("Current directory not accessible"));

    args.exec = {
        let toml_config = factory::ExecConfig::load_toml(&init_dir);
        args.exec.merge(toml_config)
    };

    let gil = Python::acquire_gil();
    let py = gil.python();

    factory::check_python(py)
        .map_err(|e| {
            e.print_and_set_sys_last_vars(py);
        })
        .unwrap();

    // Import class definitions
    let py_factory = factory::get_py_factory(py)
        .map_err(|e| {
            e.print_and_set_sys_last_vars(py);
        })
        .unwrap();

    let (steps, cfg_dict) = factory::config::read(py, &args.exec)
        .map_err(|e| {
            e.print_and_set_sys_last_vars(py);
        })
        .unwrap();

    let target_name = args.exec.target.clone().expect("No target name given");

    match factory::run(py, &steps, &target_name, cfg_dict, &args.exec, py_factory) {
        Ok(stats) => {
            if let Some(path) = args.exec.stats_dot {
                fs::write(path, factory::depgraph::to_dot(&steps, stats).as_bytes())
                    .expect("Unable to write `stats_dot` file");
            }
            0
        },
        Err(err) => {
            err.show(py);
            1
        },
    }
}
