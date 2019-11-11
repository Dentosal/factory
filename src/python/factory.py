from typing import Any, List, Set, Dict, Optional, Union, Callable, Mapping

from dataclasses import dataclass, field
from pathlib import Path

EnvDict = Dict[str, Union[Path, str, None]]

@dataclass(frozen=True, eq=True)
class Cmd:
    """A command to be executed."""

    cmd: List[Union[Path, str, None]]  # TODO: lists, flattening?
    inputs: Optional[Set[Path]] = None  # TODO: lists, flattening?
    output: Optional[Path] = None
    cwd: Optional[Path] = None
    env: EnvDict = field(default_factory=dict)
    stdout_file: Optional[Path] = None
    stderr_file: Optional[Path] = None

    def __hash__(self):
        return hash(repr(self))


@dataclass(frozen=True, eq=True)
class Expr:
    """
    Python expression to be evaluated on this step.
    Result is stored to a named variable.
    """

    name: str
    expr: Any

    def __hash__(self):
        return hash(repr(self))


@dataclass(frozen=True, eq=True)
class Assert:
    """Python expression that must return true."""

    expr: bool
    error_msg: Optional[str]

    def __hash__(self):
        return hash(repr(self))


StepCmd = Union[Cmd, Expr, Assert]

@dataclass(frozen=True, eq=True)
class Step:
    cmd: Union[StepCmd, Callable[[Mapping[str, Any]], StepCmd]]
    requires: Set[Callable[[Path, Mapping[str, Any]], Cmd]] = field(default_factory=set)
    env: EnvDict = field(default_factory=dict)
    freshvar: Union[None, str, Callable[[EnvDict], str]] = None
    condition: Union[bool, Callable[[EnvDict], bool]] = True
    note: Union[None, str] = None

    def __hash__(self):
        return hash(repr(self))
