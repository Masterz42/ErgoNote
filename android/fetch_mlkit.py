#!/usr/bin/env python3
"""Fetch and unpack Google ML Kit Text Recognition (Latin/French, bundled offline)
Maven dependencies into a staging directory for javac, d8, aapt2, and APK packaging.
"""

import os
import shutil
import sys
import urllib.request
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

REPOS = [
    "https://dl.google.com/dl/android/maven2",
    "https://repo1.maven.org/maven2",
]

ROOT_COORDS = [
    ("com.google.mlkit", "text-recognition", "16.0.1"),
    ("com.google.android.gms", "play-services-mlkit-document-scanner", "16.0.0"),
]

TARGET_ABI = "arm64-v8a"


def cache_root() -> Path:
    xdg = os.environ.get("XDG_CACHE_HOME")
    base = Path(xdg) if xdg else Path.home() / ".cache"
    return base / "ergonote" / "android-mlkit"


def fetch_bytes(rel_path: str, dest: Path) -> bytes:
    if dest.is_file() and dest.stat().st_size > 0:
        return dest.read_bytes()
    dest.parent.mkdir(parents=True, exist_ok=True)
    last_err = None
    for repo in REPOS:
        url = f"{repo}/{rel_path}"
        try:
            with urllib.request.urlopen(url, timeout=30) as resp:
                data = resp.read()
            tmp = dest.with_suffix(dest.suffix + ".part")
            tmp.write_bytes(data)
            tmp.replace(dest)
            return data
        except Exception as err:
            last_err = err
    raise RuntimeError(f"Failed to download {rel_path}: {last_err}")


def resolve_tree(cache: Path):
    seen = {}
    queue = list(ROOT_COORDS)
    while queue:
        group, artifact, version = queue.pop(0)
        if (group, artifact) in seen:
            continue
        base_rel = f"{group.replace('.', '/')}/{artifact}/{version}/{artifact}-{version}"
        pom_bytes = fetch_bytes(f"{base_rel}.pom", cache / "poms" / f"{group}__{artifact}__{version}.pom")
        root = ET.fromstring(pom_bytes)
        ns = {"m": "http://maven.apache.org/POM/4.0.0"}
        pkg_el = root.find("m:packaging", ns)
        pkg = pkg_el.text.strip() if pkg_el is not None and pkg_el.text else "jar"
        seen[(group, artifact)] = (version, pkg)

        deps_el = root.find("m:dependencies", ns)
        if deps_el is None:
            continue
        for dep in deps_el.findall("m:dependency", ns):
            scope_el = dep.find("m:scope", ns)
            scope = scope_el.text.strip() if scope_el is not None and scope_el.text else "compile"
            opt_el = dep.find("m:optional", ns)
            if scope in ("test", "provided") or (opt_el is not None and opt_el.text == "true"):
                continue
            dg = dep.find("m:groupId", ns).text.strip()
            da = dep.find("m:artifactId", ns).text.strip()
            dv_el = dep.find("m:version", ns)
            if dv_el is None or not dv_el.text:
                continue
            dv = dv_el.text.strip().strip("[]()")
            if "," in dv:
                dv = dv.split(",")[0].strip()
            if (dg, da) not in seen:
                queue.append((dg, da, dv))
    return seen


def stage_mlkit(out_dir: Path) -> None:
    cache = cache_root()
    artifacts = resolve_tree(cache)

    jars_dir = out_dir / "jars"
    assets_dir = out_dir / "assets"
    jni_dir = out_dir / "jni" / TARGET_ABI
    if out_dir.exists():
        shutil.rmtree(out_dir)
    jars_dir.mkdir(parents=True, exist_ok=True)
    assets_dir.mkdir(parents=True, exist_ok=True)
    jni_dir.mkdir(parents=True, exist_ok=True)

    for (group, artifact), (version, pkg) in sorted(artifacts.items()):
        if pkg == "pom":
            continue
        ext = "aar" if pkg == "aar" else "jar"
        rel = f"{group.replace('.', '/')}/{artifact}/{version}/{artifact}-{version}.{ext}"
        cached = cache / "artifacts" / f"{group}__{artifact}__{version}.{ext}"
        fetch_bytes(rel, cached)

        tag = f"{group}__{artifact}__{version}".replace(".", "_")
        if ext == "jar":
            with zipfile.ZipFile(cached, "r") as zf:
                has_classes = any(n.endswith(".class") for n in zf.namelist())
            if has_classes:
                shutil.copyfile(cached, jars_dir / f"{tag}.jar")
            continue

        with zipfile.ZipFile(cached, "r") as zf:
            for member in zf.namelist():
                if member == "classes.jar":
                    (jars_dir / f"{tag}.jar").write_bytes(zf.read(member))
                elif member.startswith("libs/") and member.endswith(".jar"):
                    lib_name = Path(member).name
                    (jars_dir / f"{tag}__{lib_name}").write_bytes(zf.read(member))
                elif member.startswith(f"jni/{TARGET_ABI}/") and member.endswith(".so"):
                    so_name = Path(member).name
                    (jni_dir / so_name).write_bytes(zf.read(member))
                elif member.startswith("assets/") and not member.endswith("/"):
                    rel_asset = member[len("assets/"):]
                    dest = assets_dir / rel_asset
                    dest.parent.mkdir(parents=True, exist_ok=True)
                    dest.write_bytes(zf.read(member))


def main() -> int:
    if len(sys.argv) != 2:
        print(f"usage: {sys.argv[0]} <staging-dir>", file=sys.stderr)
        return 2
    out_dir = Path(sys.argv[1])
    stage_mlkit(out_dir)
    jars = sorted(out_dir.joinpath("jars").glob("*.jar"))
    sos = sorted(out_dir.joinpath("jni", TARGET_ABI).glob("*.so"))
    assets = [p for p in out_dir.joinpath("assets").rglob("*") if p.is_file()]
    print(f"staged ML Kit dependencies: {len(jars)} jars, {len(sos)} native libs, {len(assets)} model assets")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
