# subs

Run a command simultaneously in every subdirectory, with optional process management.

```
Usage: target/debug/subs [options] PROGRAM [root_dir]

Options:
    -t, --type TYPE     set the management type [choices: watch, socket, none]
                        [default: none]
    -s, --socket NAME   set the socket path. sending the socket a message like
                        "restart xxx" will restart the process running in the
                        directory "xxx". [default: ./subsocket]
    -i, --watch-ignore PATTERN
                        pattern to ignore when watching (matches whole path)
    -h, --help          get help

PROGRAM will be run in parallel in every subdirectory (SUB), as SUB's owner.
A placeholder "{}" is available to PROGRAM, it will be replaced with SUB.
```

keep your subs happy
