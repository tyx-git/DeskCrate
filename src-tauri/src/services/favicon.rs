// 网址图标的抓取与整理。
//
// 添加网址类应用时，把站点自己的图标抓下来存进 `icons/{app_id}.png`，让它和
// 本地程序在侧栏里长得一样。流程分三步：
//
//   1. 取首页 HTML，从 `<link rel="…icon…">` 里挑出候选地址；
//   2. 依次下载候选，谁能解出图片就用谁；都失败就退回站点根目录的
//      `/favicon.ico`；
//   3. 解码、缩到统一上限，交给图标提取器按既有约定存成 PNG。
//
// 整个过程**允许失败**：抓不到图标只是回到内置的那个地球图标，不应该让
// 「添加一个网址」整件事失败。
//
// URL 相关的几个函数（`split_url` / `resolve_reference` / `icon_candidates`）
// 都是不碰网络、不碰 Windows API 的纯函数，测试层可以直接覆盖。

use base64::Engine;
use image::imageops::{self, FilterType};
use image::RgbaImage;

use crate::{
    error::AppResult,
    models::AppError,
    system::http::{self, UrlParts},
};

// 一次抓取的预算。
//
// 首页 HTML 只要前面一截就够了（图标声明都在 `<head>` 里），256KB 足够；
// 单个图标 1MB 已经远远超过任何 favicon 的合理大小。
const PAGE_MAX_BYTES: usize = 256 * 1024;
const ICON_MAX_BYTES: usize = 1024 * 1024;
const TIMEOUT_MS: i32 = 3000;

// 最多尝试几个候选地址。站点可能声明一堆不同尺寸的图标，逐个试下去会把
// 「添加一个网址」拖成好几秒的等待。
const MAX_CANDIDATES: usize = 3;

// 图标缩放后的最长边。128 已经足够侧栏那个 44px 的格子在高 DPI 屏上清晰。
const ICON_MAX_EDGE: u32 = 128;

/// 抓取 `url` 的站点图标，返回可直接存成 PNG 的图像。
pub(crate) fn fetch_icon(url: &str) -> AppResult<RgbaImage> {
    let candidates = collect_candidates(url);

    let mut last_error = AppError::new("favicon_unavailable", "没有可用的网址图标".to_owned());

    for candidate in candidates.iter().take(MAX_CANDIDATES) {
        match fetch_image(candidate) {
            Ok(image) => return Ok(image),
            Err(error) => {
                log::debug!("网址图标候选 {} 不可用: {}", candidate, error.message);
                last_error = error;
            }
        }
    }

    Err(last_error)
}

/// 组装候选地址：`<link rel=icon>` 里声明的优先，站点根目录的
/// `/favicon.ico` 兜底。
fn collect_candidates(url: &str) -> Vec<String> {
    let mut candidates = Vec::new();

    if let Some(parts) = split_url(url) {
        if let Ok(page) = http::get(&parts, PAGE_MAX_BYTES, TIMEOUT_MS) {
            if page.status == 200 {
                let html = String::from_utf8_lossy(&page.body);
                candidates.extend(icon_candidates(&html, url));
            }
        }
    }

    candidates.push(origin_favicon(url));
    candidates
}

/// 下载（或就地解码）一个候选地址，成功则返回图像。
fn fetch_image(candidate: &str) -> AppResult<RgbaImage> {
    if let Some(image) = decode_data_uri(candidate) {
        return Ok(image);
    }

    let parts = split_url(candidate).ok_or_else(|| {
        AppError::new(
            "favicon_url_invalid",
            format!("不是一个可请求的网址: {candidate}"),
        )
    })?;

    let response = http::get(&parts, ICON_MAX_BYTES, TIMEOUT_MS)?;
    if response.status != 200 {
        return Err(AppError::new(
            "favicon_fetch_failed",
            format!("{} 返回了状态 {}", candidate, response.status),
        ));
    }

    decode_image(&response.body, candidate)
}

/// 解码一段图片字节，并缩到统一的边长上限。
fn decode_image(bytes: &[u8], source: &str) -> AppResult<RgbaImage> {
    let image = image::load_from_memory(bytes).map_err(|error| {
        AppError::new(
            "favicon_decode_failed",
            format!("无法解析 {} 返回的图片: {error}", source),
        )
    })?;

    Ok(fit_within(image.to_rgba8()))
}

