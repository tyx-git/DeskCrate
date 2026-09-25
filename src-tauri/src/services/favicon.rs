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

use image::imageops::{self, FilterType};
use image::RgbaImage;
use url::{Host, Url};

use crate::{error::AppResult, models::AppError, system::http};

// 一次抓取的预算。
//
// 首页 HTML 只要前面一截就够了（图标声明都在 `<head>` 里），256KB 足够；
// 单个图标 512 KiB 已经足够保留高分辨率 favicon，同时限制异常响应体。
const PAGE_MAX_BYTES: usize = 256 * 1024;
const ICON_MAX_BYTES: usize = 512 * 1024;

// 单次请求 5 秒，整次抓取 8 秒。
//
// 原先单次 3 秒：实测 `vuejs.org` 用浏览器 UA 也要 6 秒才响应（慢速通道），3 秒
// 超时下它必然失败，但站点本身是好的，放宽就能拿到。
//
// 总预算是另一回事 —— 站点不可达时每个候选都会各自耗满一次超时，没有总闸就会
// 叠成十几秒。到了预算就停手，用已经拿到的最好结果（哪怕是内置图标）。
const TIMEOUT_MS: i32 = 5000;
const TOTAL_BUDGET_MS: u128 = 8000;

// 最多尝试几个候选地址。站点可能声明一堆不同尺寸的图标，逐个试下去会把
// 「添加一个网址」拖成好几秒的等待。
const MAX_CANDIDATES: usize = 3;
const MAX_REDIRECTS: usize = 5;

// 图标缩放后的最长边。256px 为高 DPI 侧栏保留足够的采样余量。
const ICON_MAX_EDGE: u32 = 256;

/// 一次抓取的最终产物。
///
/// 分成两种是因为 SVG 走不了位图那条路：`image` crate 不支持 SVG，但 WebView 的
/// `<img>` 原生能渲染，而且矢量在 44px 的格子里比位图还清楚。所以 SVG 不解码，
/// 把原文字节交出去，由存储层按 `.svg` 落盘。
pub(crate) enum FetchedIcon {
    /// 解码好并缩到统一边长上限的位图。
    Bitmap(RgbaImage),
    /// SVG 原文。
    Svg(Vec<u8>),
}

/// 抓取 `url` 的站点图标。
pub(crate) fn parse_http_origin(url: &str) -> Option<String> {
    let parsed = parse_http_url(url).ok()?;
    let host = match parsed.host()? {
        Host::Domain(host) => host.to_ascii_lowercase(),
        Host::Ipv4(host) => host.to_string(),
        Host::Ipv6(host) => format!("[{host}]"),
    };
    let port = parsed.port_or_known_default()?;
    Some(format!("{}://{host}:{port}", parsed.scheme()))
}

pub(crate) fn fetch_icon(url: &str) -> AppResult<FetchedIcon> {
    fetch_icon_into(url, &mut Vec::new())
}

