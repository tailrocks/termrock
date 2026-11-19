# Generated-workflow guard setup

The guard checks generated workflow files in the effective staged index before commits and in every non-deletion branch or tag commit before pushes. Branch files and configuration are input data. Git hooks run a checker copied into owner-private Git common metadata, so a checkout cannot replace the checker that runs for that checkout.

## Install or update

Review the checker, installer, fixed configuration, embedded seed commit, and their pins. Then run:

```sh
bash scripts/install-generated-workflow-guard.sh
```

This is an explicit trust action. A branch change, commit, or push does not update the installed checker. Run the installer again from reviewed checkout code to install a reviewed update. The installer verifies the checked-in checker and support files against fixed digests, qualifies official Mise and Python artifacts, and checks the generated-hook directory before changing it. It refuses symlinked or foreign pre-commit and pre-push files. If setup stops before hook registration, existing hook files remain in place.

For setup when the anonymous GitHub API limit is exhausted, disable shell tracing and pass a GitHub token in the installer environment: `GITHUB_COM_TOKEN=$(gh auth token) bash scripts/install-generated-workflow-guard.sh`. The token must contain only ASCII letters, digits, and underscores, and be at most 512 characters. Setup uses it only for fixed-host blob requests for the verified seed. Hooks discard this variable and use anonymous requests if they need a missing blob.

The installed checker and its manifests live under the repository's Git common directory. That lets linked worktrees share one trusted installation while each hook resolves its own worktree, effective index, and split-index base. The hooks invoke the installed checker directly with a scrubbed environment. They do not load checkout or user hk configuration.

## Tool and dependency cache

The fixed configuration records the checker format, approved seed, Cargo lock digest, Mise version, Python version, and inventory helper digest. The installer verifies the official artifact hashes for the current platform and checks the Python runtime file inventory. The Python generation identity includes its platform, version, artifact, lock, private Mise configuration, inventory helper, and Mise identity, so a reviewed update can create a new generation without replacing the previous one. The installer then runs the installed checker against the pinned seed to qualify Mise tools, Rustup, and the locked Cargo dependencies. The resulting generation is content-inventoried and atomically published under Git common metadata.

Hooks verify the installed tool/dependency inventory before executing it. They use per-invocation temporary homes, Mise state, Git snapshots, and Cargo build output. A missing or changed cache fails closed and asks for explicit trusted setup. Warm checks select the qualified artifacts and configure Cargo offline. Snapshot blobs missing locally may still use the bounded fixed-source GitHub API hydration path; missing tree metadata fails closed.

On macOS, the real installed-checker harness runs two warm public hooks under `sandbox-exec` with networking denied and confirms that a curl control request is denied. On other hosts, treat offline behavior as qualified only when the caller enforces or observes a no-egress boundary. The hermetic regression matrix controls the external HTTP boundary with deterministic fixtures and does not qualify the real toolchain.

The real installed-checker harness creates a shallow source without the approved seed commit, checks matching commits and pushes, selected-index behavior, linked and split indexes, installed-cache integrity, and foreign-hook refusal, then confirms that staged workflow drift is rejected by the real Velnor comparison. Set `BASELINE_GUARD_EVIDENCE_DIR` to preserve its logs and cache manifests.

The checker pin is updated only in reviewed source together with its configuration and authenticated artifact identities. The installer does not modify global Git configuration or run a checker from the checkout after hook registration.

## Re-freezing the trusted seed

The seed is a parentless commit whose tree is a reviewed source commit plus the untracked lock sentinel `scripts/trusted-seed-cargo.lock`. Do not edit any of its pins by hand. From a clean checkout of the commit to freeze, with Python 3.11 or newer:

```sh
python3 scripts/refreeze-trusted-seed.py --check   # verify the pins against the committed artifacts, write nothing
python3 scripts/refreeze-trusted-seed.py --write   # rebuild the seed, its packs, the policy and every pin, then run --check
```

`--write` refuses a dirty tree and a source commit that `origin/main` does not contain. Review the diff, run the installer from the reviewed checkout, then run `scripts/check-generated-workflows.sh pre-commit` and `pre-push`. The `qualified_*` receipts in `scripts/policy/check-generated-workflow-auth.py` need a real installer qualification run and are not rewritten.