/// 把图像缩到最长边不超过 `ICON_MAX_EDGE`，保持比例。
///
/// `imageops::resize` 是按给定的宽高硬缩的，直接传 `(MAX, MAX)` 会把非正方形
/// 的图标拉变形，所以这里先按长边算出目标宽高再缩。
fn fit_within(image: RgbaImage) -> RgbaImage {
    let (width, height) = image.dimensions();
    let longest = width.max(height);
    if longest <= ICON_MAX_EDGE {
        return image;
    }

    let scale = f64::from(ICON_MAX_EDGE) / f64::from(longest);
    let target_width = ((f64::from(width) * scale).round() as u32).max(1);
    let target_height = ((f64::from(height) * scale).round() as u32).max(1);

    imageops::resize(&image, target_width, target_height, FilterType::Lanczos3)
}

/// 就地解码 `data:image/…;base64,…` 形式的内联图标。
///
/// 有些站点把小图标直接内联在 HTML 里，省一个请求。这种候选不需要联网。
fn decode_data_uri(candidate: &str) -> Option<RgbaImage> {
    let rest = candidate.strip_prefix("data:")?;
    if !rest.starts_with("image/") {
        return None;
    }

    let (meta, payload) = rest.split_once(',')?;
    // 形如 `image/png;base64`；不是 base64 的（URL 编码的 SVG 之类）直接放弃，
    // 反正后面还有别的候选和兜底。
    if !meta.to_ascii_lowercase().contains(";base64") {
        return None;
    }

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .ok()?;

    decode_image(&bytes, "内联图标").ok()
}

/// 站点根目录的 `/favicon.ico`。
///
/// 这是最老、也最通用的约定：即使页面没有声明任何图标，浏览器也会尝试它。
fn origin_favicon(url: &str) -> String {
    let (scheme, rest) = url.split_once("://").unwrap_or(("https", url));
    format!("{scheme}://{}/favicon.ico", authority_of(rest))
}

/// 把 URL 拆成 WinHTTP 需要的三部分。
///
/// `path_and_query` 至少是 `"/"` —— WinHTTP 的 `WinHttpOpenRequest` 传空路径
/// 会被当成无效请求。
pub(crate) fn split_url(url: &str) -> Option<UrlParts<'_>> {
    let (scheme, rest) = url.split_once("://")?;
    if scheme.is_empty() || rest.is_empty() {
        return None;
    }
    let is_https = scheme.eq_ignore_ascii_case("https");

    let (authority, path) = match rest.find(['/', '?', '#']) {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, "/"),
    };

    // 丢掉可能存在的 `user:pass@`；认证不在支持范围内，留着只会让主机名解析
    // 出奇怪的结果。
    let authority = authority.rsplit('@').next().unwrap_or(authority);

    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => {
            (host, port.parse::<u16>().ok()?)
        }
        _ => (authority, if is_https { 443 } else { 80 }),
    };

    if host.is_empty() {
        return None;
    }

    Some(UrlParts {
        host,
        port,
        path_and_query: if path.is_empty() { "/" } else { path },
        is_https,
    })
}

/// 把页面里写的相对地址解析成绝对地址。
///
/// 覆盖四种写法：绝对地址、协议相对 `//host/path`、根相对 `/path`、
/// 文档相对 `path` / `./path` / `../path`。`data:` 原样返回，交给
/// `decode_data_uri` 处理。
pub(crate) fn resolve_reference(base: &str, reference: &str) -> Option<String> {
    let reference = reference.trim();
    if reference.is_empty() {
        return None;
    }

    if reference.starts_with("data:") {
        return Some(reference.to_owned());
    }

    if let Some(rest) = reference.strip_prefix("//") {
        let scheme = base.split_once("://").map(|(scheme, _)| scheme)?;
        return Some(format!("{scheme}://{rest}"));
    }

    if reference.split_once("://").is_some() {
        return Some(reference.to_owned());
    }

    let (scheme, rest) = base.split_once("://")?;
    let authority = authority_of(rest);

    if let Some(path) = reference.strip_prefix('/') {
        return Some(format!("{scheme}://{authority}/{path}"));
    }

    let joined = join_path(&document_dir(rest), reference);
    Some(format!("{scheme}://{authority}{joined}"))
}

