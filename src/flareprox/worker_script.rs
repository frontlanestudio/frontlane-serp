/// Embedded JavaScript code for the FlareProx Cloudflare Worker pass-through proxy.
pub const FLAREPROX_WORKER_JS: &str = r#"/**
 * FlareProx - Cloudflare Worker HTTP Forwarding Proxy
 * Built for Frontlane SERP
 */
addEventListener('fetch', event => {
  event.respondWith(handleRequest(event.request))
});

async function handleRequest(request) {
  try {
    const url = new URL(request.url);
    const targetUrl = getTargetUrl(url, request.headers);

    if (!targetUrl) {
      return createErrorResponse('No target URL specified', {
        usage: {
          query_param: '?url=https://example.com',
          header: 'X-Target-URL: https://example.com',
          path: '/https://example.com'
        }
      }, 400);
    }

    let targetURL;
    try {
      targetURL = new URL(targetUrl);
    } catch (e) {
      return createErrorResponse('Invalid target URL', { provided: targetUrl }, 400);
    }

    // Build target URL with remaining query parameters
    const targetParams = new URLSearchParams(targetURL.search);
    for (const [key, value] of url.searchParams) {
      if (!['url', '_cb', '_t'].includes(key)) {
        targetParams.append(key, value);
      }
    }
    const searchStr = targetParams.toString();
    if (searchStr) {
      targetURL.search = searchStr;
    }

    // Create proxied request
    const proxyRequest = createProxyRequest(request, targetURL);
    const response = await fetch(proxyRequest);

    // Process and return response
    return createProxyResponse(response, request.method);
  } catch (error) {
    return createErrorResponse('Proxy request failed', {
      message: error.message,
      timestamp: new Date().toISOString()
    }, 500);
  }
}

function getTargetUrl(url, headers) {
  // Priority: header > query param > path
  let targetUrl = headers.get('X-Target-URL') || headers.get('x-target-url');

  if (!targetUrl) {
    targetUrl = url.searchParams.get('url');
  }

  if (!targetUrl && url.pathname !== '/' && url.pathname.length > 1) {
    const pathUrl = url.pathname.slice(1);
    if (pathUrl.startsWith('http://') || pathUrl.startsWith('https://')) {
      targetUrl = pathUrl;
    }
  }

  return targetUrl;
}

function createProxyRequest(request, targetURL) {
  const proxyHeaders = new Headers();
  const allowedHeaders = [
    'accept', 'accept-language', 'accept-encoding', 'authorization',
    'cache-control', 'content-type', 'origin', 'referer', 'user-agent',
    'cookie', 'sec-ch-ua', 'sec-ch-ua-mobile', 'sec-ch-ua-platform',
    'sec-fetch-dest', 'sec-fetch-mode', 'sec-fetch-site', 'sec-fetch-user',
    'upgrade-insecure-requests'
  ];

  for (const [key, value] of request.headers) {
    const lowerKey = key.toLowerCase();
    if (allowedHeaders.includes(lowerKey)) {
      proxyHeaders.set(key, value);
    }
  }

  proxyHeaders.set('Host', targetURL.hostname);

  const customXff = request.headers.get('X-My-X-Forwarded-For') || request.headers.get('X-Forwarded-For');
  if (customXff) {
    proxyHeaders.set('X-Forwarded-For', customXff);
  } else {
    proxyHeaders.set('X-Forwarded-For', generateRandomIP());
  }

  return new Request(targetURL.toString(), {
    method: request.method,
    headers: proxyHeaders,
    body: ['GET', 'HEAD'].includes(request.method) ? null : request.body,
    redirect: 'follow'
  });
}

function createProxyResponse(response, requestMethod) {
  const responseHeaders = new Headers();

  for (const [key, value] of response.headers) {
    const lower = key.toLowerCase();
    if (!['content-encoding', 'content-length', 'transfer-encoding'].includes(lower)) {
      responseHeaders.set(key, value);
    }
  }

  responseHeaders.set('Access-Control-Allow-Origin', '*');
  responseHeaders.set('Access-Control-Allow-Methods', 'GET, POST, PUT, DELETE, OPTIONS, PATCH, HEAD');
  responseHeaders.set('Access-Control-Allow-Headers', '*');

  if (requestMethod === 'OPTIONS') {
    return new Response(null, { status: 204, headers: responseHeaders });
  }

  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers: responseHeaders
  });
}

function createErrorResponse(error, details, status) {
  return new Response(JSON.stringify({ error, ...details }), {
    status,
    headers: { 'Content-Type': 'application/json' }
  });
}

function generateRandomIP() {
  return [1, 2, 3, 4].map(() => Math.floor(Math.random() * 254) + 1).join('.');
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_script_contains_key_elements() {
        assert!(FLAREPROX_WORKER_JS.contains("addEventListener('fetch'"));
        assert!(FLAREPROX_WORKER_JS.contains("X-Target-URL"));
        assert!(FLAREPROX_WORKER_JS.contains("createProxyRequest"));
        assert!(FLAREPROX_WORKER_JS.contains("generateRandomIP"));
    }
}
