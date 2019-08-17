use std::fs::{metadata, read_dir, DirEntry};
use std::os::unix::prelude::*;
use std::process::{Child, Command};

pub struct Subprocess {
    name: String,
    uid: u32,
    gid: u32,
    dir: String,
    pub process: Option<Child>,
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

    fn new(path: &std::path::PathBuf) -> Result<Subprocess, std::io::Error> {
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        let dir = path.to_str().unwrap().to_string();
        let sub_metadata = metadata(&dir)?;
        let uid = sub_metadata.uid();
        let gid = sub_metadata.gid();

        Ok(Subprocess {
            name,
            uid,
            gid,
            dir,
            process: None,
        })
    }

    pub fn start(&mut self, program: &str) -> Result<(), std::io::Error> {
        if let Some(process) = &mut self.process {
            process.kill()?;
        }
        let program = program.replace("{}", &self.name);
        self.process = Some(self.make_process(&program)?);
        Ok(())
    }
}

pub type Processes = std::collections::HashMap<String, Subprocess>;

pub fn start(options: &crate::options::Options) -> Result<Processes, std::io::Error> {
    let subdirectories = read_dir(&options.root_dir)?;
    let mut processes: Processes = std::collections::HashMap::new();

    for sub in subdirectories {
        let sub: DirEntry = sub?;
        let path = sub.path();
        if !path.is_dir() {
            println!("Ignoring non-directory: {:?}", path);
            continue;
        }
        let mut process = Subprocess::new(&path)?;
        process.start(&options.program)?;
        if let Some(name) = path.file_name() {
            processes.insert(name.to_str().unwrap().to_string(), process);
        }
    }

    Ok(processes)
}