/// 把文档相对路径拼到文档目录上，并折叠 `.` 与 `..`。
///
/// 站点里写 `./icon.png` 或 `../static/icon.png` 都很常见，不折叠就会拼出
/// `/docs/./icon.png` 这种地址 —— 多数服务器能容忍，但也有直接 404 的。
fn join_path(dir: &str, reference: &str) -> String {
    let mut segments: Vec<&str> = dir.split('/').filter(|part| !part.is_empty()).collect();

    for part in reference.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            other => segments.push(other),
        }
    }

    format!("/{}", segments.join("/"))
}

/// 从 HTML 里按出现顺序挑出图标候选。
///
/// 只扫 `<link>` 标签，且要求 `rel` 里出现 `icon` —— 这样 `apple-touch-icon`、
/// `shortcut icon`、`mask-icon` 都能命中，而样式表之类的不会被算进来。
pub(crate) fn icon_candidates(html: &str, base: &str) -> Vec<String> {
    // 大小写不敏感地找标签，但属性值（尤其是 URL）必须按原样取出。
    let lower = html.to_ascii_lowercase();
    let mut candidates = Vec::new();
    let mut cursor = 0;

    while let Some(offset) = lower[cursor..].find("<link") {
        let start = cursor + offset;
        let Some(relative_end) = lower[start..].find('>') else {
            break;
        };
        let end = start + relative_end;
        cursor = end + 1;

        let tag = &lower[start..=end];
        let Some(rel) = attribute_value(tag, "rel") else {
            continue;
        };
        if !rel.contains("icon") {
            continue;
        }

        // `attribute_span` 返回的是相对 `tag` 的偏移量，取原样文本时要补回
        // `start` —— 直接用它去切 `html` 会切到完全不相干的字节上去。
        let Some((from, to)) = attribute_span(tag, "href") else {
            continue;
        };
        let href = html[start + from..start + to]
            .trim()
            .trim_matches(|c| c == '"' || c == '\'');
        if let Some(resolved) = resolve_reference(base, href) {
            candidates.push(resolved);
        }
    }

    candidates
}

/// 取 `name="value"` 里 value 在 `tag` 中的字节范围。
fn attribute_span(tag: &str, name: &str) -> Option<(usize, usize)> {
    let bytes = tag.as_bytes();
    let mut search = 0;

    loop {
        let offset = tag[search..].find(name)?;
        // 前面必须是空白或 `<`，否则会命中 `data-href` 这类名字里带 href 的属性。
        let at = search + offset;
        let boundary_ok = at == 0 || matches!(bytes[at - 1], b' ' | b'\t' | b'\n' | b'\r' | b'<');
        if !boundary_ok {
            search = at + name.len();
            continue;
        }

        let mut cursor = at + name.len();
        cursor += tag[cursor..]
            .chars()
            .take_while(|c| c.is_whitespace())
            .count();
        if tag.as_bytes().get(cursor) != Some(&b'=') {
            search = at + name.len();
            continue;
        }
        cursor += 1;
        let skip = tag[cursor..]
            .chars()
            .take_while(|c| c.is_whitespace())
            .count();
        cursor += skip;

        let quote = *tag.as_bytes().get(cursor)?;
        let (value_start, value_end) = if quote == b'"' || quote == b'\'' {
            let start = cursor + 1;
            let end = start + tag[start..].find(quote as char)?;
            (start, end)
        } else {
            let end = cursor
                + tag[cursor..]
                    .find(|c: char| c.is_whitespace() || c == '>')
                    .unwrap_or(tag.len() - cursor);
            (cursor, end)
        };

        return Some((value_start, value_end));
    }
}

/// 取属性值（已小写化）。
fn attribute_value(tag: &str, name: &str) -> Option<String> {
    let (from, to) = attribute_span(tag, name)?;
    Some(tag[from..to].to_ascii_lowercase())
}

/// 取 URL 里的 `host[:port]` 部分。
fn authority_of(after_scheme: &str) -> &str {
    match after_scheme.find(['/', '?', '#']) {
        Some(index) => &after_scheme[..index],
        None => after_scheme,
    }
}

