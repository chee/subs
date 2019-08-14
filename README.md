# subsock

Run a command simultaneously in every subdirectory, and manage with a socket.

```
Usage: target/debug/subsock [options] PROGRAM [root_dir]

Options:
    -s, --socket NAME   set the socket path. [default: subsocket]
    -S, --no-socket     do not create a socket
    -h, --help          get help

PROGRAM will be run in parallel in every subdirectory (SUB), as SUB's owner.
A placeholder '{}' is available to PROGRAM, it will be replaced with SUB.
A socket will be created.
Sending the socket a message like 'restart SUB' will restart that SUB's process.
```

keep your subs happy