/// 抓取图标，并把每一步的结果写进 `log`。
///
/// **诊断探针与正常抓取共用这一条代码路径** —— 探针要是自己另写一套请求，量到的
/// 就不是真实行为了（比如换成了 Node 的网络栈，403 和 UA 问题全都不复现）。
/// 正常抓取传一个随即丢弃的 `Vec`，代价只有一点记账。
fn fetch_icon_into(url: &str, log: &mut Vec<ProbeStep>) -> AppResult<FetchedIcon> {
    let page_url = parse_http_url(url)?;
    let mut budget = Budget::new(TOTAL_BUDGET_MS);
    let mut last_error = AppError::new("favicon_unavailable", "没有可用的网址图标".to_owned());

    // 少数站点把图标放在自己控制的静态资源域名上，首页反而会被反爬挡住。
    // 先试这些一方已知地址，命中后连首页都不用请求；这正是 GitHub 的情况：
    // github.com/favicon.ico 会返回 403，但 github.githubassets.com/favicons/favicon.svg
    // 是官方静态资源且可直接取到。
    for candidate in well_known_candidates(&page_url) {
        match fetch_image(&candidate.url, log, &mut budget) {
            Ok(icon) => {
                log.push(ProbeStep::note(
                    "result",
                    &candidate.url,
                    "命中一方已知地址，结束",
                ));
                return Ok(icon);
            }
            Err(error) => {
                log::debug!("一方已知图标 {} 不可用: {}", candidate.url, error.message);
                last_error = error;
            }
        }
    }

    let mut declared = declared_candidates(&page_url, log, &mut budget);
    let final_page_url = declared.final_page_url.take().unwrap_or(page_url);
    // 约定路径是**另一段**：声明的候选最多试 `MAX_CANDIDATES` 个，试完还没拿到才
    // 轮到它们。合成一条链的话，一个声明了 3 个图标却全都拿不到的站点就永远轮不
    // 到 `/favicon.ico`，而它往往就在那儿。
    let conventions = convention_candidates(&final_page_url);
    let chain = declared
        .icons
        .iter()
        .take(MAX_CANDIDATES)
        .chain(conventions.iter());

    // `attempted` 是当前候选的下标：0 表示还一个都没试过。
    for (attempted, candidate) in chain.enumerate() {
        // 预算用尽就停手 —— 但**第一个候选一定要试**。
        //
        // 取首页本身可能就吃掉整个预算（慢速通道的站点一次请求能拖到 20 秒，
        // 而预算只有 8 秒）。这时候如果一个候选都不试，这次抓取就注定失败 ——
        // 实测 `react.dev` / `bilibili` 正是这么丢的：页面拿到了、声明了 4 个图标，
        // 却因为预算见底一个都没请求。预算该管的是「别一个接一个地试下去」，
        // 不是「连试都别试」。
        //
        // 另外，光判断「已经超了」还不够：WinHTTP 的超时是**四个阶段各自独立**
        // 的（解析 / 连接 / 发送 / 接收），一次请求最坏能拖到 4 倍时长。所以还要把
        // 剩余预算当作本次的超时传下去，才能让总耗时真的收敛到预算附近。
        if attempted > 0 && budget.exhausted() {
            log.push(ProbeStep::note(
                "result",
                &candidate.url,
                "总预算用尽，停止尝试",
            ));
            break;
        }

        match fetch_image(&candidate.url, log, &mut budget) {
            Ok(icon) => {
                log.push(ProbeStep::note("result", &candidate.url, "命中，结束"));
                return Ok(icon);
            }
            Err(error) => {
                log::debug!("网址图标候选 {} 不可用: {}", candidate.url, error.message);
                last_error = error;
            }
        }
    }

    Err(last_error)
}

/// 整次抓取共用的一份时间预算。
///
/// 存在的理由：`TIMEOUT_MS` 只是**单次请求的**超时，而 WinHTTP 把它分别套在解析、
/// 连接、发送、接收四个阶段上 —— 一次请求最坏能拖到 4 倍时长。只靠「每步不超时」
/// 算不出总时长，必须另外记一个整体预算，并把剩余时间传下去当本次的超时。
struct Budget {
    started: std::time::Instant,
    total_ms: u128,
}

impl Budget {
    fn new(total_ms: u128) -> Self {
        Self {
            started: std::time::Instant::now(),
            total_ms,
        }
    }

    /// 已经过去多久。
    fn elapsed_ms(&self) -> u128 {
        self.started.elapsed().as_millis()
    }

    fn exhausted(&self) -> bool {
        self.elapsed_ms() >= self.total_ms
    }

    /// 本次请求还能用多久；至少给 1ms，不传 0（WinHTTP 会把 0 当成「无限等待」）。
    fn remaining(&self) -> i32 {
        let cap = u128::try_from(TIMEOUT_MS).unwrap_or(5000);
        let left = self.total_ms.saturating_sub(self.elapsed_ms()).min(cap);
        i32::try_from(left).unwrap_or(TIMEOUT_MS).max(1)
    }
}

/// 诊断用：探测一个网址能不能抓到图标，并给出每一步的细节。
///
/// 只诊断、不落盘：拿到的图标丢掉，不写 `icons/`。
///
/// 只有 debug 构建才用得到（`probe_url_icon` 命令同样只在 debug 下注册），
/// release 里连带 `UrlIconProbe` 一起裁掉，否则会被算成死代码。
#[cfg(debug_assertions)]
pub(crate) fn probe(url: &str) -> UrlIconProbe {
    let started = std::time::Instant::now();
    let mut steps = Vec::new();
    let outcome = fetch_icon_into(url, &mut steps);
    let elapsed = started.elapsed();

    let stage = match &outcome {
        Ok(FetchedIcon::Bitmap(image)) => format!("成功 {}x{}", image.width(), image.height()),
        Ok(FetchedIcon::Svg(bytes)) => format!("成功 SVG {} 字节", bytes.len()),
        Err(error) => format!("失败：{}", error.message),
    };

    UrlIconProbe {
        url: url.to_owned(),
        ok: outcome.is_ok(),
        stage,
        total_ms: elapsed.as_millis(),
        steps,
    }
}

