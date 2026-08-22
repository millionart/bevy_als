#!/usr/bin/env python3
"""M2.5 import ALS curve tables (M0.5 uniform-sampled JSON) into CurveAsset RON files.

Run from repo root: python tools/import/gen_curves.py

For each assets/curves/**/*.json (schema: {asset, sampling:{mode,n,t_min,t_max}, keys:[{t,v}...]}),
writes a sibling .curve.ron with mode=Linear preserving the sampled table exactly.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
CURVES = REPO / "assets" / "curves"


def ron_f32(x: float) -> str:
    s = repr(float(x))
    return s if ("." in s or "e" in s or "inf" in s or "nan" in s) else s + ".0"


def main() -> int:
    failed = 0
    n_out = 0
    for jp in sorted(CURVES.rglob("*.json")):
        try:
            data = json.loads(jp.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as e:
            print(f"FAIL read {jp}: {e}", file=sys.stderr)
            failed += 1
            continue
        keys = data.get("keys") or []
        if not keys:
            print(f"SKIP empty {jp}", file=sys.stderr)
            continue
        rows = ", ".join(
            f"(t: {ron_f32(k['t'])}, v: {ron_f32(k['v'])})" for k in keys
        )
        ron = "(mode: Linear, keys: [" + rows + "])\n"
        out = jp.with_suffix(".curve.ron")
        out.write_text(ron, encoding="utf-8", newline="\n")
        n_out += 1
    print(f"done: {n_out} .curve.ron written, {failed} failures")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
