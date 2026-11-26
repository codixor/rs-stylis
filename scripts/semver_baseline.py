"""Select a non-yanked crates.io baseline. A matching tag compares to an older release."""
import json
import os
from pathlib import Path
import tomllib
from urllib.error import HTTPError
from urllib.request import Request, urlopen


def precedence(version):
    core, _, pre = version.split("+", 1)[0].partition("-")
    ids = tuple((0, int(p)) if p.isdigit() else (1, p) for p in pre.split(".")) if pre else ()
    return tuple(map(int, core.split("."))), not pre, ids


def baseline():
    manifest = tomllib.loads(Path("Cargo.toml").read_text(encoding="utf-8"))
    package = manifest["package"]
    version = package["version"]
    if isinstance(version, dict):
        version = manifest["workspace"]["package"]["version"]
    name = package["name"]
    index_url = f"https://index.crates.io/{name[:2]}/{name[2:4]}/{name}"
    request = Request(index_url, headers={"User-Agent": f"codixor-{name}-ci"})
    try:
        with urlopen(request, timeout=30) as response:
            releases = [json.loads(line) for line in response]
    except HTTPError as error:
        if error.code == 404 and version == "0.1.0":
            return ""
        raise
    tagged = os.environ.get("GITHUB_REF_TYPE") == "tag"
    current = precedence(version)
    candidates = []
    for release in releases:
        candidate = release["vers"]
        key = precedence(candidate)
        if not release["yanked"] and (key < current or (not tagged and key == current)):
            candidates.append((key, candidate))
    if candidates:
        return max(candidates)[1]
    if version == "0.1.0":
        return ""
    raise SystemExit(f"No eligible published baseline for {name} {version}")


if __name__ == "__main__":
    print(baseline())