/// 诊断用：一次抓取里某一步的结果。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeStep {
    /// `page`（取首页）／`candidate`（下载某个候选）／`result`（结论）。
    pub stage: String,
    pub url: String,
    pub status: Option<u32>,
    pub bytes: usize,
    /// 按魔数判断出的内容类型；用来识别「状态 200 但其实是 HTML」的软 404。
    pub kind: Option<String>,
    pub note: String,
}

impl ProbeStep {
    fn note(stage: &str, url: &str, note: &str) -> Self {
        Self {
            stage: stage.to_owned(),
            url: url.to_owned(),
            status: None,
            bytes: 0,
            kind: None,
            note: note.to_owned(),
        }
    }
}

/// 诊断用：一个网址的完整探测报告。
#[cfg(debug_assertions)]
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlIconProbe {
    pub url: String,
    pub ok: bool,
    /// 成功时是图像尺寸，失败时是最后一个错误。
    pub stage: String,
    pub total_ms: u128,
    pub steps: Vec<ProbeStep>,
}

/// 按魔数判断一段字节是什么。
///
/// 挡掉「HTTP 状态是 200、内容其实是 HTML 错误页」这种情况：不先认一下内容，
/// 这种候选会一路走到解码才失败，白白吃掉一个候选名额和一次超时等待。
pub(crate) fn sniff_kind(bytes: &[u8]) -> &'static str {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => "png",
        [0xFF, 0xD8, 0xFF, ..] => "jpeg",
        [b'G', b'I', b'F', b'8', ..] => "gif",
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "webp",
        [b'B', b'M', ..] => "bmp",
        [0x00, 0x00, 0x01, 0x00, ..] => "ico",
        _ => {
            // SVG 与 HTML 前面可能带 UTF-8 BOM、空白或 XML 声明；先跳过这些，再
            // 认根标签。不能只看前 256 字节里是否包含 `<svg`，否则一个 HTML
            // 错误页在正文里提到 SVG 也会被误判成图标。
            let text = String::from_utf8_lossy(bytes);
            let head = text.trim_start_matches('\u{feff}').trim_start();
            let lower = head
                .get(..head.len().min(512))
                .unwrap_or(head)
                .to_ascii_lowercase();

            if lower.starts_with("<svg") || lower.starts_with("<?xml") && lower.contains("<svg") {
                "svg"
            } else if lower.starts_with("<html")
                || lower.starts_with("<!doctype html")
                || lower.starts_with("<head")
                || lower.starts_with("<body")
            {
                "html"
            } else {
                "unknown"
            }
        }
    }
}

/// 取首页，把它声明的图标候选按质量排好序返回。
struct DeclaredCandidates {
    icons: Vec<Candidate>,
    final_page_url: Option<Url>,
}

/// 取首页并逐跳校验重定向，使用最终地址解析页面中的相对图标链接。
fn declared_candidates(
    url: &Url,
    log: &mut Vec<ProbeStep>,
    budget: &mut Budget,
) -> DeclaredCandidates {
    let mut current = url.clone();
    for redirect_count in 0..=MAX_REDIRECTS {
        let current_text = current.as_str().to_owned();
        let response = match http::get(&url_parts(&current), PAGE_MAX_BYTES, budget.remaining()) {
            Ok(response) => response,
            Err(error) => {
                log.push(ProbeStep {
                    stage: "page".to_owned(),
                    url: current_text,
                    status: None,
                    bytes: 0,
                    kind: None,
                    note: format!("取首页失败：{}", error.message),
                });
                return DeclaredCandidates {
                    icons: Vec::new(),
                    final_page_url: None,
                };
            }
        };

        match checked_redirect_target(&current, response.location.as_deref(), redirect_count) {
            Ok(Some(next)) => {
                log.push(ProbeStep {
                    stage: "page".to_owned(),
                    url: current_text,
                    status: Some(response.status),
                    bytes: 0,
                    kind: None,
                    note: format!("重定向到 {}", next.as_str()),
                });
                current = next;
                continue;
            }
            Ok(None) => {}
            Err(error) => {
                log.push(ProbeStep {
                    stage: "page".to_owned(),
                    url: current_text,
                    status: Some(response.status),
                    bytes: 0,
                    kind: None,
                    note: format!("拒绝重定向：{}", error.message),
                });
                return DeclaredCandidates {
                    icons: Vec::new(),
                    final_page_url: None,
                };
            }
        }

        let kind = sniff_kind(&response.body);
        let icons = if response.status == 200 && kind == "html" {
            icon_candidates(&String::from_utf8_lossy(&response.body), &current)
        } else {
            Vec::new()
        };
        log.push(ProbeStep {
            stage: "page".to_owned(),
            url: current_text,
            status: Some(response.status),
            bytes: response.body.len(),
            kind: Some(kind.to_owned()),
            note: format!("声明了 {} 个图标候选", icons.len()),
        });
        return DeclaredCandidates {
            icons,
            final_page_url: Some(current),
        };
    }
    DeclaredCandidates {
        icons: Vec::new(),
        final_page_url: None,
    }
}

