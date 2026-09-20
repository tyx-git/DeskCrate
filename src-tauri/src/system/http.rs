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
    WinHttpSetTimeouts, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_FLAG_SECURE,
    WINHTTP_OPEN_REQUEST_FLAGS, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
};

use crate::{error::AppResult, models::AppError};

// 一次 GET 的上限与超时。
//
// 图标只有几十 KB，但 HTML 首页可能有几 MB；不加限制地读下去，抓取就会变成
// 卡住添加流程好几秒的元凶。
const READ_CHUNK: usize = 16 * 1024;

/// 一次 GET 的结果。
pub(crate) struct HttpResponse {
    /// HTTP 状态码。
    pub status: u32,
    /// 响应体，长度不超过调用方给定的上限。
    pub body: Vec<u8>,
}

/// 发起一个 GET 请求，最多读 `max_bytes` 字节。
///
/// `timeout_ms` 分别套在解析、连接、发送和接收上。任一步失败都返回错误，
/// 调用方据此决定是换个候选地址还是放弃。
pub(crate) fn get(
    url: &UrlParts<'_>,
    max_bytes: usize,
    timeout_ms: i32,
) -> AppResult<HttpResponse> {
    let agent = to_wide("DeskCrate/0.1");
    let server = to_wide(url.host);
    let path = to_wide(url.path_and_query);
    let method = to_wide("GET");

    unsafe {
        // 会话 -> 连接 -> 请求，三级句柄都必须关掉；用 `Handle` 包一层，
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

        // https 必须带 `WINHTTP_FLAG_SECURE`，否则 WinHTTP 会用明文去连 443 端口。
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
        .map_err(|error| windows_error("http_timeout_failed", error))?;

        WinHttpSendRequest(request.get(), None, None, 0, 0, 0)
            .map_err(|error| windows_error("http_send_failed", error))?;
        WinHttpReceiveResponse(request.get(), std::ptr::null_mut())
            .map_err(|error| windows_error("http_receive_failed", error))?;

        let status = query_status(request.get())?;
        let body = read_body(request.get(), max_bytes)?;

        Ok(HttpResponse { status, body })
    }
}

/// 拆好的 URL：WinHTTP 的 `WinHttpConnect` / `WinHttpOpenRequest` 要分开传
/// 主机名和路径，所以调用方先拆好。
pub(crate) struct UrlParts<'a> {
    pub host: &'a str,
    pub port: u16,
    pub path_and_query: &'a str,
    pub is_https: bool,
}

/// 读状态码。带 `WINHTTP_QUERY_FLAG_NUMBER` 时缓冲区是一个 `u32`。
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
        .map_err(|error| windows_error("http_status_failed", error))?;
    }

    Ok(status)
}

/// 循环读响应体，累计到 `max_bytes` 就停。
///
/// 超长响应不报错、直接截断 —— 抓图标时「少读一点」比「整次失败」有用得多：
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
                .map_err(|error| windows_error("http_query_length_failed", error))?;
        }
        if available == 0 {
            break;
        }

        let to_read = available as usize;
        let mut read: u32 = 0;
        unsafe {
            WinHttpReadData(
                request,
                buffer.as_mut_ptr().cast::<c_void>(),
                u32::try_from(to_read).unwrap_or(u32::MAX),
                std::ptr::addr_of_mut!(read),
            )
            .map_err(|error| windows_error("http_read_failed", error))?;
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

/// WinHTTP 句柄的所有权包装，离开作用域即关闭。
///
/// 直接拿裸指针写下去，任何一次 `?` 提前返回都会泄漏句柄；这类泄漏在一个
/// 常驻的桌面程序里会一直累积。
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

// WinHTTP 的三级句柄都在同一线程里创建和使用，不需要跨线程。
unsafe impl Send for Handle {}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn windows_error(code: &'static str, error: windows::core::Error) -> AppError {
    AppError::new(code, error.to_string())
}
