//! Free, key-less internet access for agents: DuckDuckGo (HTML/lite) + Wikipedia search,
//! optional self-hosted SearXNG, and page fetching converted to Markdown. Results are cached in SQLite.

use std::time::Duration;

use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;

const SEARCH_TTL_SECS: i64 = 60 * 60;
const FETCH_TTL_SECS: i64 = 6 * 60 * 60;
const BROWSER_UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub source: String,
}

#[derive(Debug, Clone, Default)]
pub struct WebConfig {
    /// When set, SearXNG is queried first.
    pub searxng_url: Option<String>,
}

#[derive(Clone)]
pub struct WebService {
    http: reqwest::Client,
    db: SqlitePool,
}

impl WebService {
    pub fn new(db: SqlitePool) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(BROWSER_UA)
            .timeout(Duration::from_secs(25))
            .redirect(reqwest::redirect::Policy::limited(8))
            .build()
            .expect("http client");
        Self { http, db }
    }

    pub async fn search(
        &self,
        query: &str,
        max: usize,
        cfg: &WebConfig,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let query = query.trim();
        anyhow::ensure!(!query.is_empty(), "empty search query");
        let max = max.clamp(1, 20);
        let key = format!(
            "search:{}:{max}:{query}",
            cfg.searxng_url.as_deref().unwrap_or("ddg")
        );
        if let Some(hit) = self.cache_get(&key, SEARCH_TTL_SECS).await {
            if let Ok(v) = serde_json::from_str(&hit) {
                return Ok(v);
            }
        }

        let mut results = Vec::new();
        let mut errors = Vec::new();
        if let Some(url) = cfg.searxng_url.as_deref().filter(|u| !u.trim().is_empty()) {
            match self.searxng(url, query).await {
                Ok(r) => results = r,
                Err(e) => errors.push(format!("searxng: {e}")),
            }
        }
        if results.is_empty() {
            match self.duckduckgo(query).await {
                Ok(r) => results = r,
                Err(e) => errors.push(format!("duckduckgo: {e}")),
            }
        }
        // Wikipedia fills gaps and is a reliable fallback when DDG rate-limits.
        if results.len() < max {
            match self.wikipedia(query, 3).await {
                Ok(r) => results.extend(
                    r.into_iter()
                        .filter(|w| !results.iter().any(|x| x.url == w.url))
                        .collect::<Vec<_>>(),
                ),
                Err(e) => errors.push(format!("wikipedia: {e}")),
            }
        }
        results.truncate(max);
        if results.is_empty() && !errors.is_empty() {
            anyhow::bail!("web search failed ({})", errors.join("; "));
        }
        self.cache_put(&key, &serde_json::to_string(&results)?)
            .await;
        Ok(results)
    }

    async fn duckduckgo(&self, query: &str) -> anyhow::Result<Vec<SearchResult>> {
        let url =
            url::Url::parse_with_params("https://html.duckduckgo.com/html/", &[("q", query)])?;
        let html = self
            .http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        let results = parse_ddg_html(&html);
        if !results.is_empty() {
            return Ok(results);
        }
        let url =
            url::Url::parse_with_params("https://lite.duckduckgo.com/lite/", &[("q", query)])?;
        let html = self
            .http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        Ok(parse_ddg_lite(&html))
    }

    async fn wikipedia(&self, query: &str, limit: usize) -> anyhow::Result<Vec<SearchResult>> {
        let url = url::Url::parse_with_params(
            "https://en.wikipedia.org/w/api.php",
            &[
                ("action", "query"),
                ("list", "search"),
                ("format", "json"),
                ("srsearch", query),
                ("srlimit", &limit.to_string()),
            ],
        )?;
        let v: Value = self
            .http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(v.pointer("/query/search")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| {
                        let title = r["title"].as_str()?;
                        Some(SearchResult {
                            title: title.to_string(),
                            url: format!(
                                "https://en.wikipedia.org/wiki/{}",
                                title.replace(' ', "_")
                            ),
                            snippet: strip_tags(r["snippet"].as_str().unwrap_or_default()),
                            source: "wikipedia".into(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn searxng(&self, base: &str, query: &str) -> anyhow::Result<Vec<SearchResult>> {
        let url = url::Url::parse_with_params(
            &format!("{}/search", base.trim_end_matches('/')),
            &[("q", query), ("format", "json")],
        )?;
        let v: Value = self
            .http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(v["results"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| {
                        Some(SearchResult {
                            title: r["title"].as_str()?.to_string(),
                            url: r["url"].as_str()?.to_string(),
                            snippet: r["content"].as_str().unwrap_or_default().to_string(),
                            source: "searxng".into(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Fetch a page and return Markdown. `offset`/`max_chars` page through long documents.
    pub async fn fetch(
        &self,
        url: &str,
        offset: usize,
        max_chars: usize,
    ) -> anyhow::Result<String> {
        let parsed = url::Url::parse(url)?;
        anyhow::ensure!(
            matches!(parsed.scheme(), "http" | "https"),
            "only http(s) URLs can be fetched"
        );
        let key = format!("fetch:{url}");
        let doc = match self.cache_get(&key, FETCH_TTL_SECS).await {
            Some(hit) => hit,
            None => {
                let resp = self.http.get(parsed).send().await?.error_for_status()?;
                let ctype = resp
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_lowercase();
                let body = resp.text().await?;
                let doc = if ctype.contains("html") || body.trim_start().starts_with('<') {
                    html_to_markdown(&body)
                } else if ctype.starts_with("text/")
                    || ctype.contains("json")
                    || ctype.contains("xml")
                    || ctype.is_empty()
                {
                    body
                } else {
                    anyhow::bail!("unsupported content type: {ctype}");
                };
                self.cache_put(&key, &doc).await;
                doc
            }
        };
        let total = doc.chars().count();
        let max_chars = max_chars.clamp(500, 40_000);
        let chunk: String = doc.chars().skip(offset).take(max_chars).collect();
        let end = offset + chunk.chars().count();
        let mut out = chunk;
        if end < total {
            out.push_str(&format!("\n\n[truncated: showing {offset}-{end} of {total} chars; call web_fetch with offset={end} for more]"));
        }
        Ok(out)
    }

    async fn cache_get(&self, key: &str, ttl_secs: i64) -> Option<String> {
        let row: Option<(String, String)> =
            sqlx::query_as("SELECT body, fetched_at FROM web_cache WHERE key = ?")
                .bind(key)
                .fetch_optional(&self.db)
                .await
                .ok()?;
        let (body, at) = row?;
        let at = chrono::DateTime::parse_from_rfc3339(&at).ok()?;
        (chrono::Utc::now().signed_duration_since(at).num_seconds() < ttl_secs).then_some(body)
    }

    async fn cache_put(&self, key: &str, body: &str) {
        let _ = sqlx::query(
            "INSERT OR REPLACE INTO web_cache (key, body, fetched_at) VALUES (?, ?, ?)",
        )
        .bind(key)
        .bind(body)
        .bind(crate::db::now())
        .execute(&self.db)
        .await;
    }
}

fn sel(s: &str) -> Selector {
    Selector::parse(s).expect("valid selector")
}

fn text_of(el: scraper::ElementRef) -> String {
    el.text()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// DDG wraps result links in a redirect (`//duckduckgo.com/l/?uddg=<target>`).
fn unwrap_ddg_link(href: &str) -> Option<String> {
    let absolute = if href.starts_with("//") {
        format!("https:{href}")
    } else {
        href.to_string()
    };
    let url = url::Url::parse(&absolute).ok()?;
    if url
        .host_str()
        .is_some_and(|h| h.ends_with("duckduckgo.com"))
    {
        if let Some((_, target)) = url.query_pairs().find(|(k, _)| k == "uddg") {
            return Some(target.into_owned());
        }
        return None; // ads and internal links
    }
    Some(absolute)
}

pub(crate) fn parse_ddg_html(html: &str) -> Vec<SearchResult> {
    let doc = Html::parse_document(html);
    let (result, link, snippet) = (
        sel("div.result"),
        sel("a.result__a"),
        sel(".result__snippet"),
    );
    doc.select(&result)
        .filter(|r| !r.value().classes().any(|c| c == "result--ad"))
        .filter_map(|r| {
            let a = r.select(&link).next()?;
            Some(SearchResult {
                title: text_of(a),
                url: unwrap_ddg_link(a.value().attr("href")?)?,
                snippet: r.select(&snippet).next().map(text_of).unwrap_or_default(),
                source: "duckduckgo".into(),
            })
        })
        .collect()
}

pub(crate) fn parse_ddg_lite(html: &str) -> Vec<SearchResult> {
    let doc = Html::parse_document(html);
    let (link, snippet) = (sel("a.result-link"), sel("td.result-snippet"));
    let snippets: Vec<String> = doc.select(&snippet).map(text_of).collect();
    doc.select(&link)
        .enumerate()
        .filter_map(|(i, a)| {
            Some(SearchResult {
                title: text_of(a),
                url: unwrap_ddg_link(a.value().attr("href")?)?,
                snippet: snippets.get(i).cloned().unwrap_or_default(),
                source: "duckduckgo".into(),
            })
        })
        .collect()
}

pub(crate) fn html_to_markdown(html: &str) -> String {
    let doc = Html::parse_document(html);
    let title = doc
        .select(&sel("title"))
        .next()
        .map(text_of)
        .unwrap_or_default();
    // Prefer the main content region when the page marks one.
    let body_html = ["main", "article", "[role=main]", "body"]
        .iter()
        .find_map(|s| doc.select(&sel(s)).next().map(|e| e.html()))
        .unwrap_or_else(|| html.to_string());
    let converter = htmd::HtmlToMarkdown::builder()
        .skip_tags(vec![
            "script", "style", "noscript", "svg", "nav", "footer", "header", "form", "iframe",
            "button",
        ])
        .build();
    let md = converter.convert(&body_html).unwrap_or_default();
    // Collapse runs of blank lines.
    let re = regex::Regex::new(r"\n{3,}").expect("regex");
    let md = re.replace_all(md.trim(), "\n\n").to_string();
    if title.is_empty() {
        md
    } else {
        format!("# {title}\n\n{md}")
    }
}

fn strip_tags(s: &str) -> String {
    let re = regex::Regex::new(r"<[^>]+>").expect("regex");
    re.replace_all(s, "")
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&#039;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ddg_html_results() {
        let html = r#"<div class="result results_links"><h2><a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fwww.rust-lang.org%2F&rut=x">Rust <b>Programming</b> Language</a></h2>
            <a class="result__snippet">A language empowering everyone.</a></div>
            <div class="result result--ad"><a class="result__a" href="//duckduckgo.com/y.js?ad=1">Ad</a></div>"#;
        let r = parse_ddg_html(html);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].url, "https://www.rust-lang.org/");
        assert_eq!(r[0].title, "Rust Programming Language");
        assert_eq!(r[0].snippet, "A language empowering everyone.");
    }

    #[test]
    fn converts_html_to_markdown() {
        let md = html_to_markdown("<html><head><title>T</title><script>x()</script></head><body><nav>menu</nav><main><h1>Hello</h1><p>World <a href='https://a.b'>link</a></p></main></body></html>");
        assert!(md.starts_with("# T"));
        assert!(md.contains("Hello"));
        assert!(md.contains("[link](https://a.b)"));
        assert!(!md.contains("menu"));
        assert!(!md.contains("x()"));
    }
}
