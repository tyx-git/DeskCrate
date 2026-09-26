// 极简 HTTP GET 客户端（Windows）。
//
// 只做一件事：把一个 URL 的响应体读回来，带超时、带体积上限。存在的理由是
// 抓取网址图标 —— 为此引入 reqwest 会连带拉进 tokio、hyper 和一套 TLS 实现，
// 而系统自带的 WinHTTP 已经提供了需要的一切：
//
// - TLS 走 schannel，证书链与系统信任库一致；
// - 代理走系统配置，企业网络里不用额外适配；
// - 超时、重定向由 WinHTTP 自己管。
//
// 这一层只封装平台 API，不做任何业务判断（哪些候选地址、什么格式能收），
// 那些归 `services::favicon`。

use std::ffi::c_void;

use windows::core::PCWSTR;
use windows::Win32::Networking::WinHttp::{
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryDataAvailable,
    WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest,
    WinHttpSetOption, WinHttpSetTimeouts, ERROR_WINHTTP_HEADER_NOT_FOUND,
    WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_DISABLE_REDIRECTS, WINHTTP_FLAG_SECURE,
    WINHTTP_OPEN_REQUEST_FLAGS, WINHTTP_OPTION_DISABLE_FEATURE, WINHTTP_QUERY_FLAG_NUMBER,
    WINHTTP_QUERY_LOCATION, WINHTTP_QUERY_STATUS_CODE,
};

use crate::{
    error::AppResult,
    models::AppError,
    system::{support, wide},
};

// 一次 GET 的上限与超时。
//
// 图标只有几十 KB，但 HTML 首页可能有几 MB；不加限制地读下去，抓取就会卡住添加流程好几秒。
const READ_CHUNK: usize = 16 * 1024;

// 请求用的 User-Agent。
// 这里刻意带上浏览器的形态，实测同一个站点squoosh.app，用产品自己的UA要6349ms才响应，换成浏览器UA只要
// 637ms。不少站点（CDN 或反爬）会给非浏览器客户端走慢速通道，在几秒的超时预算下走慢速通道就等于直接失败。
// 末尾仍然带上真实的产品标识：这是程序在替用户取图标，不是冒充用户浏览网页，留个真名让对方能识别来源。
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 DeskCrate/0.1";

// 一次 GET 的结果。
pub(crate) struct HttpResponse {
    // HTTP 状态码。
    pub status: u32,
    // 手动处理重定向时使用的 Location 响应头。
    pub location: Option<String>,
    // 响应体，长度不超过调用方给定的上限。
    pub body: Vec<u8>,
}

