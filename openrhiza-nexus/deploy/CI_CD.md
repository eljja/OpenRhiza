# OpenRhiza Nexus CI/CD

This repository deploys `openrhiza-nexus` from `main`.

## GitHub Actions workflow

File:

- `.github/workflows/deploy-openrhiza-nexus.yml`

Trigger:

- push to `main` when `openrhiza-nexus/**` changes
- manual `workflow_dispatch`

## Required GitHub secrets

- `OPENRHIZA_HOST`: Ubuntu server host or IP
- `OPENRHIZA_USER`: SSH user for deployment
- `OPENRHIZA_SSH_KEY`: private key for the deploy user
- `OPENRHIZA_REPO_PATH`: absolute repository path on the server
- `OPENRHIZA_SERVICE_NAME`: systemd service name, usually `openrhiza-nexus`

## Server expectations

- the repository is already cloned on the server at `OPENRHIZA_REPO_PATH`
- the deploy user can run:
  - `git fetch`
  - `git pull --ff-only`
  - `npm ci`
  - `npm run build`
- the deploy user can restart the service with:
  - `sudo systemctl restart openrhiza-nexus`
- `openrhiza-nexus` listens on `127.0.0.1:3000`

## Post-deploy verification

The workflow verifies:

- `GET /api/health`
- `POST /api/v1/skill/download` for `skill_registry_lookup_v1`

If either fails, the deployment job fails.
