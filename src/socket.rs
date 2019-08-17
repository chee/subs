use regex::Regex;
use std::fs;
use std::io::Read;
use std::os::unix::net::UnixListener;

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

pub fn manage(
    mut processes: super::Processes,
    options: super::Options,
) -> Result<(), std::io::Error> {
    let sock_path = options.sock_path.clone();
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
    }
    fs::remove_file(&sock_path).unwrap_or_default();
    Ok(())
}
