use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use structopt::{self, StructOpt};

#[derive(Debug, Deserialize, StructOpt, Default)]
#[structopt(rename_all = "kebab-case")]
#[serde(default, deny_unknown_fields)]
pub struct ExecConfig {
    /// Root directory override.
    /// Current directory is used if this is not set.
    #[structopt(short = "-d", long, parse(from_os_str))]
    pub root_dir: Option<PathBuf>,

    /// Python config file.
    #[structopt(short, long, parse(from_os_str))]
    pub config: Option<PathBuf>,

    /// Number of threads, logical CPU core count is used by default
    #[structopt(short = "-p", long)]
    pub threads: Option<usize>,

    /// Run all commands, even if the output file is fresh
    #[structopt(short, long)]
    pub refresh: bool,

    /// Disable progress bar and other unnecessary output
    #[structopt(short, long)]
    pub quiet: bool,

    /// Display process stdout and stderr (after completion)
    #[structopt(short, long)]
    pub transparent: bool,

    /// Output file for graphviz dot file containing build plan
    #[structopt(short, long, parse(from_os_str))]
    pub plan_dot: Option<PathBuf>,

    /// Output file for graphviz dot file containing build statistic
    #[structopt(short, long, parse(from_os_str))]
    pub stats_dot: Option<PathBuf>,

    /// Target to execute
    pub target: Option<String>,
}
impl ExecConfig {
    pub fn threads(&self) -> usize {
        self.threads.unwrap_or_else(num_cpus::get)
    }

    pub fn load_toml(dir_path: &Path) -> Self {
        let contents = fs::read(dir_path.join("Factory.toml")).expect("Factory.toml missing");
        let mut s: Self = toml::from_slice(&contents).expect("Invalid toml");

        s.root_dir = Some(if let Some(cfg_rd) = s.root_dir {
            cfg_rd.canonicalize().unwrap()
        } else {
            dir_path.canonicalize().unwrap()
        });

        // TODO: Validate configuration options

        s
    }

    pub fn merge(self, other: Self) -> Self {
        Self {
            root_dir: self.root_dir.or(other.root_dir),
            config: self.config.or(other.config),
            threads: self.threads.or(other.threads),
            refresh: self.refresh || other.refresh,
            quiet: self.quiet || other.quiet,
            transparent: self.transparent || other.transparent,
            plan_dot: self.plan_dot.or(other.plan_dot),
            stats_dot: self.stats_dot.or(other.stats_dot),
            target: self.target.or(other.target),
        }
    }

    pub fn root_dir(&self) -> PathBuf {
        self.root_dir.clone().unwrap()
    }

    pub fn python(&self) -> PathBuf {
        self.config.clone().unwrap()
    }
}
