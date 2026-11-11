import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import difflib
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def progress_file(directory, previous):
    latest, name = previous
    for path in directory.rglob("*"):
        if path.suffix not in {".json", ".log"}:
            continue
        try:
            modified = path.stat().st_mtime_ns
        except FileNotFoundError:
            continue
        if modified > latest:
            latest, name = modified, path.relative_to(directory).as_posix()
    return latest, name


def run(command, directory, source, stall_seconds=180, limit_seconds=1500, poll_seconds=1.0):
    scratch = directory / "native-temp"
    scratch.mkdir()
    environment = dict(os.environ, TMP=str(scratch), TEMP=str(scratch), TMPDIR=str(scratch))
    started = last_progress_at = time.monotonic()
    progress = time.time_ns(), "process_started"
    reason = "completed"
    with (directory / "stdout.log").open("wb") as stdout, (directory / "stderr.log").open("wb") as stderr:
        process = subprocess.Popen(command, cwd=ROOT, env=environment, stdout=stdout, stderr=stderr,
                                   start_new_session=os.name == "posix")
        while True:
            now = time.monotonic()
            updated = progress_file(directory, progress)
            if updated != progress:
                progress, last_progress_at = updated, now
            status = process.poll()
            if status is not None:
                reason = "completed" if status == 0 else "process_failed"
                break
            if now - last_progress_at >= stall_seconds or now - started >= limit_seconds:
                reason = "no_progress_limit" if now - last_progress_at >= stall_seconds else "time_limit"
                if os.name == "posix":
                    try:
                        os.killpg(process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                else:
                    subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"], capture_output=True, timeout=10)
                status = process.wait(timeout=10)
                break
            time.sleep(poll_seconds)
    result = {"source":source, "success":reason == "completed", "exit_reason":reason, "process_exit_code":status,
              "last_progress":progress[1], "elapsed_seconds":round(now - started, 3),
              "no_progress_seconds":round(now - last_progress_at, 3), "no_progress_limit_seconds":stall_seconds,
              "timeout_seconds":limit_seconds}
    write_json(directory / "watchdog.json", result)
    return result


def inventory(root):
    files = {}
    manifests = sorted(root.rglob("installation.manifest.json"))
    for manifest in manifests:
        data = read_json(manifest)
        assert data["format"] == "ue_animgraph_installation_manifest_v1"
        for relative in [manifest.relative_to(root).as_posix(), *data["files"]]:
            path = root / relative
            assert path.resolve().is_relative_to(root.resolve()), relative
            files[relative] = path.read_bytes()
    assert manifests
    return manifests, files


def generate(index, manifest, content, asset_root, output):
    data = read_json(manifest)
    source = data["source"]["package_path"]
    package = source.split(".", 1)[0]
    assert package.startswith("/ALS/"), package
    uasset = content / (package.removeprefix("/ALS/") + ".uasset")
    assert uasset.is_file() and uasset.name == data["source"]["uasset_name"], source
    directory = output / f"{index:02d}"
    install = directory / "install"
    install.mkdir(parents=True)
    command = [sys.executable, str(ROOT / "tools/animgraph_authoring/ue_animgraph_converter.py"),
               "--uasset", str(uasset), "--content-root", str(content), "--asset-root", str(asset_root),
               "--output-root", str(directory / "generated"), "--install-root", str(install),
               "--install-namespace", data["namespace"], "--write"]
    result = run(command, directory, source)
    return result, install


def compare(original, generated, output):
    changed = [name for name in sorted(original.keys() | generated.keys()) if original.get(name) != generated.get(name)]
    rows = []
    with (output / "full_diff.patch").open("w", encoding="utf-8") as patch:
        for name in changed:
            before, after = original.get(name), generated.get(name)
            rows.append({"file":name, "before_sha256":hashlib.sha256(before).hexdigest() if before is not None else None,
                         "after_sha256":hashlib.sha256(after).hexdigest() if after is not None else None})
            patch.writelines(difflib.unified_diff((before or b"").decode().splitlines(keepends=True),
                (after or b"").decode().splitlines(keepends=True), fromfile="a/assets/" + name, tofile="b/assets/" + name))
    return {"installed_files":len(original), "generated_files":len(generated), "changes":rows}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--content-root", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            for label, code, reason in [("finish", "print('done')", "completed"),
                                        ("stalled", "import time; time.sleep(3)", "no_progress_limit")]:
                work = directory / label
                work.mkdir()
                result = run([sys.executable, "-c", code], work, label, 0.15, 5, 0.025)
                assert result["exit_reason"] == reason and read_json(work / "watchdog.json") == result
            comparison = compare({"one":b"a\n", "two":b"b\n"}, {"one":b"c\n", "three":b"d\n"}, directory)
            assert [row["file"] for row in comparison["changes"]] == ["one", "three", "two"]
        print("Installed corpus runner self-test PASS", flush=True)
        return
    if args.content_root is None or args.output is None:
        parser.error("content root and output are required")
    asset_root = ROOT / "assets"
    manifests, original = inventory(asset_root)
    manifests.sort(key=lambda path: (-len(read_json(path)["files"]), path.as_posix()))
    output = args.output.resolve()
    output.mkdir(parents=True)
    results = {}
    generated = {}

    def record(index, result, install):
        results[index] = result
        write_json(output / "corpus-progress.json", {"roots":len(manifests), "results":results})
        print(f"{len(results)}/{len(manifests)} {result['source']} {result['exit_reason']}", flush=True)
        if result["success"]:
            _, files = inventory(install)
            for name, content in files.items():
                assert generated.setdefault(name, content) == content, name

    content_root = args.content_root.resolve()
    result, install = generate(0, manifests[0], content_root, asset_root, output)
    record(0, result, install)
    if not result["success"]:
        raise SystemExit(1)
    with ThreadPoolExecutor(max_workers=2) as workers:
        tasks = {workers.submit(generate, index, manifest, content_root, asset_root, output):index
                 for index, manifest in enumerate(manifests[1:], 1)}
        for task in as_completed(tasks):
            record(tasks[task], *task.result())
    if any(not result["success"] for result in results.values()):
        raise SystemExit(1)
    write_json(output / "corpus-diff.json", compare(original, generated, output))
    print(f"Compared {len(original)} installed and {len(generated)} generated files", flush=True)


if __name__ == "__main__":
    main()
