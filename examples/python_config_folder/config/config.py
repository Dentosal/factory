from typing import Tuple, List, Set, Optional

import random
from pathlib import Path

from factory import *

from .commands import touch_file


def step_touch_first(root_dir) -> Step:
    return Step(cmd=touch_file(root_dir / "example_dir" / "test.txt"))


def step_all(root_dir) -> Step:
    return Step(requires={step_touch_first}, cmd=Assert(expr=True, error_msg=""))


def init(cfg):
    cfg["RANDOM_SEED"] = random.random()


def init_fs(root_dir, cfg):
    (root_dir / "example_dir").mkdir(exist_ok=True)
