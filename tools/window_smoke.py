#!/usr/bin/env python3
"""Test the actual window ONLY inside an owned, private Xvfb server.

Never connects to the caller's desktop. Requires Xvfb, xdotool, and xprop.
All X clients receive an explicit private DISPLAY and XAUTHORITY. Input events
are additionally addressed to the game window after checking its process ID.
"""
import argparse
import json
import os
from pathlib import Path
import re
import select
import shutil
import subprocess
import tempfile
import time


def exercise(args, environment, root):
    def xdo(*arguments, check=True):
        return subprocess.run([args.xdotool, *map(str, arguments)], env=environment,
                              check=check, capture_output=True, text=True, timeout=5)
    installed = root / "app/bin/river-raid"
    installed.parent.mkdir(parents=True)
    shutil.copy2(args.binary.resolve(), installed)
    installed.chmod(0o555)
    installed.parent.chmod(0o555)
    installed.parent.parent.chmod(0o555)
    environment = dict(environment)
    for name in list(environment):
        if name.startswith("BASALT_"):
            environment.pop(name)
    audio_options = ["--audio-device", args.audio_device] if args.audio_device else ["--no-audio"]
    process = subprocess.Popen([str(installed), *audio_options], env=environment, cwd="/",
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    try:
        window = None
        for _ in range(100):
            # --all is critical: xdotool otherwise ORs PID/name predicates.
            found = xdo("search", "--all", "--pid", process.pid,
                        "--name", "^River Raid", check=False)
            if found.returncode == 0 and found.stdout.strip():
                window = found.stdout.splitlines()[0]
                break
            if process.poll() is not None:
                raise RuntimeError(f"game exited before mapping: {process.communicate()}")
            time.sleep(0.05)
        if window is None:
            raise RuntimeError("game did not map its own window")
        def prop(*names):
            return subprocess.check_output([args.xprop, "-id", window, *names],
                                           env=environment, text=True, timeout=5)
        props = prop("WM_CLASS", "_NET_WM_PID", "_NET_WM_WINDOW_OPACITY", "_NET_WM_WINDOW_TYPE")
        if f"_NET_WM_PID(CARDINAL) = {process.pid}" not in props:
            raise RuntimeError("window does not belong to the spawned game process")
        if '"river-raid", "RiverRaid"' not in props:
            raise RuntimeError(f"missing application class: {props}")
        if "4294967295" not in props or "_NET_WM_WINDOW_TYPE_NORMAL" not in props:
            raise RuntimeError(f"missing opaque normal window metadata: {props}")
        xdo("windowfocus", "--sync", window)
        def key(name):
            xdo("keydown", "--window", window, name)
            time.sleep(0.08)
            # Q closes the owned window on key-down; key-up then has no target.
            xdo("keyup", "--window", window, name, check=name != "q" and process.poll() is None)
            time.sleep(0.08)
        key("Return")
        for width, height in [(800, 600), (1100, 650), (440, 700)]:
            xdo("windowsize", "--sync", window, width, height)
            time.sleep(0.12)
            opaque = prop("_NET_WM_OPAQUE_REGION")
            if [int(n) for n in re.findall(r"\d+", opaque)] != [0, 0, width, height]:
                raise RuntimeError(f"opaque region did not follow resize: {opaque}")
        key("F1")
        if "Controls" not in xdo("getwindowname", window).stdout:
            raise RuntimeError("F1 did not open the controls overlay")
        key("Return")
        key("Escape")
        if "Paused" not in xdo("getwindowname", window).stdout:
            raise RuntimeError("Escape did not pause the game")
        key("q")
        stdout, stderr = process.communicate(timeout=5)
        if process.returncode != 0:
            raise RuntimeError(f"normal keyboard quit failed ({process.returncode}): {stderr}")
        if args.audio_device and "Audio unavailable" in stderr:
            raise RuntimeError(f"ALSA device did not open: {stderr}")
        if not (root / "data/save/records.json").is_file():
            raise RuntimeError("normal quit did not save inside the install root")
        return {"display": "owned private Xvfb", "window_class": "river-raid/RiverRaid",
                "opacity": 4294967295, "resize": [[800, 600], [1100, 650], [440, 700]],
                "keyboard_quit_exit": 0, "read_only_app": True, "install_relative_fallback": True, "save": "data/save/records.json",
                "alsa_device": args.audio_device,
                "stdout": stdout.strip(), "stderr": stderr.strip()}
    finally:
        installed.parent.chmod(0o755)
        installed.parent.parent.chmod(0o755)
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/release/river-raid"))
    parser.add_argument("--xvfb", default="Xvfb")
    parser.add_argument("--xdotool", default="xdotool")
    parser.add_argument("--xprop", default="xprop")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--audio-device", help="Exercise host ALSA with this device (e.g. null); default disables audio")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="river-raid-private-display-") as temporary:
        root = Path(temporary)
        read_fd, write_fd = os.pipe()
        server = None
        try:
            # -displayfd selects a free X server number without touching an
            # existing display. No X command is run until this server reports it.
            server_environment = {**os.environ, "DISPLAY": "",
                                  "XAUTHORITY": str(root / "private-xauthority")}
            server_environment.pop("WAYLAND_DISPLAY", None)
            with (root / "xvfb.log").open("wb") as log:
                server = subprocess.Popen([args.xvfb, "-displayfd", str(write_fd), "-screen", "0",
                    "1280x1024x24", "-nolisten", "tcp"], pass_fds=(write_fd,),
                    stdout=log, stderr=subprocess.STDOUT, env=server_environment)
            os.close(write_fd)
            write_fd = None
            if not select.select([read_fd], [], [], 5)[0]:
                raise RuntimeError("private Xvfb did not start")
            display_number = os.read(read_fd, 64).decode().strip()
            if not display_number.isdecimal() or server.poll() is not None:
                raise RuntimeError("private Xvfb failed to allocate its display")
            environment = {**server_environment, "DISPLAY": f":{display_number}"}
            for variable in list(environment):
                if variable.startswith("BASALT_"):
                    environment.pop(variable)
            report = exercise(args, environment, root)
            if args.output:
                args.output.parent.mkdir(parents=True, exist_ok=True)
                args.output.write_text(json.dumps(report, indent=2) + "\n")
            print(json.dumps(report, indent=2))
        finally:
            os.close(read_fd)
            if write_fd is not None:
                os.close(write_fd)
            if server is not None:
                server.terminate()
                server.wait(timeout=3)


if __name__ == "__main__":
    main()
