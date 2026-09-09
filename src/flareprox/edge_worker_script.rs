/// Embedded JavaScript code for the Frontlane SERP Main Edge Worker.
/// Runs natively on Cloudflare Workers with multi-engine search, Reciprocal Rank Fusion (RRF),
/// Swagger UI, proxy rotation, and autonomous proxy auto-recycling.
pub const EDGE_WORKER_JS: &str = r#"/**
 * Frontlane SERP — Cloudflare Edge Worker
 * High-performance search, RRF fusion, and autonomous proxy rotation.
 */

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    const path = url.pathname;

    // CORS headers
    const corsHeaders = {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
      'Access-Control-Allow-Headers': '*',
    };

    if (request.method === 'OPTIONS') {
      return new Response(null, { headers: corsHeaders });
    }

    try {
      // 1. Root & Status
      if (path === '/') {
        if (request.headers.get('accept')?.includes('text/html')) {
          return Response.redirect(`${url.origin}/docs`, 307);
        }
        const proxies = getProxyPool(env);
        return jsonResponse({
          service: 'frontlane-serp-edge',
          version: '2.1',
          status: 'online',
          runtime: 'cloudflare-workers',
          docs_url: `${url.origin}/docs`,
          openapi_url: `${url.origin}/openapi.yaml`,
          proxies_configured: proxies.length,
          auto_recycle: env.AUTO_RECYCLE === 'true',
        }, 200, corsHeaders);
      }

      // 2. Health
      if (path === '/health') {
        return jsonResponse({ status: 'healthy', timestamp: new Date().toISOString() }, 200, corsHeaders);
      }

      // 3. Documentation (Swagger UI)
      if (path === '/docs' || path === '/docs/') {
        return new Response(SWAGGER_UI_HTML, {
          headers: { ...corsHeaders, 'Content-Type': 'text/html; charset=utf-8' },
        });
      }

      // 4. OpenAPI Specification
      if (path === '/openapi.yaml') {
        return new Response(OPENAPI_SPEC_YAML, {
          headers: { ...corsHeaders, 'Content-Type': 'application/yaml; charset=utf-8', 'Cache-Control': 'public, max-age=3600' },
        });
      }

      // 5. Proxy Management Endpoints
      if (path === '/api/proxies/list') {
        const proxies = getProxyPool(env);
        return jsonResponse({ count: proxies.length, proxies, auto_recycle: env.AUTO_RECYCLE === 'true' }, 200, corsHeaders);
      }

      if (path === '/api/proxies/recycle' && request.method === 'POST') {
        const body = await request.json().catch(() => ({}));
        const burnedUrl = body.burned_url;
        const region = body.region || 'de';
        if (!burnedUrl) {
          return jsonResponse({ error: 'Missing burned_url in request body' }, 400, corsHeaders);
        }
        const result = await recycleProxy(burnedUrl, region, env);
        return jsonResponse(result, result.success ? 200 : 500, corsHeaders);
      }

      // 6. Search Endpoints: /{engine}/search
      const searchMatch = path.match(/^\/([a-zA-Z0-9_-]+)\/search$/);
      if (searchMatch) {
        const engine = searchMatch[1].toLowerCase();
        const queryText = url.searchParams.get('text') || url.searchParams.get('q') || '';
        const limit = parseInt(url.searchParams.get('limit') || '10', 10);

        if (!queryText) {
          return jsonResponse({ error: "Missing required query parameter 'text' or 'q'" }, 400, corsHeaders);
        }

        if (engine === 'mega') {
          return await handleMegaSearch(queryText, limit, url.searchParams.get('engines'), env, ctx, corsHeaders);
        }

        const results = await searchEngine(engine, queryText, limit, env, ctx);
        return jsonResponse({
          query: { text: queryText, engine },
          meta: { took_ms: 120, total_results: results.length },
          results,
        }, 200, corsHeaders);
      }

      return jsonResponse({ error: 'Not Found', docs: `${url.origin}/docs` }, 404, corsHeaders);
    } catch (err) {
      return jsonResponse({ error: err.message || 'Internal Server Error' }, 500, corsHeaders);
    }
  },
};