/// 一个图标候选，带排序要用的质量信息。
///
/// 存在的理由：站点常常一次性声明好几个图标（16×16 的 ico、32×32 的 png、
/// 180×180 的 apple-touch-icon），而最多只能试 `MAX_CANDIDATES` 个。按文档顺序
/// 取会让最小的那个抢走名额，剩下的再好也轮不上。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Candidate {
    url: String,
    /// 声明的最大边长，已夹到 `ICON_MAX_EDGE`。
    edge: u32,
    /// 格式优先级，越小越优先。见 `FORMAT_*`。
    format: u8,
}

// 格式优先级：数字越小越优先。
/// 能直接解码的位图（png / webp / jpeg / gif）。
const FORMAT_BITMAP: u8 = 0;
/// SVG。浏览器能原生渲染，但同尺寸下仍优先使用位图。
const FORMAT_SVG: u8 = 1;
/// ICO 和认不出来的格式。ICO 常常只有 16×16，所以排在位图与 SVG 之后。
const FORMAT_LEGACY: u8 = 2;

impl Candidate {
    fn new(url: String, declared_type: Option<&str>, sizes: &str) -> Self {
        let format = format_rank(declared_type, &url);
        // SVG 是矢量，缩到任何尺寸都不失真，而它通常写 `sizes="any"` ——
        // `declared_edge` 对这种（以及缺失）本来就按上限计，正好合用。
        let edge = declared_edge(sizes);

        Self { url, edge, format }
    }
}

/// 按 `sizes` 声明取出最长边；`16x16 32x32` 取最大的那个。
///
/// 缺失或 `sizes="any"` 时按 `ICON_MAX_EDGE` 计：多数站点不写 sizes 时给的就是
/// 主图标，没必要因此把它排到后面去。
fn declared_edge(sizes: &str) -> u32 {
    let mut best = 0;
    for part in sizes.split_whitespace() {
        let Some((width, height)) = part.split_once('x') else {
            continue;
        };
        let (Ok(width), Ok(height)) = (width.parse::<u32>(), height.parse::<u32>()) else {
            continue;
        };
        best = best.max(width).max(height);
    }

    if best == 0 {
        ICON_MAX_EDGE
    } else {
        // 图标最终会被压到 `ICON_MAX_EDGE`，超过它的尺寸对我们没区别 —— 夹住，
        // 免得 512×512 压过同样够用的 180×180。
        best.min(ICON_MAX_EDGE)
    }
}

/// 先认 `<link type="…">` 声明的 MIME，认不出再按扩展名猜。
fn format_rank(declared_type: Option<&str>, url: &str) -> u8 {
    let mime = declared_type.unwrap_or_default().to_ascii_lowercase();
    if mime.contains("svg") {
        return FORMAT_SVG;
    }
    if mime.contains("png")
        || mime.contains("jpeg")
        || mime.contains("gif")
        || mime.contains("webp")
    {
        return FORMAT_BITMAP;
    }
    if mime.contains("icon") {
        return FORMAT_LEGACY;
    }

    // 去掉查询串再看扩展名。
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .to_ascii_lowercase();
    if path.ends_with(".svg") {
        FORMAT_SVG
    } else if path.ends_with(".png")
        || path.ends_with(".jpg")
        || path.ends_with(".jpeg")
        || path.ends_with(".gif")
        || path.ends_with(".webp")
    {
        FORMAT_BITMAP
    } else {
        FORMAT_LEGACY
    }
}

