#!/usr/bin/env python3
"""Small OpenAI Responses API solver used by the opt-in live AIME smoke."""

from __future__ import annotations

import json
import os
import sys
import urllib.error
import urllib.request


def output_text(payload: dict) -> str:
    if isinstance(payload.get("output_text"), str):
        return payload["output_text"]
    chunks: list[str] = []
    for item in payload.get("output", []):
        for content in item.get("content", []):
            if content.get("type") == "output_text":
                chunks.append(content.get("text", ""))
    return "\n".join(chunk for chunk in chunks if chunk).strip()


def main() -> None:
    api_key = os.environ.get("OPENAI_API_KEY")
    if not api_key:
        raise SystemExit("OPENAI_API_KEY is required")

    model = os.environ.get("LEAVEN_OPENAI_MODEL", "gpt-4.1-mini")
    system_prompt = os.environ["LEAVEN_AIME_SYSTEM_PROMPT"]
    problem = os.environ["LEAVEN_AIME_PROBLEM"]
    body = {
        "model": model,
        "instructions": system_prompt,
        "input": f"Solve this AIME-style problem. Return only the final integer.\n\n{problem}",
    }
    request = urllib.request.Request(
        "https://api.openai.com/v1/responses",
        data=json.dumps(body).encode("utf-8"),
        headers={
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            payload = json.loads(response.read())
    except urllib.error.HTTPError as error:
        sys.stderr.write(error.read().decode("utf-8", errors="replace"))
        raise SystemExit(error.code) from error

    text = output_text(payload)
    if not text:
        raise SystemExit("response did not contain text output")
    print(text.strip())


if __name__ == "__main__":
    main()
