use crate::subprocesses::Processes;
use notify::{watcher, RecursiveMode, Watcher};
use regex::Regex;
use std::time::Duration;

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

pub fn manage(mut processes: Processes, options: crate::options::Options) {
    let (sender, receiver) = std::sync::mpsc::channel();

    let mut watcher = watcher(sender, Duration::from_secs(2)).unwrap();
    watcher
        .watch(&options.root_dir, RecursiveMode::Recursive)
        .unwrap();

    loop {
        match receiver.recv() {
            Ok(event) => {
                let changed_path = match event {
                    notify::DebouncedEvent::NoticeWrite(path) => {
                        handle_change(path, options.watch_ignore.as_ref())
                    }
                    notify::DebouncedEvent::Create(path) => {
                        handle_change(path, options.watch_ignore.as_ref())
                    }
                    notify::DebouncedEvent::Write(path) => {
                        handle_change(path, options.watch_ignore.as_ref())
                    }
                    notify::DebouncedEvent::Chmod(path) => {
                        handle_change(path, options.watch_ignore.as_ref())
                    }
                    // TODO: figure out how to handle remove,rename
                    // notify::DebouncedEvent::Remove(path) => {}
                    // notify::DebouncedEvent::Rename(path, _path) => {}
                    // notify::DebouncedEvent::NoticeRemove(path) => {}
                    _ => None,
                };
                match changed_path {
                    Some(path) => {
                        // this whole bit is a big ol' yeet
                        let canonical_path = path.canonicalize();
                        if canonical_path.is_err() {
                            return;
                        }
                        let canonical_path = canonical_path.unwrap();
                        let root_dir_path = std::path::Path::new(&options.root_dir).canonicalize();
                        if root_dir_path.is_err() {
                            return;
                        }
                        let root_dir_path = root_dir_path.unwrap();
                        let changed_file = canonical_path.strip_prefix(&root_dir_path);
                        if let Ok(file) = changed_file {
                            if let Some(first) = file.components().next() {
                                if let Some(name) = first.as_os_str().to_str() {
                                    let sub = processes.get_mut(name);
                                    match sub {
                                        Some(sub) => sub
                                            .start(&options.program)
                                            .expect("tried to restart and failed"),
                                        None => println!(
                                            "received news about {}, but i'm not following them?",
                                            name
                                        ),
                                    }
                                } else {
                                    println!("{:?} is not valid utf-8", first)
                                }
                            } else {
                                println!("{:?} was not under {:?}. ignoring", file, root_dir_path)
                            }
                        }
                    }
                    None => (),
                }
            }
            Err(error) => {
                println!("yeet! {:?}", error);
            }
        }
    }
}
