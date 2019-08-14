extern crate regex;

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

fn main() -> Result<(), std::io::Error> {
    let args: Vec<String> = std::env::args().collect();
    let program = if let Some(program) = args.get(1) {
        program
    } else {
        panic!("Usage: {} <program> [root_dir]");
    };

    let default_root_dir = ".".to_string();
    let root_dir = args.get(2).unwrap_or(&default_root_dir);

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

    let sock_path = "./subsocket";
    fs::remove_file(sock_path).unwrap_or_default();
    let sock = UnixListener::bind(sock_path)?;
    for stream in sock.incoming() {
        let mut buf = vec![];
        let count = stream?.read_to_end(&mut buf)?;
        println!("count: {:?}", count);
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
    Ok(())
}
