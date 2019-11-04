#![deny(unused_must_use)]

use factory;

use pyo3::prelude::*;
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    pretty_env_logger::init();

    let args: Vec<String> = env::args().skip(1).collect();

    let init_dir = args
        .get(0)
        .map(|s| Path::new(s).to_owned())
        .unwrap_or_else(|| env::current_dir().expect("Current directory not accessible"));

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

    let (steps, cfg_dict, toml_config) = factory::config::read(py, &init_dir)
        .map_err(|e| {
            e.print_and_set_sys_last_vars(py);
        })
        .unwrap();

    let stats = factory::run(py, &steps, cfg_dict, &toml_config, py_factory)
        .map_err(|e| {
            e.print_and_set_sys_last_vars(py);
        })
        .unwrap();

    if let Some(path) = toml_config.stats_dot {
        fs::write(path, factory::depgraph::to_dot(&steps, stats).as_bytes())
            .expect("Unable to write `stats_dot` file");
    }
}