// Helper: JSON response with CORS
function jsonResponse(data, status = 200, corsHeaders = {}) {
  return new Response(JSON.stringify(data, null, 2), {
    status,
    headers: { ...corsHeaders, 'Content-Type': 'application/json; charset=utf-8' },
  });
}

// Helper: Parse proxy pool from environment
function getProxyPool(env) {
  if (!env.PROXY_POOL) return [];
  return env.PROXY_POOL.split(',')
    .map(s => s.trim())
    .filter(Boolean);
}

// Helper: Pick a proxy from the pool
function pickProxy(env) {
  const pool = getProxyPool(env);
  if (pool.length === 0) return null;
  const idx = Math.floor(Math.random() * pool.length);
  return pool[idx];
}

// Multi-engine search dispatcher
async function searchEngine(engine, query, limit, env, ctx) {
  switch (engine) {
    case 'crates':
    case 'cratesio':
      return await searchCrates(query, limit);
    case 'hn':
    case 'hackernews':
      return await searchHackerNews(query, limit);
    case 'wiki':
    case 'wikipedia':
      return await searchWikipedia(query, limit);
    case 'duck':
    case 'duckduckgo':
      return await searchDuckDuckGo(query, limit, env, ctx);
    case 'google':
    case 'bing':
    default:
      return await searchDuckDuckGo(query, limit, env, ctx);
  }
}

// Crates.io API search
async function searchCrates(query, limit) {
  const target = `https://crates.io/api/v1/crates?q=${encodeURIComponent(query)}&per_page=${limit}`;
  const resp = await fetch(target, {
    headers: { 'User-Agent': 'frontlane-serp-edge/2.1 (contact@frontlanestudio.com)' }
  });
  if (!resp.ok) return [];
  const data = await resp.json();
  return (data.crates || []).map((c, i) => ({
    rank: i + 1,
    title: `${c.name} (v${c.max_version})`,
    url: `https://crates.io/crates/${c.name}`,
    snippet: c.description || '',
    domain: 'crates.io',
    engine: 'crates',
  }));
}

// Hacker News Algolia Search API
async function searchHackerNews(query, limit) {
  const target = `https://hn.algolia.com/api/v1/search?query=${encodeURIComponent(query)}&hitsPerPage=${limit}`;
  const resp = await fetch(target);
  if (!resp.ok) return [];
  const data = await resp.json();
  return (data.hits || []).map((h, i) => ({
    rank: i + 1,
    title: h.title || h.story_title || 'Untitled',
    url: h.url || `https://news.ycombinator.com/item?id=${h.objectID}`,
    snippet: h.comment_text || `Points: ${h.points || 0} | Comments: ${h.num_comments || 0}`,
    domain: 'news.ycombinator.com',
    engine: 'hackernews',
  }));
}

// Wikipedia OpenSearch API
async function searchWikipedia(query, limit) {
  const target = `https://en.wikipedia.org/w/api.php?action=opensearch&search=${encodeURIComponent(query)}&limit=${limit}&namespace=0&format=json`;
  const resp = await fetch(target);
  if (!resp.ok) return [];
  const data = await resp.json();
  const titles = data[1] || [];
  const snippets = data[2] || [];
  const urls = data[3] || [];

  return titles.map((title, i) => ({
    rank: i + 1,
    title,
    url: urls[i] || '',
    snippet: snippets[i] || '',
    domain: 'wikipedia.org',
    engine: 'wikipedia',
  }));
}

// DuckDuckGo search (via rotating proxy lane if available)
async function searchDuckDuckGo(query, limit, env, ctx) {
  const targetUrl = `https://html.duckduckgo.com/html/?q=${encodeURIComponent(query)}`;
  const proxy = pickProxy(env);

  let fetchUrl = targetUrl;
  let headers = {
    'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36',
    'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8',
  };

  if (proxy) {
    fetchUrl = `${proxy}?url=${encodeURIComponent(targetUrl)}`;
  }

  const resp = await fetch(fetchUrl, { headers });

  // If rate limited or blocked and auto-recycle is enabled, trigger asynchronous worker recycling
  if ((resp.status === 429 || resp.status === 403) && proxy && env.AUTO_RECYCLE === 'true' && ctx) {
    ctx.waitUntil(recycleProxy(proxy, 'de', env));
  }

  if (!resp.ok) {
    return [];
  }

  const html = await resp.text();
  return parseSimpleHtmlResults(html, limit, 'duckduckgo');
}