/// 取 URL 里文档所在目录，末尾带 `/`。
fn document_dir(after_scheme: &str) -> String {
    let path = match after_scheme.find(['/', '?', '#']) {
        Some(index) => &after_scheme[index..],
        None => "/",
    };
    let path = match path.find(['?', '#']) {
        Some(index) => &path[..index],
        None => path,
    };

    match path.rfind('/') {
        Some(index) => format!("{}/", &path[..index]),
        None => "/".to_owned(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_url_derives_the_default_port_from_the_scheme() {
        let parts = split_url("https://example.com/a/b?c=1").expect("https URL");
        assert_eq!(parts.host, "example.com");
        assert_eq!(parts.port, 443);
        assert_eq!(parts.path_and_query, "/a/b?c=1");
        assert!(parts.is_https);

        let parts = split_url("http://example.com").expect("http URL");
        assert_eq!(parts.port, 80);
        // WinHTTP 不接受空路径。
        assert_eq!(parts.path_and_query, "/");
        assert!(!parts.is_https);
    }

    #[test]
    fn split_url_keeps_an_explicit_port_and_drops_credentials() {
        let parts = split_url("http://example.com:8080/x").expect("explicit port");
        assert_eq!(parts.host, "example.com");
        assert_eq!(parts.port, 8080);

        let parts = split_url("https://user:pw@example.com/x").expect("credentials");
        assert_eq!(parts.host, "example.com");

        // 不是 URL 的输入要能被挡住，否则 WinHTTP 会拿着空主机名去连。
        assert!(split_url("not-a-url").is_none());
        assert!(split_url("https://").is_none());
    }

    #[test]
    fn resolve_reference_covers_absolute_protocol_root_and_document_forms() {
        assert_eq!(
            resolve_reference("https://a.com/x/y", "https://b.com/i.png").as_deref(),
            Some("https://b.com/i.png")
        );
        assert_eq!(
            resolve_reference("http://a.com/x/y", "//b.com/i.png").as_deref(),
            Some("http://b.com/i.png")
        );
        assert_eq!(
            resolve_reference("https://a.com/x/y", "/root/i.png").as_deref(),
            Some("https://a.com/root/i.png")
        );
        assert_eq!(
            resolve_reference("https://a.com/x/y?z=1", "i.png").as_deref(),
            Some("https://a.com/x/i.png")
        );
        // `data:` 原样返回，交给 `decode_data_uri` 就地解码。
        assert_eq!(
            resolve_reference("https://a.com", "data:image/png;base64,AAAA").as_deref(),
            Some("data:image/png;base64,AAAA")
        );
    }

    #[test]
    fn icon_candidates_picks_icon_links_and_ignores_stylesheets() {
        let html = r#"
            <html><head>
              <link rel="stylesheet" href="/style.css">
              <link rel="icon" href="/fav.png" sizes="32x32">
              <link rel='apple-touch-icon' href='./touch.png'>
            </head></html>
        "#;

        let candidates = icon_candidates(html, "https://a.com/docs/page");
        assert_eq!(
            candidates,
            vec![
                "https://a.com/fav.png".to_owned(),
                "https://a.com/docs/touch.png".to_owned()
            ]
        );
    }

    #[test]
    fn icon_candidates_ignores_attributes_that_merely_contain_the_name() {
        // `data-href` 里也有 `href`，不能被误当成真正的属性。
        let html = r#"<link rel="icon" data-href="/wrong.png" href="/right.png">"#;
        assert_eq!(
            icon_candidates(html, "https://a.com"),
            vec!["https://a.com/right.png".to_owned()]
        );
    }

    #[test]
    fn fit_within_leaves_small_images_untouched() {
        let small = RgbaImage::from_pixel(16, 16, image::Rgba([1, 2, 3, 255]));
        assert_eq!(fit_within(small.clone()).dimensions(), (16, 16));

        let wide = RgbaImage::from_pixel(1024, 256, image::Rgba([1, 2, 3, 255]));
        let (width, height) = fit_within(wide).dimensions();
        assert_eq!(width, ICON_MAX_EDGE, "最长边应被压到上限");
        assert_eq!(height, 32, "短边按比例缩放，不能变形");
    }
}
