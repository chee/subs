extern crate tokio;
extern crate regex;

use std::os::unix::prelude::*;
use std::env;
use std::fs;
//use tokio::net::{UnixListener, UnixStream};
use tokio::prelude::*;
use std::process::{Command, Child};
use std::os::unix::net::{UnixListener, UnixStream};
use regex::Regex;

fn run_npm_command (cmd: &str, path: &str, uid: u32, gid: u32) -> Result<Child, std::io::Error> {
	Command::new("npm")
		.arg(cmd)
		.arg("--prefix")
		.arg(path)
		.uid(uid)
		.gid(gid)
		.spawn()
}

struct Subprocess {
	uid: u32,
	gid: u32,
	dir: String,
	process: Option<Child>
}

impl Subprocess {
	fn make_process (path: &str, uid: u32, gid: u32) -> Result<Child, std::io::Error> {
		run_npm_command("start", path, uid, gid)
	}

	fn new (sub: String) -> Result<Subprocess, std::io::Error> {
		let sub_metadata = fs::metadata(&sub)?;
		let uid = sub_metadata.uid();
		let gid = sub_metadata.gid();

		let mut s = Subprocess {
			uid,
			gid,
			dir: sub,
			process: None
		};

		s.start()?;

		Ok(s)
	}

	fn get_file_path (&self, file: &str) -> std::path::PathBuf {
		std::path::Path::new(&format!("{}/{}", &self.dir, file)).to_owned()
	}

	fn start (&mut self) -> Result<(), std::io::Error> {
		fs::remove_file(self.get_file_path("sock")).unwrap_or_default();
		if let Some(process) = &mut self.process {
			process.kill()?;
		}
		self.process = Some(Subprocess::make_process(&self.dir, self.uid, self.gid)?);
		Ok(())
	}
}

struct Msg<'a> {
	command: &'a str,
	sub: &'a str
}

fn parse_msg (msg: &str) -> Option<Msg> {
	let msg_regex = Regex::new(r"(\S+) (\S+)").unwrap();
	msg_regex.captures(msg).map(|capture| {
		let groups = (capture.get(1), capture.get(2));
		match groups {
			(Some(command), Some(name)) =>
				Some(Msg {
					command: command.as_str(),
					sub: name.as_str()
				}),
			_ => None
		}
	})?
}

fn main() -> Result<(), std::io::Error> {
	let args: Vec<String> = env::args().collect();
	let subdirectories = fs::read_dir("./test")?;

	let mut processes = std::collections::HashMap::new();

	for sub in subdirectories {
		let sub = sub?;
		let path = sub.path();
		let process = Subprocess::new(path.to_str().unwrap().to_string())?;
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
							sub.start()?;
						} else {
							println!("recieved unusual command: {}", msg.command)
						}
					},
					None => {
						println!("recieved unusual person: {}", msg.sub)
					}
				}
			},
			None => {
				println!(
					"recieved unusual message. message should be <command> <sub>. got: {}",
					string
				)
			}
		}
	}
	Ok(())
}
