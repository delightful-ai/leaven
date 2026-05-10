#!/usr/bin/env python3
"""Materialize the public AIME GEPA example dataset cache.

This mirrors the upstream GEPA AIME example's HuggingFace sources:
AI-MO/aimo-validation-aime for train/validation and MathArena/aime_2025
for final held-out test.
"""

from __future__ import annotations

import argparse
import json
import random
from pathlib import Path

from datasets import load_dataset


def aime_case(item: dict, *, needs_modular: bool) -> dict:
    return {
        "problem": item["problem"],
        "answer": int(item["answer"]),
        "solution": item.get("solution", ""),
        "needs_modular": needs_modular,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--out",
        default="target/leaven-aime-cache/aime.json",
        help="JSON cache path consumed by LEAVEN_AIME_CACHE",
    )
    args = parser.parse_args()

    train_val = [
        aime_case(item, needs_modular=True)
        for item in load_dataset("AI-MO/aimo-validation-aime", "default", split="train")
    ]
    random.Random(0).shuffle(train_val)

    test = [
        aime_case(item, needs_modular=True)
        for item in load_dataset("MathArena/aime_2025", "default", split="train")
    ]

    train_size = len(train_val) // 2
    cache = {
        "train": train_val[:train_size],
        "validation": train_val[train_size:],
        "test": test,
    }

    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(cache, indent=2, sort_keys=True), encoding="utf-8")
    print(out)


if __name__ == "__main__":
    main()
