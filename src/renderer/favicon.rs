pub fn declared_url(page_url: &str, html: &str) -> Option<String> {
    let href = favicon_href_from_html(html)?;
    let page = url::Url::parse(page_url).ok()?;
    let base = html_tags(html, "base")
        .find_map(|tag| tag_attribute(tag, "href"))
        .and_then(|href| page.join(&href).ok())
        .unwrap_or(page);
    base.join(&href).ok().map(Into::into)
}

pub fn fallback_url(page_url: &str) -> Option<String> {
    let page = url::Url::parse(page_url).ok()?;
    if !matches!(page.scheme(), "http" | "https") {
        return None;
    }
    page.join("/favicon.ico").ok().map(Into::into)
}

fn favicon_href_from_html(html: &str) -> Option<String> {
    for tag in html_tags(html, "link") {
        if tag_attribute(tag, "rel").is_some_and(|rel| rel_declares_favicon(&rel))
            && let Some(href) = tag_attribute(tag, "href").filter(|href| !href.is_empty())
        {
            return Some(href);
        }
    }
    None
}

fn html_tags<'a>(html: &'a str, name: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    let lowercase = html.to_ascii_lowercase();
    let needle = format!("<{name}");
    let mut offset = 0;

    std::iter::from_fn(move || {
        while let Some(relative_start) = lowercase[offset..].find(&needle) {
            let start = offset + relative_start;
            let name_end = start + needle.len();
            let boundary = lowercase.as_bytes().get(name_end).copied();
            offset = name_end;
            if !boundary
                .is_some_and(|byte| byte.is_ascii_whitespace() || matches!(byte, b'/' | b'>'))
            {
                continue;
            }
            let relative_end = lowercase[name_end..].find('>')?;
            let end = name_end + relative_end + 1;
            offset = end;
            return Some(&html[start..end]);
        }
        None
    })
}

fn tag_attribute(tag: &str, wanted: &str) -> Option<String> {
    let bytes = tag.as_bytes();
    let mut index = tag.find(char::is_whitespace)?;

    while index < bytes.len() {
        while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
            index += 1;
        }
        if bytes
            .get(index)
            .is_none_or(|byte| matches!(byte, b'/' | b'>'))
        {
            break;
        }

        let name_start = index;
        while bytes
            .get(index)
            .is_some_and(|byte| !byte.is_ascii_whitespace() && !matches!(byte, b'=' | b'/' | b'>'))
        {
            index += 1;
        }
        let name = &tag[name_start..index];
        while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
            index += 1;
        }
        if bytes.get(index) != Some(&b'=') {
            continue;
        }
        index += 1;
        while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
            index += 1;
        }

        let quote = bytes
            .get(index)
            .copied()
            .filter(|byte| matches!(byte, b'\'' | b'"'));
        if quote.is_some() {
            index += 1;
        }
        let value_start = index;
        while bytes.get(index).is_some_and(|byte| {
            quote.map_or_else(
                || !byte.is_ascii_whitespace() && *byte != b'>',
                |quote| *byte != quote,
            )
        }) {
            index += 1;
        }
        let value = &tag[value_start..index];
        if quote.is_some() {
            index += 1;
        }
        if name.eq_ignore_ascii_case(wanted) {
            return Some(value.replace("&amp;", "&"));
        }
    }
    None
}

fn rel_declares_favicon(rel: &str) -> bool {
    rel.split_ascii_whitespace()
        .any(|token| token.eq_ignore_ascii_case("icon"))
}

#[cfg(test)]
mod tests {
    use super::{declared_url, fallback_url, favicon_href_from_html, rel_declares_favicon};

    #[test]
    fn derives_the_conventional_favicon_url_from_the_page_origin() {
        assert_eq!(
            fallback_url("https://www.google.com/search?q=wind"),
            Some("https://www.google.com/favicon.ico".to_string())
        );
        assert_eq!(
            fallback_url("http://localhost:3000/dashboard"),
            Some("http://localhost:3000/favicon.ico".to_string())
        );
        assert_eq!(fallback_url("arc://new-tab"), None);
    }

    #[test]
    fn recognizes_html_favicon_link_relations() {
        assert!(rel_declares_favicon("icon"));
        assert!(rel_declares_favicon("shortcut ICON"));
        assert!(!rel_declares_favicon("apple-touch-icon"));
        assert!(!rel_declares_favicon("stylesheet"));
    }

    #[test]
    fn discovers_and_resolves_a_declared_favicon() {
        let html = r#"
            <html><head>
                <base href="https://cdn.example.com/assets/">
                <link href='icons/app.png?v=2&amp;size=64' rel='shortcut ICON'>
            </head></html>
        "#;

        assert_eq!(
            declared_url("https://example.com/dashboard", html),
            Some("https://cdn.example.com/assets/icons/app.png?v=2&size=64".to_string())
        );
        assert_eq!(
            favicon_href_from_html("<link rel=icon href=/favicon.ico>"),
            Some("/favicon.ico".to_string())
        );
    }
}
