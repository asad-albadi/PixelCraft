use serde::Deserialize;
use serde_json::{Value, json};
use std::{net::IpAddr, time::Duration};
use ureq::http::{self, Method, Uri, header};

const MAX_REQUEST_BYTES: usize = 64 * 1024;
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Request<'a> {
    url: &'a str,
    method: Option<&'a str>,
    body: Option<&'a str>,
    content_type: Option<&'a str>,
}

/// Validate the request and obtain approval before starting DNS or HTTP.
pub fn script_network_request(
    request: &Value,
    authorize: impl FnOnce(&str) -> Result<(), String>,
) -> Result<Value, String> {
    let data = Request::deserialize(request).map_err(|error| error.to_string())?;
    if data.url.len() > 2048 || data.url.contains('#') {
        return Err("Invalid Lua HTTP URL".into());
    }
    let uri = data
        .url
        .parse::<Uri>()
        .map_err(|_| "Invalid Lua HTTP URL")?;
    let authority = uri.authority().ok_or("Lua HTTP requires an absolute URL")?;
    if authority.as_str().contains('@') {
        return Err("Lua HTTP URLs cannot contain user credentials".into());
    }
    let loopback = authority
        .host()
        .trim_matches(['[', ']'])
        .parse::<IpAddr>()
        .is_ok_and(|ip| ip.is_loopback());
    if uri.scheme_str() != Some("https") && !(uri.scheme_str() == Some("http") && loopback) {
        return Err("Lua HTTP requires HTTPS or an HTTP loopback address".into());
    }
    let method = match data.method.unwrap_or("GET") {
        "GET" => Method::GET,
        "HEAD" => Method::HEAD,
        "POST" => Method::POST,
        "PUT" => Method::PUT,
        "PATCH" => Method::PATCH,
        "DELETE" => Method::DELETE,
        _ => return Err("Unsupported Lua HTTP method".into()),
    };
    let body = data.body.unwrap_or("");
    if body.len() > MAX_REQUEST_BYTES
        || (!body.is_empty() && matches!(method, Method::GET | Method::HEAD))
    {
        return Err("Invalid Lua HTTP request body".into());
    }
    let content_type = data.content_type.unwrap_or("text/plain; charset=utf-8");
    if content_type.len() > 200 || !content_type.bytes().all(|b| (0x20..=0x7e).contains(&b)) {
        return Err("Invalid Lua HTTP content type".into());
    }
    let mut description = format!("{method} {uri}");
    if !body.is_empty() {
        let prefix: String = body.chars().take(1024).collect();
        description.push_str(&format!(
            "\n{content_type} · {} bytes\n{prefix:?}",
            body.len()
        ));
        if prefix.len() != body.len() {
            description.push('…');
        }
    }
    let send_body =
        data.body.is_some() || matches!(method, Method::POST | Method::PUT | Method::PATCH);
    let mut request = http::Request::builder().method(method).uri(uri);
    if send_body {
        request = request.header(header::CONTENT_TYPE, content_type);
    }
    let request = request
        .body(body.as_bytes())
        .map_err(|error| error.to_string())?;
    authorize(&description)?;

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .proxy(None)
        .max_redirects(0)
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(12)))
        .build()
        .into();
    let mut response = agent.run(request).map_err(|error| error.to_string())?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let body = response
        .body_mut()
        .with_config()
        .limit(MAX_RESPONSE_BYTES)
        .read_to_string()
        .map_err(|error| error.to_string())?;
    Ok(json!({"status":status,"contentType":content_type,"body":body}))
}
