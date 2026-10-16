import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import unittest


ROOT = Path(__file__).resolve().parents[2]


def quoted(value):
    return "'" + str(value).replace("'", "''") + "'"


class PlatformToolsTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="als platform ")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name).resolve()
        (self.root / "tools").mkdir()
        for source in [*ROOT.glob("*.ps1"), *ROOT.joinpath("tools").glob("*.ps1")]:
            shutil.copy2(source, self.root / source.relative_to(ROOT))
        shutil.copytree(ROOT / "tools/export", self.root / "tools/export")

    def run_script(self, body, **environment):
        script = self.root / "fixture.ps1"
        script.write_text(body, encoding="utf-8")
        return subprocess.run(
            ["pwsh", "-NoProfile", "-File", str(script)],
            cwd=self.root,
            env={**os.environ, **environment},
            capture_output=True,
            text=True,
            timeout=20,
        )

    def test_manual_launcher_keeps_target_profile_jobs_and_exit(self):
        for jobs, status in [("", 0), ("3", 17)]:
            with self.subTest(jobs=jobs, status=status):
                result = self.run_script(
                    "function global:cargo { "
                    "@{argv=@($args);target=$env:CARGO_TARGET_DIR} | ConvertTo-Json -Compress | Set-Content call.json; "
                    "$global:LASTEXITCODE=[int]$env:FIXTURE_STATUS }; "
                    "& ./run_manual.ps1; exit $LASTEXITCODE",
                    CARGO_BUILD_JOBS=jobs,
                    CARGO_TARGET_DIR="unrelated-target",
                    FIXTURE_STATUS=str(status),
                )
                self.assertEqual(result.returncode, status, result.stderr)
                call = json.loads((self.root / "call.json").read_text(encoding="utf-8-sig"))
                self.assertEqual(call["argv"], ["run", "--jobs", jobs or "2", "--profile", "play", "--example", "manual_playground"])
                self.assertEqual(Path(call["target"]), self.root / "target_game")

    def test_visual_runner_checks_report_even_when_manual_fails(self):
        (self.root / "run_manual.ps1").write_text(
            "$dir=$env:ALS_ANIMATION_VISUAL_EVIDENCE_DIR; "
            "'{}' | Set-Content (Join-Path $dir report.json); exit 17",
            encoding="utf-8",
        )
        result = self.run_script(
            "function global:python { 'checked' | Set-Content validator.txt; $global:LASTEXITCODE=0 }; "
            "function global:python3 { python @args }; "
            "& ./tools/run_animation_visual_validation.ps1 -Case base -FunctionalOnly"
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue((self.root / "validator.txt").is_file(), result.stderr)
        report = json.loads((self.root / "target/animation_visual/base/runner.json").read_text(encoding="utf-8-sig"))
        self.assertEqual(report["manual_exit_code"], 17)
        self.assertEqual(report["validation_exit_code"], 0)

    def test_visual_runner_rejects_outside_directory_without_deleting_it(self):
        outside = self.root / "keep"
        outside.mkdir()
        marker = outside / "marker"
        marker.write_text("keep", encoding="utf-8")
        result = self.run_script(
            "& ./tools/run_animation_visual_validation.ps1 -OutputDirectory " + quoted(outside)
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(marker.read_text(), "keep")

    def test_reader_build_uses_native_uat_and_propagates_failure(self):
        engine = self.root / "engine space"
        native = "Win64" if os.name == "nt" else "Mac"
        command = engine / "Binaries" / native / ("UnrealEditor-Cmd.exe" if os.name == "nt" else "UnrealEditor-Cmd")
        command.parent.mkdir(parents=True)
        command.touch()
        uat = engine / "Build/BatchFiles" / ("RunUAT.bat" if os.name == "nt" else "RunUAT.sh")
        uat.parent.mkdir(parents=True)
        uat.write_text(
            '@echo off\necho %* > "%BUILD_ARGS%"\nexit /b 23\n'
            if os.name == "nt" else '#!/bin/sh\nprintf "%s\\n" "$@" > "$BUILD_ARGS"\nexit 23\n',
            encoding="utf-8",
        )
        uat.chmod(0o700)
        (self.root / "tools/env.local.json").write_text(json.dumps({"ue_cmd": str(command)}), encoding="utf-8")
        output = self.root / "build arguments.txt"
        result = self.run_script("& ./tools/build_ue_source_reader.ps1; exit $LASTEXITCODE", BUILD_ARGS=str(output))
        self.assertEqual(result.returncode, 23, result.stderr)
        arguments = output.read_text()
        self.assertIn("-TargetPlatforms=" + native, arguments)
        self.assertIn("-MaxParallelActions=2", arguments)
        self.assertIn(str(self.root / "target/ue_graph_source_reader_package"), arguments)

    def test_python_prefers_worktree_venv(self):
        python = self.root / ".venv" / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
        python.parent.mkdir(parents=True)
        python.touch()
        result = self.run_script("& ./tools/python.ps1")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(Path(result.stdout.strip()), python)

    def test_integration_stops_at_first_failed_case(self):
        (self.root / "run_manual.ps1").write_text(
            '$env:ALS_MANUAL_INTEGRATION_CASE | Add-Content (Join-Path $PSScriptRoot cases.txt); '
            'if ($env:ALS_MANUAL_INTEGRATION_CASE -eq "ragdoll") { exit 17 }; exit 0',
            encoding="utf-8",
        )
        result = self.run_script("& ./run_manual_integration_gate.ps1; exit $LASTEXITCODE")
        self.assertEqual(result.returncode, 17, result.stderr)
        self.assertEqual((self.root / "cases.txt").read_text(encoding="utf-8-sig").splitlines(), ["base", "ragdoll"])

    def test_graph_editor_validates_before_launch(self):
        for status, only in [(9, ""), (0, "1"), (0, "")]:
            with self.subTest(status=status, only=only):
                marker = self.root / "cargo.txt"
                marker.unlink(missing_ok=True)
                result = self.run_script(
                    'function global:python { $global:LASTEXITCODE=[int]$env:FIXTURE_STATUS }; '
                    'function global:python3 { python @args }; '
                    'function global:cargo { "launched" | Set-Content cargo.txt; $global:LASTEXITCODE=0 }; '
                    '& ./als_graph_editor.ps1; exit $LASTEXITCODE',
                    FIXTURE_STATUS=str(status), ALS_GRAPH_EDITOR_VALIDATE_ONLY=only,
                )
                self.assertEqual(result.returncode, status, result.stderr)
                self.assertEqual(marker.exists(), status == 0 and not only)

    def test_reader_rejects_other_platform_cache(self):
        source = self.root / "source"
        (source / "Source").mkdir(parents=True)
        (source / "Source/Reader.cpp").touch()
        (source / "AlsGraphSourceReader.uplugin").touch()
        package = self.root / "package"
        package.mkdir()
        plugin = package / "AlsGraphSourceReader.uplugin"
        plugin.touch()
        for windows, folder, extension in [(True, "Win64", "dll"), (False, "Mac", "dylib")]:
            with self.subTest(platform=folder):
                library = package / "Binaries" / folder / (("" if windows else "lib") + "UnrealEditor-AlsGraphSourceReader." + extension)
                library.parent.mkdir(parents=True)
                library.touch()
                os.utime(library, (time.time() + 5, time.time() + 5))
                result = self.run_script(
                    '. ./tools/run_ue_source_reader.ps1 -FunctionsOnly; '
                    'Set-Variable IsWindows -Force -Value $' + str(windows).lower() + '; '
                    'Test-ReaderBuildRequired ' + quoted(source) + ' ' + quoted(plugin) + '; '
                    'Set-Variable IsWindows -Force -Value $' + str(not windows).lower() + '; '
                    'Test-ReaderBuildRequired ' + quoted(source) + ' ' + quoted(plugin)
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout.splitlines(), ["False", "True"])
                library.unlink()

    def test_windows_build_command_on_any_host(self):
        script = self.root / "tools/build_ue_source_reader.ps1"
        script.write_text(script.read_text().replace("& $uat ", "Invoke-FixtureUat $uat "), encoding="utf-8")
        engine = self.root / "engine space"
        (self.root / "tools/env.local.json").write_text(json.dumps({"ue_cmd": str(engine / "Binaries/Win64/UnrealEditor-Cmd.exe")}), encoding="utf-8")
        result = self.run_script(
            'Set-Variable IsWindows -Force -Value $true; '
            'function global:Invoke-FixtureUat { ConvertTo-Json -InputObject @($args) -Compress | Set-Content build.json; $global:LASTEXITCODE=23 }; '
            '& ./tools/build_ue_source_reader.ps1; exit $LASTEXITCODE'
        )
        self.assertEqual(result.returncode, 23, result.stderr)
        arguments = json.loads((self.root / "build.json").read_text(encoding="utf-8-sig"))
        self.assertEqual(Path(arguments[0]), engine / "Build/BatchFiles/RunUAT.bat")
        self.assertIn("-TargetPlatforms=Win64", arguments)

    def test_export_keeps_spaced_arguments_and_failure_code(self):
        project = self.root / "project with spaces.ps1"
        project.write_text('ConvertTo-Json -InputObject @($args) -Compress | Set-Content arguments.json; exit 23', encoding="utf-8")
        (self.root / "tools/env.local.json").write_text(json.dumps({"ue_cmd": shutil.which("pwsh"), "ue_project": str(project)}), encoding="utf-8")
        result = self.run_script('& ./tools/export/run_cmdlet.ps1 -Script "tools/a script.py" -Map "/Game/A map"; exit $LASTEXITCODE')
        self.assertEqual(result.returncode, 23, result.stderr)
        arguments = json.loads((self.root / "arguments.json").read_text(encoding="utf-8-sig"))
        self.assertIn("/Game/A map", arguments)
        self.assertIn("-script=" + str(self.root / "tools/a script.py"), arguments)
        if os.name != "nt":
            result = subprocess.run(["bash", "tools/export/run_cmdlet.sh", "tools/a script.py"], cwd=self.root, capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 23, result.stderr)

    def test_worktree_copies_caches_and_mtimes_without_incremental(self):
        def git(*args):
            subprocess.run(["git", *args], cwd=self.root, check=True, capture_output=True, timeout=10)

        git("init")
        (self.root / "tracked.txt").write_text("source", encoding="utf-8")
        os.utime(self.root / "tracked.txt", (time.time() - 3600, time.time() - 3600))
        git("add", "tracked.txt")
        git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-m", "fixture")
        for path in ["target/debug/deps/keep", "target/debug/incremental/drop", "target_game/play/deps/keep", "target_game/play/deps/drop.pdb"]:
            target = self.root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text("cache", encoding="utf-8")
        destination = self.root / "new worktree"
        result = self.run_script("& ./tools/new-worktree.ps1 -Name '" + self.root.name + "/new worktree' -Branch codex/fixture")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((destination / "tracked.txt").stat().st_mtime_ns, (self.root / "tracked.txt").stat().st_mtime_ns)
        for path in ["target/debug/deps/keep", "target_game/play/deps/keep"]:
            self.assertEqual((destination / path).read_text(), "cache")
        for path in ["target/debug/incremental/drop", "target_game/play/deps/drop.pdb"]:
            self.assertFalse((destination / path).exists())


if __name__ == "__main__":
    unittest.main()
