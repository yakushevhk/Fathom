// Zero-dependency web search provider with DuckDuckGo HTML parser and user-agent rotation.
// Used by agents-proxy to give bots live web search capabilities.

export interface WebSearchResult {
  title: string;
  url: string;
  snippet: string;
}

export async function executeWebSearch(query: string, limit = 5): Promise<WebSearchResult[]> {
  const trimmed = query.trim();
  if (!trimmed) return [];
  const maxResults = Math.min(10, Math.max(1, limit));

  // Primary: DuckDuckGo HTML endpoint
  try {
    const params = new URLSearchParams({ q: trimmed });
    const response = await fetch(`https://html.duckduckgo.com/html/?${params.toString()}`, {
      method: "POST",
      headers: {
        "User-Agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
        "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        "Accept-Language": "en-US,en;q=0.9,ru;q=0.8",
        "Content-Type": "application/x-www-form-urlencoded",
      },
      body: params.toString(),
      signal: AbortSignal.timeout(10_000),
    });

    if (!response.ok) {
      throw new Error(`HTTP ${response.status} ${response.statusText}`);
    }

    const html = await response.text();
    const results: WebSearchResult[] = [];

    // Extract search result blocks using robust regex pattern
    // DuckDuckGo HTML structure: class="result__body" ... class="result__title" ... class="result__snippet"
    const resultBlocks = html.split(/class="result\s+results_links/gi).slice(1);

    for (const block of resultBlocks) {
      if (results.length >= maxResults) break;

      // Extract URL & Title
      const urlMatch = block.match(/href="([^"]+)"[^>]*class="result__url"[^>]*>([\s\S]*?)<\/a>/i) ||
                       block.match(/<a[^>]*class="result__snippet"[^>]*href="([^"]+)"/i) ||
                       block.match(/<a[^>]*class="result__url"[^>]*href="([^"]+)"/i) ||
                       block.match(/href="(\/\/duckduckgo\.com\/l\/\?[^"]+)"/i);

      const titleMatch = block.match(/<a[^>]*class="result__a"[^>]*>([\s\S]*?)<\/a>/i);
      const snippetMatch = block.match(/<a[^>]*class="result__snippet"[^>]*>([\s\S]*?)<\/a>/i);

      let targetUrl = "";
      if (urlMatch && urlMatch[1]) {
        let rawHref = urlMatch[1];
        if (rawHref.includes("uddg=")) {
          try {
            const parsed = new URL(rawHref, "https://html.duckduckgo.com");
            targetUrl = decodeURIComponent(parsed.searchParams.get("uddg") || "");
          } catch {
            targetUrl = rawHref;
          }
        } else {
          targetUrl = rawHref.startsWith("//") ? `https:${rawHref}` : rawHref;
        }
      }

      const rawTitle = titleMatch ? titleMatch[1].replace(/<[^>]+>/g, "").trim() : "";
      const rawSnippet = snippetMatch ? snippetMatch[1].replace(/<[^>]+>/g, "").trim() : "";

      if (targetUrl && (rawTitle || rawSnippet)) {
        results.push({
          title: cleanHtmlEntities(rawTitle),
          url: targetUrl,
          snippet: cleanHtmlEntities(rawSnippet),
        });
      }
    }

    if (results.length > 0) {
      return results;
    }
  } catch {
    // DuckDuckGo HTML blocked or timed out, attempt JSON instant answers fallback
  }

  // Fallback: DuckDuckGo Instant Answer API
  try {
    const params = new URLSearchParams({ q: trimmed, format: "json", no_html: "1", skip_disambig: "1" });
    const response = await fetch(`https://api.duckduckgo.com/?${params.toString()}`, {
      headers: { "User-Agent": "Parallel/1.0" },
      signal: AbortSignal.timeout(8_000),
    });

    if (response.ok) {
      const data = (await response.json()) as {
        AbstractText?: string;
        AbstractURL?: string;
        Heading?: string;
        RelatedTopics?: Array<{ Text?: string; FirstURL?: string }>;
      };

      const results: WebSearchResult[] = [];
      if (data.AbstractText && data.AbstractURL) {
        results.push({
          title: data.Heading || trimmed,
          url: data.AbstractURL,
          snippet: data.AbstractText,
        });
      }
      if (Array.isArray(data.RelatedTopics)) {
        for (const topic of data.RelatedTopics) {
          if (results.length >= maxResults) break;
          if (topic.Text && topic.FirstURL) {
            results.push({
              title: topic.Text.slice(0, 60),
              url: topic.FirstURL,
              snippet: topic.Text,
            });
          }
        }
      }
      if (results.length > 0) return results;
    }
  } catch {
    // Both failed
  }

  return [];
}

function cleanHtmlEntities(str: string): string {
  return str
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&nbsp;/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}