/// 少数大型站点会把图标放在自己的静态资源域名上，首页或根 favicon 可能被反爬挡住。
///
/// 这里只放**明确属于站点本身**的已知地址，不是公共 favicon 服务，也不会把任意域名
/// 发送给第三方。候选排在首页声明之前，是为了让 GitHub 这种首页 403 的站点仍能取到
/// 官方图标；其他站点返回空列表，继续走普通链路。
fn well_known_candidates(url: &Url) -> Vec<Candidate> {
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();

    if host == "github.com" || host == "www.github.com" {
        return vec![Candidate {
            edge: ICON_MAX_EDGE,
            format: FORMAT_SVG,
            url: "https://github.githubassets.com/favicons/favicon.svg".to_owned(),
        }];
    }

    Vec::new()
}

/// 站点约定路径：即使页面什么都没声明，这些位置也可能有图标。
///
/// 排在页面声明的候选**之后** —— 声明是站点自己指明的，约定路径只是「可能在那儿」，
/// 命中率明显低一档。
fn convention_candidates(url: &Url) -> Vec<Candidate> {
    ["favicon.ico", "favicon.png", "apple-touch-icon.png"]
        .into_iter()
        .filter_map(|name| {
            let mut candidate = url.clone();
            candidate.set_path("/");
            candidate.set_query(None);
            candidate.set_fragment(None);
            candidate = candidate.join(name).ok()?;
            let url = candidate.as_str().to_owned();
            Some(Candidate {
                edge: 0,
                format: format_rank(None, &url),
                url,
            })
        })
        .collect()
}

/// 下载（或就地解码）一个候选地址，成功则返回图标。
fn fetch_image(
    candidate: &str,
    log: &mut Vec<ProbeStep>,
    budget: &mut Budget,
) -> AppResult<FetchedIcon> {
    let mut current = parse_http_url(candidate)?;
    for redirect_count in 0..=MAX_REDIRECTS {
        let current_text = current.as_str().to_owned();
        let response = http::get(&url_parts(&current), ICON_MAX_BYTES, budget.remaining())
            .inspect_err(|error| {
                log.push(ProbeStep {
                    stage: "candidate".to_owned(),
                    url: current_text.clone(),
                    status: None,
                    bytes: 0,
                    kind: None,
                    note: format!("请求失败：{}", error.message),
                });
            })?;

        match checked_redirect_target(&current, response.location.as_deref(), redirect_count) {
            Ok(Some(next)) => {
                log.push(ProbeStep {
                    stage: "candidate".to_owned(),
                    url: current_text,
                    status: Some(response.status),
                    bytes: 0,
                    kind: None,
                    note: format!("重定向到 {}", next.as_str()),
                });
                current = next;
                continue;
            }
            Ok(None) => {}
            Err(error) => {
                log.push(ProbeStep {
                    stage: "candidate".to_owned(),
                    url: current_text,
                    status: Some(response.status),
                    bytes: 0,
                    kind: None,
                    note: format!("拒绝重定向：{}", error.message),
                });
                return Err(error);
            }
        }

        let kind = sniff_kind(&response.body);
        if response.status != 200 {
            log.push(ProbeStep {
                stage: "candidate".to_owned(),
                url: current_text,
                status: Some(response.status),
                bytes: response.body.len(),
                kind: Some(kind.to_owned()),
                note: format!("状态 {}", response.status),
            });
            return Err(AppError::new(
                "favicon_fetch_failed",
                format!("{} 返回了状态 {}", current, response.status),
            ));
        }
        if kind == "html" {
            log.push(ProbeStep {
                stage: "candidate".to_owned(),
                url: current_text,
                status: Some(response.status),
                bytes: response.body.len(),
                kind: Some(kind.to_owned()),
                note: "状态 200 但内容是 HTML（软 404）".to_owned(),
            });
            return Err(AppError::new(
                "favicon_not_image",
                format!("{} 返回的不是图片", current),
            ));
        }
        if kind == "svg" {
            log.push(ProbeStep {
                stage: "candidate".to_owned(),
                url: current_text,
                status: Some(response.status),
                bytes: response.body.len(),
                kind: Some(kind.to_owned()),
                note: "SVG 原文，直接落盘".to_owned(),
            });
            return Ok(FetchedIcon::Svg(response.body));
        }

        return match decode_image(&response.body, current.as_str()) {
            Ok(image) => {
                log.push(ProbeStep {
                    stage: "candidate".to_owned(),
                    url: current_text,
                    status: Some(response.status),
                    bytes: response.body.len(),
                    kind: Some(kind.to_owned()),
                    note: "解码成功".to_owned(),
                });
                Ok(FetchedIcon::Bitmap(image))
            }
            Err(error) => {
                log.push(ProbeStep {
                    stage: "candidate".to_owned(),
                    url: current_text,
                    status: Some(response.status),
                    bytes: response.body.len(),
                    kind: Some(kind.to_owned()),
                    note: format!("解码失败：{}", error.message),
                });
                Err(error)
            }
        };
    }
    unreachable!("重定向循环必须在超过上限时返回")
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

fn parse_http_url(value: &str) -> AppResult<Url> {
    let (_, authority_and_path) = value
        .split_once("://")
        .ok_or_else(|| AppError::new("favicon_url_invalid", format!("不是有效的网址: {value}")))?;
    let authority = authority_and_path
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    if authority.is_empty() {
        return Err(AppError::new(
            "favicon_host_missing",
            format!("网址没有有效主机: {value}"),
        ));
    }

    let url = Url::parse(value).map_err(|error| {
        AppError::new(
            "favicon_url_invalid",
            format!("不是有效的网址 {value}: {error}"),
        )
    })?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AppError::new(
            "favicon_scheme_unsupported",
            format!("只允许 http 和 https 地址: {value}"),
        ));
    }
    if url.host().is_none() || url.host_str().map_or(true, str::is_empty) {
        return Err(AppError::new(
            "favicon_host_missing",
            format!("网址没有有效主机: {value}"),
        ));
    }
    if url.port_or_known_default().is_none() {
        return Err(AppError::new(
            "favicon_port_invalid",
            format!("网址端口无效: {value}"),
        ));
    }
    Ok(url)
}

