from typing import List

from factory import Step, Cmd
from pathlib import Path


def cmd_cargo_build(pdir: Path, target_file: str):
    return Cmd(
        inputs={pdir / "src/", pdir / "Cargo.toml"},
        output=pdir / "target" / "release" / target_file,
        cmd=["cargo", "build", "--release"],
        cwd=pdir,
    )


def cmd_compile_c(src: Path, dst: Path):
    return Cmd(inputs={src}, output=dst, cmd=["gcc", "-c", src, "-o", dst])


def cmd_link(inputs: List[Path], output: Path):
    # Inputs must be a list, as order of files matters when linking
    return Cmd(inputs=inputs, output=output, cmd=["gcc", "-o", output] + inputs)


def init_fs(root_dir, cfg):
    (root_dir / "target").mkdir(exist_ok=True)


def step_build_rust(root_dir):
    return Step(cmd=cmd_cargo_build(root_dir, "libexample.so"))


def step_build_c(root_dir):
    return Step(
        cmd=cmd_compile_c(root_dir / "src/example.c", root_dir / "target/example.o")
    )


def step_link(root_dir):
    return Step(
        requires={step_build_rust, step_build_c},
        cmd=cmd_link(
            [
                root_dir / "target/example.o",
                root_dir / "target/release/libexample.so",
            ],
            "target/example",
        ),
    )
