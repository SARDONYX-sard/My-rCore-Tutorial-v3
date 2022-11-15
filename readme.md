# rCore-Tutorial-Book-v3 3.6.0-alpha.1

RISC-V OS written in Rust

- Architecture: RISC-V RV64

## Feature

- [x] Batch Processing System

- [x] Cooperative Multitask

- [x] Timesharing Multitask

- [x] Page table

- [x] Process

- [x] File System

- [x] InterProcess Communication and I/O Redirection

- [x] Concurrency

- [ ] I/O Device Management

## How to fix `Cannot open DISPLAY:0` on WSL of Windows11

Execute the following command.

```powershell
wsl --shutdown # shutdown docker, other WSL
```

And more...

```powershell
cd <YOUR_PROJECT_PATH>

wsl # enter default WSL

# Why not use `\wsl$╱Ubuntu╱home╱`? (For performance)
# - https://docs.docker.com/desktop/windows/wsl/#best-practices
code . # open project with vscode

# -> GUI Reopen container
# - https://code.visualstudio.com/docs/devcontainers/containers#_open-a-wsl-2-folder-in-a-container-on-windows
```

![sample](https://user-images.githubusercontent.com/68905624/189535647-8db48562-5cf9-4225-a2f3-42174ab3e995.gif)
