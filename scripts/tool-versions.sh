#!/usr/bin/env bash
# Pinned versions of the tools the gates depend on. The single source for
# scripts/dev-setup.sh, scripts/doctor.sh and .github/actions/setup, so a developer and CI
# always run the same versions. Changing one is its own PR (CLAUDE.md section 5).
export UV_VERSION=0.12.15
export CARGO_MUTANTS_VERSION=27.1.0
export CARGO_LLVM_COV_VERSION=0.9.1
