/// URL detection for terminal grid text.
///
/// Scans a line of text for `http://` and `https://` URLs and returns their
/// column spans. Used by the G3 Ctrl+Click-to-open feature.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlSpan {
    pub col_start: usize,
    pub col_end: usize, // inclusive
    pub url: String,
}

/// Detect all `http://` and `https://` URLs in a single line of text.
///
/// Returns a list of `UrlSpan` with column ranges (inclusive) and the URL string.
/// Trailing punctuation (`.`, `,`, `;`, `:`, `!`, `?`, `"`, `'`, `)`, `]`) is stripped
/// when it appears at the very end of a URL, as it is almost always sentence punctuation
/// rather than part of the URL itself.
pub fn detect_urls(line: &str) -> Vec<UrlSpan> {
    let mut results = Vec::new();
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Look for "http://" or "https://"
        let remaining = &line[i..];
        let scheme_len = if remaining.starts_with("https://") {
            8
        } else if remaining.starts_with("http://") {
            7
        } else {
            i += 1;
            continue;
        };

        // Must have at least one URL char after the scheme
        let url_start = i;
        let mut url_end = i + scheme_len;

        while url_end < len && is_url_char(bytes[url_end] as char) {
            url_end += 1;
        }

        // url_end is now one past the last URL char
        if url_end == i + scheme_len {
            // No chars after scheme — not a real URL
            i = url_end;
            continue;
        }

        // Strip trailing punctuation that is likely sentence-level, not part of the URL
        while url_end > i + scheme_len && is_trailing_punct(bytes[url_end - 1] as char) {
            url_end -= 1;
        }

        let url = &line[url_start..url_end];
        results.push(UrlSpan {
            col_start: url_start,
            col_end: url_end - 1, // inclusive
            url: url.to_string(),
        });

        i = url_end;
    }

    results
}

/// Return the URL at the given column, or `None` if the column is not inside a URL.
pub fn url_at_col(line: &str, col: usize) -> Option<String> {
    detect_urls(line)
        .into_iter()
        .find(|span| col >= span.col_start && col <= span.col_end)
        .map(|span| span.url)
}

fn is_url_char(c: char) -> bool {
    matches!(c,
        'a'..='z' | 'A'..='Z' | '0'..='9'
        | '-' | '.' | '_' | '~' | ':' | '/' | '?' | '#'
        | '[' | ']' | '@' | '!' | '$' | '&' | '\'' | '('
        | ')' | '*' | '+' | ',' | ';' | '=' | '%'
    )
}

fn is_trailing_punct(c: char) -> bool {
    matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | '"' | '\'' | ')' | ']')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_simple_http() {
        let spans = detect_urls("visit http://example.com for info");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].url, "http://example.com");
        assert_eq!(spans[0].col_start, 6);
        assert_eq!(spans[0].col_end, 23);
    }

    #[test]
    fn detect_simple_https() {
        let spans = detect_urls("go to https://example.com/path");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].url, "https://example.com/path");
    }

    #[test]
    fn detect_multiple_urls() {
        let spans = detect_urls("http://a.com and https://b.com");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].url, "http://a.com");
        assert_eq!(spans[1].url, "https://b.com");
    }

    #[test]
    fn strip_trailing_period() {
        let spans = detect_urls("See https://example.com.");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].url, "https://example.com");
    }

    #[test]
    fn strip_trailing_comma() {
        let spans = detect_urls("Visit https://example.com, then");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].url, "https://example.com");
    }

    #[test]
    fn strip_trailing_paren() {
        let spans = detect_urls("(see https://example.com)");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].url, "https://example.com");
    }

    #[test]
    fn preserve_query_string() {
        let spans = detect_urls("https://example.com/search?q=hello&lang=en");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].url, "https://example.com/search?q=hello&lang=en");
    }

    #[test]
    fn preserve_fragment() {
        let spans = detect_urls("https://example.com/page#section");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].url, "https://example.com/page#section");
    }

    #[test]
    fn preserve_port() {
        let spans = detect_urls("http://localhost:8080/api");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].url, "http://localhost:8080/api");
    }

    #[test]
    fn url_with_percent_encoding() {
        let spans = detect_urls("https://example.com/path%20with%20spaces");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].url, "https://example.com/path%20with%20spaces");
    }

    #[test]
    fn no_urls() {
        let spans = detect_urls("just some plain text");
        assert!(spans.is_empty());
    }

    #[test]
    fn empty_line() {
        let spans = detect_urls("");
        assert!(spans.is_empty());
    }

    #[test]
    fn bare_scheme_no_url() {
        let spans = detect_urls("http:// is not a url");
        assert!(spans.is_empty());
    }

    #[test]
    fn url_at_col_hit() {
        let line = "visit https://example.com ok";
        assert_eq!(url_at_col(line, 6), Some("https://example.com".to_string()));
        assert_eq!(url_at_col(line, 15), Some("https://example.com".to_string()));
        assert_eq!(url_at_col(line, 24), Some("https://example.com".to_string()));
    }

    #[test]
    fn url_at_col_miss() {
        let line = "visit https://example.com ok";
        assert_eq!(url_at_col(line, 0), None);
        assert_eq!(url_at_col(line, 5), None);
        assert_eq!(url_at_col(line, 26), None);
    }

    #[test]
    fn url_at_start_of_line() {
        let spans = detect_urls("https://start.com rest");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].col_start, 0);
        assert_eq!(spans[0].url, "https://start.com");
    }

    #[test]
    fn url_at_end_of_line() {
        let spans = detect_urls("text https://end.com");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].url, "https://end.com");
    }

    #[test]
    fn complex_github_url() {
        let spans = detect_urls("https://github.com/user/repo/issues/123#issuecomment-456");
        assert_eq!(spans.len(), 1);
        assert_eq!(
            spans[0].url,
            "https://github.com/user/repo/issues/123#issuecomment-456"
        );
    }
}
