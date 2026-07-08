# lb-cargo volume: `Permission denied` opening downloaded crates

**Symptom.** First `make windows-executable` run died mid-download:

```
warning: failed to write cache, path: /usr/local/cargo/registry/index/.../hostname, error: Permission denied (os error 13)
error: failed to open `/usr/local/cargo/registry/cache/index.crates.io-.../Inflector-0.11.4.crate`
Caused by: Permission denied (os error 13)
```

**Cause.** The shared `lb-cargo` named volume held root-owned files from an
earlier container run, while the build runs `--user $(id -u):$(id -g)` (the
Makefile's dev-mode convention). The image pre-chmods the cache dirs 0777, but
that only seeds the volume on *first* mount — files written later by a root
container keep root ownership.

**Fix.** One-off perms repair:

```sh
docker run --rm -v lb-cargo:/reg ubuntu:22.04 bash -c 'chmod -R a+rwX /reg'
```

(or `make clean` to drop and re-populate the volumes). If it recurs, the durable
fix is to never run the image as root against the shared volumes.

**Regression guard.** Behavioural, not unit-testable: the `windows-executable`
lane itself exercises the volume on every build.
