from pathlib import Path
from factory import Cmd


def touch_file(path: Path) -> Cmd:
    return Cmd(cmd=["touch", path])
