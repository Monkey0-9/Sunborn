# Sunborn showcase site

The public landing page and playground for Sunborn. It is a static Next.js
export, published to GitHub Pages by `.github/workflows/pages.yml`, and is
separate from the operator dashboard in `frontend/`.

- `/` explains what Sunborn does, how a request flows, and the adversarial
  benchmark results.
- `/playground/` lets visitors send prompts and watch the cache decide: exact
  hit, semantic hit, or refuse an unsafe reuse.

## Develop

```bash
cd site
npm install
npm run dev        # http://localhost:3001
npm test           # simulator unit tests
npm run lint       # type check
npm run build      # static export into site/out
```

To check the exact Pages build locally:

```bash
PAGES_BASE_PATH=/Sunborn npm run build
```

## Playground modes

**Simulation** runs entirely in the browser (`src/lib/sim-cache.mjs`). It
follows the same decision order as the gateway (exact match, semantic match,
guardrails, origin) with simplified matching, and labels its latency and cost
figures as illustrative. It needs no gateway and no API key, so it is what
public visitors see by default.

**Live gateway** sends prompts from the visitor's browser straight to a
Sunborn gateway they run themselves, through the OpenAI-compatible
`/v1/chat/completions` endpoint, and reads `sunborn_cached` and
`sunborn_tier` from the response. Nothing is proxied through GitHub, and no
provider key is ever in this site.

### Running Live mode against your own gateway

1. Put your provider key in `deploy/.env` (git ignores it). For Gemini:

   ```bash
   echo "GEMINI_API_KEY=your-key" >> deploy/.env
   ```

   Leave out any key you don't have instead of setting it empty. With no
   provider key the gateway answers from its built-in mock origin.

2. Start the stack from the repo root:

   ```bash
   docker compose -f deploy/docker-compose.yml up -d --build
   ```

3. Open the playground (Pages URL or `npm run dev`), switch to **Live
   gateway**, and fill in:
   - Gateway URL: `http://localhost:8080`
   - Sunborn API key: the gateway's `SUNBORN_ADMIN_KEY` for local testing (the
     compose file sets a development default), or a scoped key created with
     `POST /v1/auth/credentials`.
   - Model: `gemini-3.8-flash` or any other `gemini-*` model.

4. Click **Test connection**, then send a prompt twice.

### If Live mode can't connect

- **CORS:** the compose file allows `localhost:3000`, `localhost:3001` and
  `https://candiate32.github.io`. If the site is served from anywhere else,
  set `SUNBORN_CORS_ALLOWED_ORIGINS` in `deploy/.env`.
- **Local network permission:** browsers treat a public HTTPS page calling
  `http://localhost` specially. Chrome asks for permission to access devices
  on your local network; allow it. Firefox allows it. Safari blocks it, so
  use `npm run dev` locally instead.

## Publishing

In the repository's **Settings → Pages**, set **Source** to **GitHub
Actions** once. After that, every push to `main` that touches `site/` builds
and deploys the site to `https://candiate32.github.io/Sunborn/`.
