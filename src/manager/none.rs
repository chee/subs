use crate::subprocesses::Processes;

pub fn manage(mut processes: Processes, _options: crate::options::Options) {
    for sub in processes.values_mut() {
        if let Some(process) = &mut sub.process {
            process.wait().unwrap();
        }
    }
}