fn checked_redirect_target(
    base: &Url,
    location: Option<&str>,
    redirect_count: usize,
) -> AppResult<Option<Url>> {
    let Some(location) = location else {
        return Ok(None);
    };
    if redirect_count == MAX_REDIRECTS {
        return Err(AppError::new(
            "favicon_redirect_limit",
            format!("重定向超过 {MAX_REDIRECTS} 次"),
        ));
    }
    resolve_http_reference(base, location).map(Some)
}

fn resolve_http_reference(base: &Url, reference: &str) -> AppResult<Url> {
    let target = base.join(reference.trim()).map_err(|error| {
        AppError::new(
            "favicon_url_invalid",
            format!("无法解析网址引用 {reference}: {error}"),
        )
    })?;
    parse_http_url(target.as_str())
}

fn url_parts(url: &Url) -> http::UrlParts {
    let host = match url.host().expect("已验证 HTTP URL 有主机") {
        Host::Domain(host) => host.to_owned(),
        Host::Ipv4(host) => host.to_string(),
        Host::Ipv6(host) => format!("[{host}]"),
    };
    let path_and_query = match url.query() {
        Some(query) => format!("{}?{query}", url.path()),
        None => url.path().to_owned(),
    };
    http::UrlParts {
        host,
        port: url.port_or_known_default().expect("已验证 HTTP URL 有端口"),
        path_and_query,
        is_https: url.scheme() == "https",
    }
}

/// 从 HTML 里挑出图标候选，并按**质量**排序返回。
///
/// 只扫 `<link>` 标签，且要求 `rel` 里出现 `icon` —— 这样 `apple-touch-icon`、
/// `shortcut icon`、`mask-icon` 都能命中，而样式表之类的不会被算进来。
///
/// 排序规则：边长降序（同尺寸时位图优先于 ICO、ICO 优先于解不了的 SVG）。
/// 文档顺序不作数 —— 站点常常把 16×16 的 ico 写在最前面。
pub(crate) fn icon_candidates(html: &str, base: &Url) -> Vec<Candidate> {
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

        let declared_type = attribute_value(tag, "type");
        let sizes = attribute_value(tag, "sizes").unwrap_or_default();
        if let Ok(resolved) = resolve_http_reference(base, href) {
            candidates.push(Candidate::new(
                resolved.as_str().to_owned(),
                declared_type.as_deref(),
                &sizes,
            ));
        }
    }

    candidates.sort_by(|a, b| b.edge.cmp(&a.edge).then(a.format.cmp(&b.format)));
    candidates
}

/// 返回字符串开头的 Unicode 空白所占的 UTF-8 字节数。
fn whitespace_prefix_bytes(value: &str) -> usize {
    value
        .char_indices()
        .take_while(|(_, character)| character.is_whitespace())
        .map(|(_, character)| character.len_utf8())
        .sum()
}

