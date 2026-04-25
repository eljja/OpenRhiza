# OpenRhiza Nexus

`openrhiza-nexus` is the first server-side program intended to back `openrhiza.com`.

It serves two roles:

- machine-oriented APIs for OpenRhiza OS nodes
- a simple human-facing read-only site for operators and observers

## Included APIs

- `GET /api/health`
- `POST /api/v1/node/register`
- `POST /api/v1/node/heartbeat`
- `POST /api/v1/hardware/report`
- `POST /api/v1/driver/query`
- `POST /api/v1/skill/query`
- `POST /api/v1/skill/download`
- `POST /api/v1/workflow/query`
- `POST /api/v1/software/query`
- `POST /api/v1/llm/query`
- `GET /api/v1/llm/google/models`
- `POST /api/v1/llm/generate`
- `POST /api/v1/evaluation/upload`

## Ubuntu deployment

### 1. Install runtime

```bash
curl -fsSL https://deb.nodesource.com/setup_22.x | sudo -E bash -
sudo apt-get install -y nodejs
node -v
npm -v
```

### 2. Install dependencies

```bash
cd openrhiza-nexus
npm ci
cp .env.example .env
```

### 3. Build and run

```bash
npm run build
HOSTNAME=0.0.0.0 PORT=3000 npm start
```

The server will listen on `http://0.0.0.0:3000`.

## SQLite storage

The registry now uses a local SQLite database through Node's built-in `node:sqlite`.

Default path:

```bash
./data/openrhiza.db
```

Override with:

```bash
OPENRHIZA_DB_PATH=/absolute/path/to/openrhiza.db
```

The first boot seeds the database with initial drivers, software, models, nodes, and evaluations.

## Google Gemini integration

If you want `openrhiza.com` to proxy requests to Google Gemini, set:

```bash
GOOGLE_GEMINI_API_KEY=your_api_key
OPENRHIZA_GEMINI_MODEL=gemini-2.5-flash
```

Relevant routes:

- `GET /api/v1/llm/google/models`
- `POST /api/v1/llm/generate`

Example request:

```bash
curl -X POST http://127.0.0.1:3000/api/v1/llm/generate \
  -H "Content-Type: application/json" \
  -d '{
    "protocol_version":"v1",
    "node_id":"demo-node",
    "provider":"google",
    "prompt":"Explain what an e1000 driver does in a text-only OS.",
    "system_instruction":"Be concise and technical."
  }'
```

## Reverse proxy

Use Nginx or Caddy in front of this service for TLS termination and public domain routing.

Suggested public routes:

- `https://openrhiza.com/`
- `https://openrhiza.com/api/health`
- `https://openrhiza.com/api/v1/...`

## systemd

A sample unit file is provided at:

- `deploy/openrhiza-nexus.service`

Example install path:

```bash
sudo mkdir -p /opt/openrhiza
sudo cp -r openrhiza-nexus /opt/openrhiza/
sudo cp /opt/openrhiza/openrhiza-nexus/deploy/openrhiza-nexus.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now openrhiza-nexus
```

## Docker

Build and run:

```bash
docker build -t openrhiza-nexus .
docker run --rm -p 3000:3000 --env-file .env openrhiza-nexus
```

## Notes

- This is still an early registry service, not the final production backend.
- The current API handlers return deterministic mock or reference data.
- The intended next step is to back these handlers with persistent storage and real artifact delivery.

