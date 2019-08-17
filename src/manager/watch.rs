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
                        let canonical_path = path.canonicalize().unwrap();
                        let root_dir_path = std::path::Path::new(&options.root_dir).canonicalize();

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
