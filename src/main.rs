extern crate futures;
extern crate getopts;
extern crate notify;
extern crate regex;
extern crate tokio;

use futures::{IntoFuture, Stream};
use notify::{watcher, RecursiveMode, Watcher};
use regex::Regex;
use std::fs;
use std::io::Read;
use std::os::unix::prelude::*;
use std::process::{Child, Command};
use std::time::Duration;
use tokio::net::UnixListener;

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

fn print_usage(dollar0: &str, opts: getopts::Options) {
    let brief = format!("Usage: {} [options] PROGRAM [root_dir]", dollar0);
    println!("{}", opts.usage(&brief));
    println!("PROGRAM will be run in parallel in every subdirectory (SUB), as SUB's owner.");
    println!("A placeholder \"{{}}\" is available to PROGRAM, it will be replaced with SUB.");
}

enum Manager {
    Watch,
    Socket,
    None,
}

struct Options {
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

fn get_watcher(root_dir: String) -> futures::sync::mpsc::UnboundedReceiver<notify::DebouncedEvent> {
    let (ss, sr) = std::sync::mpsc::channel();
    let (snd, rcv) = futures::sync::mpsc::unbounded();
    std::thread::spawn(move || {
        let mut watcher = watcher(ss, Duration::from_secs(1)).unwrap();
        watcher.watch(&root_dir, RecursiveMode::Recursive).unwrap();
        loop {
            match sr.recv() {
                Ok(event) => {
                    snd.unbounded_send(event).unwrap();
                }
                Err(error) => {
                    panic!("yeet! {:?}", error);
                }
            }
        }
    });
    rcv
}

fn main() -> Result<(), std::io::Error> {
    let options = get_options();
    let subdirectories = fs::read_dir(&options.root_dir)?;

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
        process.start(&options.program.replace("{}", sub_name))?;
        if let Some(name) = path.file_name() {
            processes.insert(name.to_str().unwrap().to_string(), process);
        }
    }

    match options.management {
        Manager::Watch => {
            fn handle_change(
                pathbuf: std::path::PathBuf,
                ignore: Option<&Regex>,
            ) -> Option<std::path::PathBuf> {
                let did_change = match ignore {
                    Some(regex) => !regex.is_match(pathbuf.to_str().unwrap()),
                    None => true,
                };

                if did_change {
                    Some(pathbuf)
                } else {
                    None
                }
            }

            // pulling this off so it can be owned by tokio
            let watch_ignore = options.watch_ignore.clone();
            let root_dir = options.root_dir.clone();

            tokio::run(
                get_watcher(options.root_dir.to_string()).for_each(move |event| {
                    let changed_path = match event {
                        notify::DebouncedEvent::NoticeWrite(path) => {
                            handle_change(path, watch_ignore.as_ref())
                        }
                        notify::DebouncedEvent::Create(path) => {
                            handle_change(path, watch_ignore.as_ref())
                        }
                        notify::DebouncedEvent::Write(path) => {
                            handle_change(path, watch_ignore.as_ref())
                        }
                        notify::DebouncedEvent::Chmod(path) => {
                            handle_change(path, watch_ignore.as_ref())
                        }
                        // TODO: figure out how to handle remove,rename
                        // notify::DebouncedEvent::Remove(path) => {
                        //     handle_change(path, watch_ignore.as_ref())
                        // }
                        // notify::DebouncedEvent::Rename(path, _path) => {
                        //     handle_change(path, watch_ignore.as_ref())
                        // }
                        // notify::DebouncedEvent::NoticeRemove(path) => {
                        //     handle_change(path, watch_ignore.as_ref())
                        // }
                        _ => None,
                    };
                    match changed_path {
                        Some(path) => {
                            // this whole bit is a big ol' yeet
                            let canonical_path = path.canonicalize().unwrap();
                            let root_dir_path = std::path::Path::new(&root_dir).canonicalize();

                            let changed_file = canonical_path.strip_prefix(root_dir_path.unwrap());
                            let changed_sub = changed_file
                                .unwrap()
                                .components()
                                .next()
                                .unwrap()
                                .as_os_str()
                                .to_str()
                                .unwrap();
                            let sub = processes.get_mut(changed_sub);
                            match sub {
                                Some(sub) => sub
                                    .start(&options.program)
                                    .expect("tried to restart and failed"),
                                None => println!(
                                    "received news about {}, but i'm not following them?",
                                    changed_sub
                                ),
                            }
                            Ok(())
                        }
                        None => Ok(()),
                    }
                }),
            )
        }
        Manager::Socket => {
            fs::remove_file(&options.sock_path).unwrap_or_default();
            let sock = UnixListener::bind(&options.sock_path)?;
            let sock_stream = sock.incoming().for_each(|mut stream| {
                let mut buf = vec![];
                stream.read_to_end(&mut buf)?;
                let string = String::from_utf8(buf).unwrap();
                let msg = parse_msg(string.as_str());
                match msg {
                    Some(msg) => {
                        let sub = processes.get_mut(msg.sub);
                        match sub {
                            Some(sub) => {
                                if msg.command == "restart" {
                                    sub.start(&options.program)?;
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
                Ok(())
            });
            tokio::run(sock_stream.into_future().and_then(|| {}));
            fs::remove_file(&options.sock_path).unwrap_or_default();
        }
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
