import argparse
import json
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

from audit_ab_als_sync_semantics import (
    animation_assets,
    blendspace_assets,
    source_rows,
)


def quote(value):
    return json.dumps(str(value), ensure_ascii=False)


def number(value):
    text = f"{float(value):.9f}".rstrip("0").rstrip(".")
    return text if "." in text else f"{text}.0"


def marker_ron(markers, indent):
    if not markers:
        return "[]"
    rows = [
        f"(name: {quote(name)}, time: {number(time)})" for name, time in markers
    ]
    return "[" + ", ".join(rows) + "]"


def samples_ron(samples):
    if not samples:
        return "[]"
    rows = []
    for sample in samples:
        rows.append(
            "(asset: {asset}, duration: {duration}, markers: {markers})".format(
                asset=quote(sample["asset"]),
                duration=number(sample["length"]),
                markers=marker_ron(sample["markers"], 0),
            )
        )
    return "[" + ", ".join(rows) + "]"


def metadata(rows):
    records = sorted(
        (row for row in rows if row["needs_mapping"]),
        key=lambda row: (row["contract"], row["outer"], row["kind"], row["asset"]),
    )
    lines = ["(", "    version: 1,", "    records: ["]
    for row in records:
        lines.extend(
            [
                "        (",
                f"            contract: {quote(row['contract'])},",
                f"            source_graph: {quote(row['outer'])},",
                f"            kind: {quote(row['kind'])},",
                f"            asset: {quote(row['asset'])},",
                f"            sync_group: {quote(row['group'])},",
                f"            group_role: {quote(row['role'])},",
                f"            sync_method: {quote(row['method'])},",
                f"            looped: {str(row['loop']).lower()},",
                f"            play_rate: {number(row['rate'])},",
                f"            start_position: {number(row['start'])},",
                f"            reset_on_asset_change: {str(row['reset_on_asset_change']).lower()},",
                f"            duration: {number(row['asset_length'])},",
                "            filter: (duration: {duration}, kind: {kind}),".format(
                    duration=number(row["interpolation_time"]),
                    kind=quote(row["interpolation_type"]),
                ),
                f"            markers: {marker_ron(row['markers'], 12)},",
                f"            samples: {samples_ron(row['samples'])},",
                "        ),",
            ]
        )
    lines.extend(["    ],", ")", ""])
    return "\n".join(lines), records


def markdown(records, manifest_matches):
    groups = Counter(record["group"] for record in records)
    marker_records = sum(
        bool(record["markers"])
        or any(sample["markers"] for sample in record["samples"])
        for record in records
    )
    lines = [
        "# ALS Sync Metadata Derivation",
        "",
        "This report is generated from source AnimBlueprint and animation uassets supplied on the command line.",
        "",
        "## Verdict",
        "",
        f"- Derived timing records: {len(records)}",
        f"- Records with marker data: {marker_records}",
        f"- Manifest matches source: {manifest_matches}",
        "",
        "## Sync Groups",
        "",
        "| Group | Records |",
        "| --- | ---: |",
    ]
    for group, count in sorted(groups.items()):
        lines.append(f"| {group} | {count} |")
    lines.extend(
        [
            "",
            "## Result",
            "",
            f"- Coverage: {'PASS' if records else 'FAIL'}",
            f"- Manifest: {'PASS' if manifest_matches else 'FAIL'}",
            "",
        ]
    )
    return "\n".join(lines), bool(records) and manifest_matches


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-root", required=True, type=Path)
    parser.add_argument("--manifest", type=Path, default=Path("assets/ALS/Animations/als_sync_metadata.ron"))
    parser.add_argument("--report", type=Path, default=Path("docs/sync-02-derived-metadata.md"))
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    animations = animation_assets(args.source_root)
    blendspaces = blendspace_assets(args.source_root, animations)
    rows, _ = source_rows(args.source_root, animations, blendspaces)
    payload, records = metadata(rows)
    manifest_matches = args.manifest.is_file() and args.manifest.read_text(encoding="utf-8") == payload
    if not args.check:
        args.manifest.parent.mkdir(parents=True, exist_ok=True)
        args.manifest.write_text(payload, encoding="utf-8", newline="\n")
        manifest_matches = True
    report, passed = markdown(records, manifest_matches)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(report, encoding="utf-8", newline="\n")
    print(
        json.dumps(
            {
                "manifest": args.manifest.as_posix(),
                "report": args.report.as_posix(),
                "records": len(records),
                "manifest_matches": manifest_matches,
                "pass": passed,
            },
            indent=2,
        )
    )
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
