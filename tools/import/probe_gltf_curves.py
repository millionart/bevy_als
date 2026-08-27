import argparse
import json


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("asset")
    args = parser.parse_args()
    with open(args.asset, encoding="utf-8") as source:
        document = json.load(source)
    for animation in document.get("animations", []):
        extension = animation.get("extensions", {}).get("ALS_float_curves", {})
        for curve in extension.get("curves", []):
            keys = list(zip(curve.get("times", []), curve.get("values", []), strict=True))
            print("%s: %s" % (curve.get("name", ""), keys))


if __name__ == "__main__":
    main()