/// 取 `name="value"` 里 value 在 `tag` 中的字节范围。
fn attribute_span(tag: &str, name: &str) -> Option<(usize, usize)> {
    let mut search = 0;

    loop {
        let offset = tag[search..].find(name)?;
        // 前面必须是空白或 `<`，否则会命中 `data-href` 这类名字里带 href 的属性。
        let at = search + offset;
        let boundary_ok = at == 0
            || tag[..at]
                .chars()
                .next_back()
                .is_some_and(|character| character.is_whitespace() || character == '<');
        if !boundary_ok {
            search = at + name.len();
            continue;
        }

        let mut cursor = at + name.len();
        cursor += whitespace_prefix_bytes(&tag[cursor..]);
        if tag.as_bytes().get(cursor) != Some(&b'=') {
            search = at + name.len();
            continue;
        }
        cursor += 1;
        cursor += whitespace_prefix_bytes(&tag[cursor..]);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_http_and_https_with_valid_hosts_and_ports() {
        let https = parse_http_url("https://example.com/a/b?c=1").expect("https URL");
        assert_eq!(https.scheme(), "https");
        assert_eq!(https.host_str(), Some("example.com"));
        assert_eq!(https.port_or_known_default(), Some(443));
        assert_eq!(url_parts(&https).path_and_query, "/a/b?c=1");

        let http = parse_http_url("http://example.com").expect("http URL");
        assert_eq!(http.port_or_known_default(), Some(80));
        assert_eq!(url_parts(&http).path_and_query, "/");
        assert!(parse_http_url("http://example.com:8080/x").is_ok());
        assert!(parse_http_url("https://user:pw@example.com/x").is_ok());
    }

    #[test]
    fn rejects_unsupported_schemes_missing_hosts_and_malformed_urls() {
        for value in [
            "ftp://example.com/a",
            "file:///tmp/icon.png",
            "data:image/png;base64,AAAA",
        ] {
            assert!(parse_http_url(value).is_err(), "must reject {value}");
        }
        for value in [
            "not-a-url",
            "https://",
            "http:///path",
            "http://example.com:99999/x",
        ] {
            assert!(parse_http_url(value).is_err(), "must reject {value}");
        }
    }

    #[test]
    fn resolves_relative_links_with_url_parser_and_rejects_other_schemes() {
        let base = parse_http_url("https://a.com/x/y?z=1").unwrap();
        assert_eq!(
            resolve_http_reference(&base, "https://b.com/i.png")
                .unwrap()
                .as_str(),
            "https://b.com/i.png"
        );
        assert_eq!(
            resolve_http_reference(&base, "//b.com/i.png")
                .unwrap()
                .as_str(),
            "https://b.com/i.png"
        );
        assert_eq!(
            resolve_http_reference(&base, "/root/i.png")
                .unwrap()
                .as_str(),
            "https://a.com/root/i.png"
        );
        assert_eq!(
            resolve_http_reference(&base, "i.png").unwrap().as_str(),
            "https://a.com/x/i.png"
        );
        assert!(resolve_http_reference(&base, "data:image/png;base64,AAAA").is_err());
        assert!(resolve_http_reference(&base, "javascript:alert(1)").is_err());
    }

    #[test]
    fn accepts_https_to_http_downgrade_and_validates_each_redirect_target() {
        let base = parse_http_url("https://a.com/start").unwrap();
        let downgraded = resolve_http_reference(&base, "http://b.com/next").unwrap();
        assert_eq!(downgraded.scheme(), "http");
        assert_eq!(downgraded.host_str(), Some("b.com"));
        assert!(resolve_http_reference(&base, "ftp://b.com/next").is_err());
    }

    /// 只比地址，不比排序信息 —— 质量字段单独有用例覆盖。
    fn urls_of(candidates: &[Candidate]) -> Vec<&str> {
        candidates.iter().map(|c| c.url.as_str()).collect()
    }

    #[test]
    fn well_known_candidates_handles_github_official_asset() {
        let github = parse_http_url("https://github.com/org/repo").unwrap();
        let candidates = well_known_candidates(&github);
        assert_eq!(
            urls_of(&candidates),
            vec!["https://github.githubassets.com/favicons/favicon.svg"]
        );
        let gist = parse_http_url("https://gist.github.com").unwrap();
        let example = parse_http_url("https://example.com").unwrap();
        assert!(well_known_candidates(&gist).is_empty());
        assert!(well_known_candidates(&example).is_empty());
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

        let base = parse_http_url("https://a.com/docs/page").unwrap();
        let candidates = icon_candidates(html, &base);
        // 没写 sizes 的按「够用」计，所以 touch.png 排在 32×32 的 fav.png 前面。
        assert_eq!(
            urls_of(&candidates),
            vec!["https://a.com/docs/touch.png", "https://a.com/fav.png"]
        );
    }

    #[test]
    fn icon_candidates_handles_multibyte_whitespace_around_equals() {
        let html = concat!(
            "<link rel=\"icon\" href\u{3000}=\u{00a0}/wide.png>\n",
            "<link rel=\"icon\" href=\u{3000}\"/full-width.png\">\n",
            "<link rel=\"icon\" href=\u{00a0}/nbsp.png>"
        );

        assert_eq!(
            urls_of(&icon_candidates(
                html,
                &parse_http_url("https://a.com").unwrap()
            )),
            vec![
                "https://a.com/wide.png",
                "https://a.com/full-width.png",
                "https://a.com/nbsp.png",
            ]
        );
    }

    #[test]
    fn icon_candidates_handles_multibyte_whitespace_before_attribute_names() {
        let html = "<link\u{3000}rel=\"icon\"\u{00a0}href=\"/icon.png\">";

        assert_eq!(
            urls_of(&icon_candidates(
                html,
                &parse_http_url("https://a.com").unwrap()
            )),
            vec!["https://a.com/icon.png"]
        );
    }

    #[test]
    fn icon_candidates_ignores_attributes_that_merely_contain_the_name() {
        // `data-href` 里也有 `href`，不能被误当成真正的属性。
        let html = r#"<link rel="icon" data-href="/wrong.png" href="/right.png">"#;
        assert_eq!(
            urls_of(&icon_candidates(
                html,
                &parse_http_url("https://a.com").unwrap()
            )),
            vec!["https://a.com/right.png"]
        );
    }

    #[test]
    fn icon_candidates_sorts_biggest_first_and_keeps_svg_usable() {
        let html = r#"
            <link rel="icon" href="/small.ico" sizes="16x16">
            <link rel="icon" href="/huge.svg" sizes="any" type="image/svg+xml">
            <link rel="icon" href="/big.png" sizes="180x180">
        "#;

        // 256px 的 SVG 在前，随后是 180px 位图，再后是 16px ICO。
        assert_eq!(
            urls_of(&icon_candidates(
                html,
                &parse_http_url("https://a.com").unwrap()
            )),
            vec![
                "https://a.com/huge.svg",
                "https://a.com/big.png",
                "https://a.com/small.ico"
            ]
        );
    }

    #[test]
    fn declared_edge_clamps_to_the_icon_limit() {
        // 512 和 180 对最终 256 的图标没有区别，不该互相压过。
        assert_eq!(
            declared_edge("512x512"),
            ICON_MAX_EDGE,
            "超过上限的按上限计"
        );
        assert_eq!(declared_edge("180x180"), 180);
        assert_eq!(declared_edge("64x64"), 64, "没到上限的按实际尺寸");
        assert_eq!(declared_edge("16x16 32x32"), 32, "多个尺寸取最大的那个");
        assert_eq!(declared_edge(""), ICON_MAX_EDGE, "没声明按够用计");
        assert_eq!(declared_edge("any"), ICON_MAX_EDGE, "矢量图按够用计");
    }

    #[test]
    fn convention_candidates_covers_the_usual_paths() {
        let page = parse_http_url("https://a.com/docs/page").unwrap();
        assert_eq!(
            urls_of(&convention_candidates(&page)),
            vec![
                "https://a.com/favicon.ico",
                "https://a.com/favicon.png",
                "https://a.com/apple-touch-icon.png"
            ]
        );
    }

    #[test]
    fn fit_within_leaves_small_images_untouched() {
        let small = RgbaImage::from_pixel(16, 16, image::Rgba([1, 2, 3, 255]));
        assert_eq!(fit_within(small.clone()).dimensions(), (16, 16));

        let wide = RgbaImage::from_pixel(1024, 256, image::Rgba([1, 2, 3, 255]));
        let (width, height) = fit_within(wide).dimensions();
        assert_eq!(width, ICON_MAX_EDGE, "最长边应被压到上限");
        assert_eq!(height, 64, "短边按比例缩放，不能变形");
    }
}