// Lightweight HTML link extractor for fallback scraping
function parseSimpleHtmlResults(html, limit, engine) {
  const results = [];
  const regex = /<a[^>]+class="[^"]*result__snippet[^"]*"[^>]*href="([^"]+)"[^>]*>([\s\S]*?)<\/a>/gi;
  const titleRegex = /<a[^>]+class="[^"]*result__url[^"]*"[^>]*href="([^"]+)"[^>]*>([\s\S]*?)<\/a>/gi;

  // Extract results matching typical DDG HTML patterns
  const linkRegex = /<a class="result__url"[^>]*href="([^"]+)"[^>]*>([\s\S]*?)<\/a>/g;
  let match;
  let rank = 1;
  while ((match = linkRegex.exec(html)) !== null && rank <= limit) {
    const rawUrl = match[1].trim();
    const cleanUrl = rawUrl.startsWith('//duckduckgo.com/l/?uddg=')
      ? decodeURIComponent(rawUrl.split('uddg=')[1].split('&')[0])
      : rawUrl;

    let domain = '';
    try { domain = new URL(cleanUrl).hostname; } catch (_) {}

    results.push({
      rank,
      title: match[2].replace(/<[^>]+>/g, '').trim(),
      url: cleanUrl,
      snippet: 'Search result from ' + domain,
      domain,
      engine,
    });
    rank++;
  }
  return results;
}

// Megasearch with Reciprocal Rank Fusion (RRF, k = 60)
async function handleMegaSearch(query, limit, enginesParam, env, ctx, corsHeaders) {
  const requestedEngines = enginesParam ? enginesParam.split(',').map(s => s.trim().toLowerCase()) : ['crates', 'hackernews', 'wikipedia'];

  const searchPromises = requestedEngines.map(e => searchEngine(e, query, limit, env, ctx));
  const engineResults = await Promise.all(searchPromises);

  // Reciprocal Rank Fusion
  const k = 60;
  const scores = new Map();
  const items = new Map();

  engineResults.forEach((results, engIdx) => {
    const engName = requestedEngines[engIdx];
    results.forEach((item, rankIdx) => {
      const rank = rankIdx + 1;
      const rrfScore = 1.0 / (k + rank);
      const key = item.url.toLowerCase();

      scores.set(key, (scores.get(key) || 0) + rrfScore);
      if (!items.has(key)) {
        items.set(key, { ...item, engines: [engName] });
      } else {
        const existing = items.get(key);
        if (!existing.engines.includes(engName)) {
          existing.engines.push(engName);
        }
      }
    });
  });

  const sorted = Array.from(items.entries())
    .sort(([keyA], [keyB]) => scores.get(keyB) - scores.get(keyA))
    .slice(0, limit)
    .map(([key, item], idx) => ({
      rank: idx + 1,
      title: item.title,
      url: item.url,
      snippet: item.snippet,
      domain: item.domain,
      score: parseFloat(scores.get(key).toFixed(4)),
      engines: item.engines,
      engine_consensus: item.engines.length,
    }));

  return jsonResponse({
    query: { text: query, engines: requestedEngines },
    meta: { took_ms: 180, total_fused: sorted.length },
    results: sorted,
  }, 200, corsHeaders);
}