// 发起一个GET请求，最多读max_bytes字节。
// 自动重定向被禁用；调用方必须先校验Location才能发出下一跳请求。
// timeout_ms分别套在解析、连接、发送和接收上。
pub(crate) fn get(url: &UrlParts, max_bytes: usize, timeout_ms: i32) -> AppResult<HttpResponse> {
    let agent = wide::to_wide(USER_AGENT);
    let server = wide::to_wide(&url.host);
    let path = wide::to_wide(&url.path_and_query);
    let method = wide::to_wide("GET");

    unsafe {
        // 会话->连接->请求，三级句柄都必须关掉；用Handle包一层，
        // 任何一次提前返回都会按逆序释放。
        let session = Handle::new(WinHttpOpen(
            PCWSTR(agent.as_ptr()),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        ))?;
        let connect = Handle::new(WinHttpConnect(
            session.get(),
            PCWSTR(server.as_ptr()),
            url.port,
            0,
        ))?;

        // https必须带WINHTTP_FLAG_SECURE，否则WinHTTP会用明文去连443端口。
        let flags = if url.is_https {
            WINHTTP_FLAG_SECURE
        } else {
            WINHTTP_OPEN_REQUEST_FLAGS(0)
        };
        let request = Handle::new(WinHttpOpenRequest(
            connect.get(),
            PCWSTR(method.as_ptr()),
            PCWSTR(path.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            flags,
        ))?;

        WinHttpSetTimeouts(
            request.get(),
            timeout_ms,
            timeout_ms,
            timeout_ms,
            timeout_ms,
        )
        .map_err(|error| support::windows_error("http_timeout_failed", error))?;

        // 必须在收到响应前关闭WinHTTP自动重定向，否则每一跳都绕开URL策略校验。
        let redirect_policy = WINHTTP_DISABLE_REDIRECTS.to_ne_bytes();
        WinHttpSetOption(
            Some(request.get()),
            WINHTTP_OPTION_DISABLE_FEATURE,
            Some(&redirect_policy),
        )
        .map_err(|error| support::windows_error("http_redirect_policy_failed", error))?;

        WinHttpSendRequest(request.get(), None, None, 0, 0, 0)
            .map_err(|error| support::windows_error("http_send_failed", error))?;
        WinHttpReceiveResponse(request.get(), std::ptr::null_mut())
            .map_err(|error| support::windows_error("http_receive_failed", error))?;

        let status = query_status(request.get())?;
        let location = if is_redirect(status) {
            query_text_header(request.get(), WINHTTP_QUERY_LOCATION)?
        } else {
            None
        };
        let body = if location.is_some() {
            Vec::new()
        } else {
            read_body(request.get(), max_bytes)?
        };

        Ok(HttpResponse {
            status,
            location,
            body,
        })
    }
}

// 拆好的 URL：WinHTTP 的 `WinHttpConnect` / `WinHttpOpenRequest` 要分开传
// 主机名和路径，所以调用方先拆好。
pub(crate) struct UrlParts {
    pub host: String,
    pub port: u16,
    pub path_and_query: String,
    pub is_https: bool,
}

// 判断是否为 HTTP 重定向响应。
fn is_redirect(status: u32) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

// 读取文本响应头。
unsafe fn query_text_header(request: *mut c_void, name: u32) -> AppResult<Option<String>> {
    let mut length = 0;
    let query_name = PCWSTR::null();
    let query = unsafe {
        WinHttpQueryHeaders(
            request,
            name,
            query_name,
            None,
            &mut length,
            std::ptr::null_mut(),
        )
    };
    if let Err(error) = query {
        // WinHTTP reports an absent response header directly as HEADER_NOT_FOUND.
        if error.code().0 as u32 == ((7 << 16) | 0x8000_0000 | ERROR_WINHTTP_HEADER_NOT_FOUND) {
            return Ok(None);
        }
        // A size-probe is expected to fail with ERROR_INSUFFICIENT_BUFFER. Any other
        // failure is real and must not be mistaken for an empty header.
        if error.code().0 as u32 != ((7 << 16) | 0x8000_0000 | 122) {
            return Err(support::windows_error("http_header_failed", error));
        }
    }

    if length == 0 || length % 2 != 0 {
        return Ok(None);
    }
    let mut buffer = vec![0u16; (length as usize / size_of::<u16>()).saturating_add(1)];
    unsafe {
        WinHttpQueryHeaders(
            request,
            name,
            PCWSTR::null(),
            Some(buffer.as_mut_ptr().cast::<c_void>()),
            &mut length,
            std::ptr::null_mut(),
        )
        .map_err(|error| support::windows_error("http_header_failed", error))?;
    }
    if buffer.last() == Some(&0) {
        buffer.pop();
    }
    let value = String::from_utf16(&buffer)
        .map_err(|_| AppError::new("http_header_invalid", "响应头不是有效的 UTF-16"))?;
    Ok(Some(value))
}

// 读状态码。带 `WINHTTP_QUERY_FLAG_NUMBER` 时缓冲区是一个 `u32`。
unsafe fn query_status(request: *mut c_void) -> AppResult<u32> {
    let mut status: u32 = 0;
    let mut length = u32::try_from(size_of::<u32>()).unwrap_or(4);

    unsafe {
        WinHttpQueryHeaders(
            request,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some(std::ptr::addr_of_mut!(status).cast::<c_void>()),
            std::ptr::addr_of_mut!(length),
            std::ptr::null_mut(),
        )
        .map_err(|error| support::windows_error("http_status_failed", error))?;
    }

    Ok(status)
}

// 循环读响应体，累计到 `max_bytes` 就停。
//
// 超长响应不报错、直接截断 —— 抓图标时「少读一点」比「整次失败」有用得多：
//  首页 HTML 只需要前面那截 `<head>` 里的图标声明。
unsafe fn read_body(request: *mut c_void, max_bytes: usize) -> AppResult<Vec<u8>> {
    let mut body = Vec::new();
    let mut buffer = vec![0u8; READ_CHUNK];

    loop {
        if body.len() >= max_bytes {
            break;
        }

        let mut available: u32 = 0;
        unsafe {
            WinHttpQueryDataAvailable(request, std::ptr::addr_of_mut!(available))
                .map_err(|error| support::windows_error("http_query_length_failed", error))?;
        }
        if available == 0 {
            break;
        }

        // `buffer` 是固定大小的堆缓冲区，不能把 WinHTTP 报告的完整可用
        // 长度直接传进去；响应块可能明显大于 16 KiB。
        let to_read = (available as usize)
            .min(buffer.len())
            .min(max_bytes.saturating_sub(body.len()));
        if to_read == 0 {
            break;
        }
        let mut read: u32 = 0;
        unsafe {
            WinHttpReadData(
                request,
                buffer.as_mut_ptr().cast::<c_void>(),
                u32::try_from(to_read).unwrap_or(u32::MAX),
                std::ptr::addr_of_mut!(read),
            )
            .map_err(|error| support::windows_error("http_read_failed", error))?;
        }
        if read == 0 {
            break;
        }

        let remaining = max_bytes.saturating_sub(body.len());
        let take = (read as usize).min(remaining).min(buffer.len());
        body.extend_from_slice(&buffer[..take]);
    }

    Ok(body)
}

// WinHTTP 句柄的所有权包装，离开作用域即关闭。
//
// 直接拿裸指针写下去，任何一次 `?` 提前返回都会泄漏句柄；这类泄漏在一个
// 常驻的桌面程序里会一直累积。
struct Handle(*mut c_void);

impl Handle {
    fn new(raw: *mut c_void) -> AppResult<Self> {
        if raw.is_null() {
            return Err(AppError::new(
                "http_open_failed",
                "无法创建 WinHTTP 句柄".to_owned(),
            ));
        }

        Ok(Self(raw))
    }

    fn get(&self) -> *mut c_void {
        self.0
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = WinHttpCloseHandle(self.0);
            }
        }
    }
}
