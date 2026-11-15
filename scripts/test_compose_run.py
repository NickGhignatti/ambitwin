#!/usr/bin/env python3
"""Self-check for scripts/compose/compose-run.py: which files and services each mode picks."""

import os
import subprocess
import sys
import tempfile
from pathlib import Path

COMPOSE_RUN = Path(__file__).resolve().parent / "compose" / "compose-run.py"

# stands in for docker: lists the stack's services, refuses everything else
DOCKER_STUB = """#!/bin/sh
[ -n "$DOCKER_STUB_FAILS" ] && { echo "required variable APP_HOST is missing a value" >&2; exit 1; }
case "$*" in
  *"config --services"*) printf 'chat\\nchat-db\\nap-collector\\nrust-base\\nsensor-simulator\\n' ;;
  *) exit 1 ;;
esac
"""


def _stack_directory() -> Path:
    stack = Path(tempfile.mkdtemp())
    for compose_file in (
        "docker-compose.yml",
        "docker-compose.prod.yml",
        "backend/chat/docker-compose.yml",
        "backend/chat/docker-compose.dev.yml",
        "devices/ap-collector/docker-compose.yml",
        "simulators/sensor-simulator/docker-compose.yml",
    ):
        path = stack / compose_file
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("services: {}\n")
    docker_stub = stack / "bin" / "docker"
    docker_stub.parent.mkdir()
    docker_stub.write_text(DOCKER_STUB)
    docker_stub.chmod(0o755)
    return stack


def _dry_run(*arguments: str) -> tuple[str, str]:
    """Returns the docker command the runner would execute and the runtime file it wrote."""
    stack = _stack_directory()
    result = subprocess.run(
        [sys.executable, str(COMPOSE_RUN), *arguments, "--dry-run"],
        cwd=stack,
        env={**os.environ, "PATH": f"{stack / 'bin'}{os.pathsep}{os.environ['PATH']}"},
        capture_output=True,
        text=True,
        check=True,
    )
    command = next(line for line in result.stdout.splitlines() if line.startswith("$ docker"))
    return command, (stack / "compose.runtime.yml").read_text()


def test_prod_pulls_the_published_images_and_never_builds():
    command, _ = _dry_run("prod")
    assert "-f compose.runtime.yml -f docker-compose.prod.yml up -d" in command
    assert "--pull always" in command
    assert "--build" not in command


def test_prod_leaves_out_the_dev_overrides():
    _, runtime_file = _dry_run("prod")
    assert "backend/chat/docker-compose.yml" in runtime_file
    assert "docker-compose.dev.yml" not in runtime_file


def test_prod_never_starts_what_has_no_published_image():
    command, _ = _dry_run("prod")
    started = command.split("--pull always")[1].split()
    assert started == ["chat", "chat-db", "sensor-simulator"]


def test_prod_keeps_an_excluded_folder_in_the_file_list_but_does_not_start_it():
    command, runtime_file = _dry_run("prod", "--no-simulators")
    # the production override names every service, so each one must still be defined
    assert "simulators/sensor-simulator/docker-compose.yml" in runtime_file
    assert "sensor-simulator" not in command


def test_prod_stops_when_the_stack_cannot_be_read():
    # e.g. APP_HOST missing from .env: starting anyway would also start the build-only services
    stack = _stack_directory()
    result = subprocess.run(
        [sys.executable, str(COMPOSE_RUN), "prod", "--dry-run"],
        cwd=stack,
        env={
            **os.environ,
            "PATH": f"{stack / 'bin'}{os.pathsep}{os.environ['PATH']}",
            "DOCKER_STUB_FAILS": "1",
        },
        capture_output=True,
        text=True,
    )
    assert result.returncode != 0
    assert "APP_HOST" in result.stderr


def test_dev_still_drops_an_excluded_folder_from_the_file_list():
    _, runtime_file = _dry_run("dev", "--no-simulators")
    assert "simulators/sensor-simulator" not in runtime_file
    assert "backend/chat/docker-compose.dev.yml" in runtime_file


if __name__ == "__main__":
    test_prod_pulls_the_published_images_and_never_builds()
    test_prod_leaves_out_the_dev_overrides()
    test_prod_never_starts_what_has_no_published_image()
    test_prod_keeps_an_excluded_folder_in_the_file_list_but_does_not_start_it()
    test_prod_stops_when_the_stack_cannot_be_read()
    test_dev_still_drops_an_excluded_folder_from_the_file_list()
    print("ok")
