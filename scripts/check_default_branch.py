"""Check that `.velnor/config.toml` names the expected default branch.

Offline by design: the expected branch is supplied by the caller, so no
GitHub call is made and forks may use any branch name.

Exit codes: 0 match, 1 mismatch, 2 invalid or missing configuration.
"""

import argparse
import pathlib
import sys
import tomllib


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--expected", required=True)
    parser.add_argument("--config", default=".velnor/config.toml")
    args = parser.parse_args()
    if not args.expected:
        print("--expected must be a nonempty branch name", file=sys.stderr)
        return 2
    try:
        with pathlib.Path(args.config).open("rb") as source:
            document = tomllib.load(source)
    except (OSError, tomllib.TOMLDecodeError) as error:
        print(f"invalid workflow configuration: {error}", file=sys.stderr)
        return 2
    workflow = document.get("workflow")
    if not isinstance(workflow, dict):
        print("invalid workflow configuration: [workflow] must be a table", file=sys.stderr)
        return 2
    if "default_branch" not in workflow:
        print("invalid workflow configuration: workflow.default_branch is missing", file=sys.stderr)
        return 2
    configured = workflow["default_branch"]
    if not isinstance(configured, str) or not configured:
        print("workflow.default_branch must be a nonempty string", file=sys.stderr)
        return 2
    if configured != args.expected:
        print(
            f"workflow.default_branch={configured!r}; expected {args.expected!r}",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