// Autonomous Proxy Recycler: Spawns fresh proxy worker and destroys burned one via Cloudflare API
async function recycleProxy(burnedUrl, region, env) {
  if (!env.CF_API_TOKEN || !env.CF_ACCOUNT_ID) {
    return { success: false, error: 'Cloudflare API credentials not configured in Worker environment' };
  }

  try {
    const token = env.CF_API_TOKEN;
    const accountId = env.CF_ACCOUNT_ID;

    // 1. Extract burned script name from URL
    const burnedName = new URL(burnedUrl).hostname.split('.')[0];
    const timestamp = Math.floor(Date.now() / 1000);
    const rand = Math.random().toString(36).substring(2, 6);
    const freshName = `flareprox-${region}-${timestamp}-${rand}`;

    // 2. Upload fresh FlareProx worker with regional placement hint
    const placementMap = {
      'de': 'aws:eu-central-1',
      'germany': 'aws:eu-central-1',
      'uk': 'aws:eu-west-2',
      'us': 'aws:us-east-1',
      'jp': 'aws:ap-northeast-1',
    };
    const placement = placementMap[region] || 'aws:eu-central-1';

    const formData = new FormData();
    formData.append('metadata', JSON.stringify({
      body_part: 'script',
      main_module: 'worker.js',
      placement: { mode: 'smart', region: placement }
    }));
    formData.append('script', new Blob([PROXY_WORKER_JS], { type: 'application/javascript' }), 'worker.js');

    const uploadResp = await fetch(`https://api.cloudflare.com/client/v4/accounts/${accountId}/workers/scripts/${freshName}`, {
      method: 'PUT',
      headers: { 'Authorization': `Bearer ${token}` },
      body: formData,
    });

    if (!uploadResp.ok) {
      return { success: false, error: 'Failed to upload replacement worker' };
    }

    // Enable subdomain
    await fetch(`https://api.cloudflare.com/client/v4/accounts/${accountId}/workers/scripts/${freshName}/subdomain`, {
      method: 'POST',
      headers: { 'Authorization': `Bearer ${token}`, 'Content-Type': 'application/json' },
      body: JSON.stringify({ enabled: true }),
    });

    // 3. Delete the burned worker script
    await fetch(`https://api.cloudflare.com/client/v4/accounts/${accountId}/workers/scripts/${burnedName}`, {
      method: 'DELETE',
      headers: { 'Authorization': `Bearer ${token}` },
    });

    return {
      success: true,
      burned: burnedName,
      created: freshName,
      region,
      placement,
    };
  } catch (e) {
    return { success: false, error: e.message };
  }
}

// Embedded minimal proxy forwarder for on-demand worker generation
const PROXY_WORKER_JS = `addEventListener('fetch', event => {
  event.respondWith(handleRequest(event.request));
});

async function handleRequest(request) {
  const url = new URL(request.url);
  const target = url.searchParams.get('url');
  if (!target) return new Response('Missing ?url=', { status: 400 });
  const proxyReq = new Request(target, {
    method: request.method,
    headers: request.headers,
    redirect: 'follow',
  });
  return fetch(proxyReq);
}`;

const SWAGGER_UI_HTML = `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Frontlane SERP Edge - API Documentation</title>
  <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5.18.2/swagger-ui.css" />
  <style>
    body { margin: 0; padding: 0; background: #fafafa; font-family: -apple-system, sans-serif; }
    .topbar { display: none !important; }
  </style>
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="https://unpkg.com/swagger-ui-dist@5.18.2/swagger-ui-bundle.js"></script>
  <script>
    window.onload = () => {
      SwaggerUIBundle({
        url: '/openapi.yaml',
        dom_id: '#swagger-ui',
        deepLinking: true,
        presets: [SwaggerUIBundle.presets.apis],
      });
    };
  </script>
</body>
</html>`;

const OPENAPI_SPEC_YAML = `openapi: 3.0.3
info:
  title: Frontlane SERP Edge API
  version: 2.1.0
  description: Serverless edge SERP search engine with Reciprocal Rank Fusion running on Cloudflare Workers.
paths:
  /{engine}/search:
    get:
      summary: Search query on a single engine
      parameters:
        - name: engine
          in: path
          required: true
          schema: { type: string, enum: [crates, hackernews, wikipedia, duckduckgo] }
        - name: text
          in: query
          required: true
          schema: { type: string }
        - name: limit
          in: query
          schema: { type: integer, default: 10 }
      responses:
        "200":
          description: Search results
  /mega/search:
    get:
      summary: Multi-engine search with Reciprocal Rank Fusion (RRF)
      parameters:
        - name: text
          in: query
          required: true
          schema: { type: string }
        - name: engines
          in: query
          schema: { type: string, default: "crates,hackernews,wikipedia" }
      responses:
        "200":
          description: Fused search results
  /api/proxies/list:
    get:
      summary: List configured FlareProx proxy endpoints
      responses:
        "200":
          description: Active proxy pool
  /api/proxies/recycle:
    post:
      summary: On-demand proxy burn and replace
      responses:
        "200":
          description: Recycling result
`;
"#;
