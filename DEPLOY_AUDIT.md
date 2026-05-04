# Deployment Audit

## AWS — Minimum Viable Setup

### Target Architecture

Single-box deployment, all services on one EC2 instance.

| Component | Choice | Notes |
|-----------|--------|-------|
| EC2 | t4g.small (1 vCPU ARM, 2GB RAM) | t4g.nano OOMs; t4g.micro borderline |
| Storage | EBS gp3 10GB | Postgres data + logs |
| Object storage | S3 | Replaces MinIO — no container overhead |
| Web server | nginx | Serves static SvelteKit build |
| Runtime | Docker Compose | Postgres + Rust binary + nginx |

### Cost Estimate

| Item | $/mo (on-demand) | $/mo (1yr reserved) |
|------|-----------------|---------------------|
| t4g.small EC2 | $6.05 | $3.70 |
| EBS gp3 10GB | $0.80 | $0.80 |
| S3 (dev scale, <1GB) | $0.50 | $0.50 |
| Data transfer (first 100GB) | $0.00 | $0.00 |
| **Total** | **~$7.35** | **~$5.00** |

### Why t4g.small is the Floor

| Service | RAM |
|---------|-----|
| Postgres (tuned, `shared_buffers=128MB`) | ~256MB |
| Rust binary | ~30MB |
| nginx | ~10MB |
| OS + headroom | ~200MB |
| **Total** | **~496MB** |

t4g.micro (1GB) survives with tuned Postgres but leaves little headroom under load.  
t4g.nano (512MB) OOMs — MinIO alone (~150MB) + Node (~100MB) would push it over without the swap below.

### Key Decisions

- **MinIO → S3**: eliminates one container, frees ~150MB RAM
- **SvelteKit `adapter-static`**: no Node process at runtime; nginx serves pre-built HTML/JS/CSS
- **Never compile Rust on EC2**: cross-compile to `aarch64-unknown-linux-gnu` in CI, deploy binary via scp/rsync

### TODO

- [ ] `docker-compose.prod.yml`
- [ ] nginx config (static SvelteKit + reverse proxy to Rust backend)
- [ ] CI cross-compile pipeline (`aarch64-unknown-linux-gnu`)
- [ ] Postgres tuning (`shared_buffers`, `max_connections`)
- [ ] SSL via Let's Encrypt (certbot)
