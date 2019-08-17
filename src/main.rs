extern crate getopts;
extern crate notify;
extern crate regex;

use regex::Regex;
use std::fs;
use std::os::unix::prelude::*;
use std::process::{Child, Command};

mod socket;
mod watch;

pub struct Subprocess {
    uid: u32,
    gid: u32,
    dir: String,
    process: Option<Child>,
}

impl Subprocess {
    fn make_process(&self, program: &str) -> Result<Child, std::io::Error> {
        Command::new("sh")
            .arg("-c")
            .arg(program)
            .current_dir(&self.dir)
            .uid(self.uid)
            .gid(self.gid)
            .spawn()
    }

    fn new(sub: String) -> Result<Subprocess, std::io::Error> {
        let sub_metadata = fs::metadata(&sub)?;
        let uid = sub_metadata.uid();
        let gid = sub_metadata.gid();

        Ok(Subprocess {
            uid,
            gid,
            dir: sub,
            process: None,
        })
    }

    fn start(&mut self, program: &str) -> Result<(), std::io::Error> {
        if let Some(process) = &mut self.process {
            process.kill()?;
        }
        self.process = Some(self.make_process(program)?);
        Ok(())
    }
}

fn print_usage(dollar0: &str, opts: getopts::Options) {
    let brief = format!("Usage: {} [options] PROGRAM [root_dir]", dollar0);
    println!("{}", opts.usage(&brief));
    println!("PROGRAM will be run in parallel in every subdirectory (SUB), as SUB's owner.");
    println!("A placeholder \"{{}}\" is available to PROGRAM, it will be replaced with SUB.");
}

pub enum Manager {
    Watch,
    Socket,
    None,
}

pub struct Options {
    sock_path: String,
    program: String,
    root_dir: String,
    watch_ignore: Option<Regex>,
    management: Manager,
}

fn get_options() -> Options {
    let args: Vec<String> = std::env::args().collect();
    let mut opts = getopts::Options::new();

    opts.optopt(
        "t",
        "type",
        "set the management type [choices: watch, socket, none] [default: none]",
        "TYPE",
    );

    opts.optopt(
        "s",
        "socket",
        "set the socket path. sending the socket a message like \"restart xxx\" will restart the process running in the directory \"xxx\". [default: ./subsocket]",
        "NAME",
    );

    opts.optopt(
        "i",
        "watch-ignore",
        "pattern to ignore when watching (matches whole path)",
        "PATTERN",
    );

    opts.optflag("h", "help", "get help");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => panic!(f.to_string()),
    };

    let program = matches.free.get(0);

    let management = match matches.opt_str("t") {
        Some(string) => match string.as_ref() {
            "watch" => Some(Manager::Watch),
            "socket" | "sock" => Some(Manager::Socket),
            "none" => Some(Manager::None),
            _ => None,
        },
        None => Some(Manager::None),
    };

    let should_show_help = matches.opt_present("h") || program.is_none() || management.is_none();

    if should_show_help {
        print_usage(&args[0], opts);
        std::process::exit(1);
    }

    let default_root_dir = ".".to_string();
    let root_dir = matches.free.get(1).unwrap_or(&default_root_dir).to_string();

    let sock_path = matches
        .opt_str("s")
        .unwrap_or_else(|| "subsocket".to_string());

    let watch_ignore = match matches.opt_str("i") {
        Some(string) => Some(Regex::new(&string).expect("-i was not a valid regex")),
        None => None,
    };

    Options {
        program: program.unwrap().to_string(),
        watch_ignore,
        sock_path,
        root_dir,
        management: management.unwrap(),
    }
}

pub type Processes = std::collections::HashMap<String, Subprocess>;

fn main() -> Result<(), std::io::Error> {
    let options = get_options();
    let subdirectories = fs::read_dir(&options.root_dir)?;

    let mut processes: Processes = std::collections::HashMap::new();

    for sub in subdirectories {
        let sub: fs::DirEntry = sub?;
        let path = sub.path();
        if !path.is_dir() {
            println!("Ignoring non-directory: {:?}", path);
            continue;
        }
        let sub_name = path.file_name().unwrap().to_str().unwrap();
        let mut process = Subprocess::new(path.to_str().unwrap().to_string())?;
        process.start(&options.program.replace("{}", sub_name))?;
        if let Some(name) = path.file_name() {
            processes.insert(name.to_str().unwrap().to_string(), process);
        }
    }

    match options.management {
        Manager::Watch => watch::manage(processes, options),
        Manager::Socket => socket::manage(processes, options)?,
        Manager::None => {
            for sub in processes.values_mut() {
                if let Some(process) = &mut sub.process {
                    process.wait().unwrap();
                }
            }
        }
    }

    Ok(())
}
