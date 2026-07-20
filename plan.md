# Extraction plan

1. Remove Plainfeed data-model assumptions from the copied Git transport and
   repository operations.
2. Preserve the bounded fetch and constrained fast-forward push behavior.
3. Add native unit tests and a provider-independent Wasmtime loopback fixture.
4. Integrate Plainfeed through a thin application adapter without changing its
   synchronization policy.
5. Keep the Gitoxide compatibility dependency self-contained so consumers do
   not need to repeat the memmap2 Cargo patch. (Complete in the current fork.)

Higher-level file ownership, conflict handling, activation, and recovery remain
outside this repository until a second real application demonstrates a shared
contract.

## Manual WASIp2 workflow

1. Expose standalone-directory comparison and complete snapshot commits without
   turning the crate into general Git porcelain.
2. Provide a public example CLI for fetch, checkout, status, commit, ref
   inspection, and the existing constrained push.
3. Provide a foreground loopback Smart HTTP fixture so users can run each guest
   command manually from a second terminal.
4. Exercise that same CLI workflow in CI. (Complete locally; pending CI run.)
