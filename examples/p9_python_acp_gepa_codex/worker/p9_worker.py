import json
import os
import sys


def main() -> int:
    run_dir = os.environ["LEAVEN_P9_RUN_DIR"]
    responses = json.loads(os.environ["LEAVEN_P9_RESPONSE_MAP"])
    observed_path = os.path.join(run_dir, "acp_observed_requests.jsonl")
    os.makedirs(run_dir, exist_ok=True)

    with open(observed_path, "a", encoding="utf-8") as observed:
        while True:
            line = sys.stdin.readline()
            if not line:
                return 0
            request = json.loads(line)
            observed.write(json.dumps(request, sort_keys=True) + "\n")
            observed.flush()

            print(
                json.dumps(
                    {
                        "jsonrpc": "2.0",
                        "method": "session/update",
                        "params": {
                            "message": f"p9 python worker handled {request['method']}",
                            "priority": "critical",
                        },
                    },
                    sort_keys=True,
                ),
                flush=True,
            )

            response = responses[request["id"]]
            if response["result"]["method"] != request["method"]:
                raise RuntimeError(
                    f"response method {response['result']['method']} does not match "
                    f"request method {request['method']}"
                )
            response["result"]["capability_fingerprint"] = os.environ[
                "LEAVEN_CAPABILITY_FINGERPRINT"
            ]
            print(json.dumps(response, sort_keys=True), flush=True)


if __name__ == "__main__":
    raise SystemExit(main())
