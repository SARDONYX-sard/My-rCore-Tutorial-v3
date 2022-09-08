# tmux simple manual

## basic commands

- tmux startup (session start).

```bash
tmux
```

- tmux session list.

```bash
tmux ls
```

- restart tmux (resume session).

```bash
tmux a [-t <target session name>]]
# e.g. tmux a -t 0
```

- exit tmux (end session)

```bash
tmux kill-session [-t <target session name>]
```

## shortcut keys

- prefix: `ctrl+b`(default)

See more: <https://tmuxcheatsheet.com>

|      command      | role                                      |
| :---------------: | :---------------------------------------- |
|    prefix + ?     | key bindings list                         |
|    prefix + s     | List of sessions                          |
|    prefix + c     | Create/Add New Window                     |
|    prefix + w     | List windows                              |
|    prefix + &     | Destroy window                            |
|    prefix + n     | Move to next window                       |
|    prefix + p     | Move to previous window                   |
|  prefix + &#124;  | Split pane left/right                     |
|    prefix + %     | split pane vertically                     |
|    prefix + h     | Move to left pane                         |
|    prefix + j     | move to bottom pane                       |
|    prefix + k     | Move to top pane                          |
|    prefix + l     | move to right pane                        |
|    prefix + H     | resize pane to the left                   |
|    prefix + J     | resize pane down                          |
|    prefix + K     | resize pane up                            |
|    prefix + L     | resize pane to the right                  |
|    prefix + x     | Destroy pane                              |
| prefix + Ctrl + o | change pane layout                        |
| prefix + Ctrl + o | replace pane                              |
|    prefix + {     | replace pane(up)                          |
| prefix + Ctrl + o | replace pane                              |
| prefix + Ctrl + o | replace pane                              |
|    prefix + [     | copy                                      |
|    prefix + [     | start copy mode (use cursor keys to move) |
|    prefix + v     | Determine copy start position (vi mode)   |
|    prefix + y     | Determine copy end position (vi mode)     |
|    prefix + v     | Determine copy end position (vi mode)     |
| prefix + Ctrl + p | Paste the copied content                  |
