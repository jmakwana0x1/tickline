# deployments

Phase 8. One JSON file per network, recording deployed addresses, the commit they were built
from, and the verification status of each contract.

`base-sepolia.json` is written by the deploy script, committed, and read by the fork suite and
the post-deploy smoke script, so "which contract is live" has exactly one answer.
