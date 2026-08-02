# Docker (CLI only)

Minimal image that builds and runs the `ade` CLI. **Not** a full ADE Desktop / Tauri runtime.

```bash
docker build -f docker/Dockerfile -t ade-cli .
docker run --rm ade-cli --help
docker run --rm ade-cli acp --probe
```

Mount a workspace when you need local files:

```bash
docker run --rm -v "$PWD:/work" -w /work ade-cli audit
```
