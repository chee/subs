extern crate getopts;
extern crate notify;
extern crate regex;

mod manager;
mod options;
mod subprocesses;

use manager::Manager;

fn main() -> Result<(), std::io::Error> {
    let options = options::get_options();

    let processes = subprocesses::start(&options)?;

    match options.management {
        Manager::Watch => manager::watch::manage(processes, options),
        Manager::Socket => manager::socket::manage(processes, options)?,
        Manager::None => manager::none::manage(processes, options),
    }

    Ok(())
}
