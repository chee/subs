extern crate getopts;
extern crate regex;

use getopts::Options;
use regex::Regex;
use std::fs;
use std::io::prelude::*;
use std::os::unix::net::UnixListener;
use std::os::unix::prelude::*;
use std::process::{Child, Command};

struct Subprocess {
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

struct Msg<'a> {
    command: &'a str,
    sub: &'a str,
}

fn parse_msg(msg: &str) -> Option<Msg> {
    let msg_regex = Regex::new(r"(\S+) (\S+)").unwrap();
    msg_regex.captures(msg).map(|capture| {
        let groups = (capture.get(1), capture.get(2));
        match groups {
            (Some(command), Some(name)) => Some(Msg {
                command: command.as_str(),
                sub: name.as_str(),
            }),
            _ => None,
        }
    })?
}

fn print_usage(dollar0: &str, opts: Options) {
    let brief = format!("Usage: {} [options] PROGRAM [root_dir]", dollar0);
    println!("{}", opts.usage(&brief));
    println!("PROGRAM will be run in parallel in every subdirectory (SUB), as SUB's owner.");
    println!("A placeholder '{{}}' is available to PROGRAM, it will be replaced with SUB.");
    println!("A socket will be created.");
    println!("Sending the socket a message like 'restart SUB' will restart that SUB's process.")
}

fn main() -> Result<(), std::io::Error> {
    let args: Vec<String> = std::env::args().collect();
    let dollar0 = &args[0];
    let mut options = Options::new();
    options.optopt(
        "s",
        "socket",
        "set the socket path. [default: subsocket]",
        "NAME",
    );
    options.optflag("S", "no-socket", "do not create a socket");
    options.optflag("h", "help", "get help");

    let matches = match options.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => panic!(f.to_string()),
    };

    let sock_path = matches.opt_str("s").unwrap_or("subsocket".to_string());

    let program = matches.free.get(0);
    let default_root_dir = ".".to_string();
    let root_dir = matches.free.get(1).unwrap_or(&default_root_dir);

    let should_show_help = matches.opt_present("h") || program.is_none();

    if should_show_help {
        print_usage(&dollar0, options);
        std::process::exit(1);
    }

    let program = program.unwrap();

    let subdirectories = fs::read_dir(root_dir)?;

    let mut processes = std::collections::HashMap::new();

    for sub in subdirectories {
        let sub: fs::DirEntry = sub?;
        let path = sub.path();
        if !path.is_dir() {
            println!("Ignoring non-directory: {:?}", path);
            continue;
        }
        let sub_name = path.file_name().unwrap().to_str().unwrap();
        let mut process = Subprocess::new(path.to_str().unwrap().to_string())?;
        process.start(&program.replace("{}", sub_name))?;
        if let Some(name) = path.file_name() {
            processes.insert(name.to_str().unwrap().to_string(), process);
        }
    }

    if matches.opt_present("S") {
        for sub in processes.values_mut() {
            if let Some(process) = &mut sub.process {
                process.wait().unwrap();
            }
        }
    } else {
        fs::remove_file(&sock_path).unwrap_or_default();
        let sock = UnixListener::bind(&sock_path)?;
        for stream in sock.incoming() {
            let mut buf = vec![];
            stream?.read_to_end(&mut buf)?;
            let string = String::from_utf8(buf).unwrap();
            let msg = parse_msg(string.as_str());
            match msg {
                Some(msg) => {
                    let sub = processes.get_mut(msg.sub);
                    match sub {
                        Some(sub) => {
                            if msg.command == "restart" {
                                sub.start(program)?;
                            } else {
                                println!("recieved unusual command: {}", msg.command)
                            }
                        }
                        None => println!("recieved unusual person: {}", msg.sub),
                    }
                }
                None => println!(
                    "recieved unusual message. message should be <command> <sub>. got: {}",
                    string
                ),
            }
        }
        fs::remove_file(&sock_path).unwrap_or_default();
    }

    Ok(())
}
