#![allow(unused)]
use std::collections::HashMap;

pub struct HttpClientConfig {
    pub username: Option<String>,
    pub password: Option<String>,
    pub max_redirects: Option<usize>,
    pub timeout: Option<u64>,
    pub proxy_url: Option<String>,
    pub request_header: Option<String>,
    pub http_cookies: Option<String>,
    pub http1: bool,
    pub http2: bool,
}
impl HttpClientConfig {
    pub fn new(
        username: Option<String>,
        password: Option<String>,
        max_redirects: Option<usize>,
        timeout: Option<u64>,
        proxy_url: Option<String>,
        request_header: Option<String>,
        http_cookies: Option<String>,
        http1: bool,
        http2: bool,
    ) -> HttpClientConfig {
        HttpClientConfig {
            username: username,
            password: password,
            max_redirects: max_redirects,
            timeout: timeout,
            proxy_url: proxy_url,
            request_header: request_header,
            http_cookies: http_cookies,
            http1: http1,
            http2: http2,
        }
    }
}
