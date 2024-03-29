# Factory - a flexible parallel build system

* Configuration written in Python
* Runs build steps simultaneously
* Skips unnecessary commands

![example image](images/rust_os.png)

## Usage

See [`examples/call_rust_from_c`](examples/call_rust_from_c) for a complete example.

## Building

Latest Rust nightly is required.

Python 3.7 or newer, including `libpython3.X` and `libpython3.X-dev` on Linux.

```bash
PYTHON_SYS_EXECUTABLE=python3.7 cargo build
```
