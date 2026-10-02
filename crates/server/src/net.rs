//! HTTP clients for reaching other atproto services.
//!
//! Handles and DID documents can point the appview at any URL, so everything
//! found through them goes through a client that refuses private, loopback
//! and link-local addresses, unless `ALLOW_PRIVATE_NETWORK` is set (dev and
//! tests, where everything is vivarium on localhost).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};

const TIMEOUT: Duration = Duration::from_secs(10);

/// The most the appview reads of any response from another service: DID
/// documents, metadata and token responses are all far smaller.
pub const BODY_CAP: usize = 64 * 1024;

/// A response body, refusing anything over `BODY_CAP` without reading past it.
pub async fn read_capped(mut res: reqwest::Response) -> Result<Vec<u8>, String> {
    let url = res.url().to_string();
    if res.content_length().is_some_and(|len| len > BODY_CAP as u64) {
        return Err(format!("{url} sent more than {BODY_CAP} bytes"));
    }
    let mut body = Vec::new();
    while let Some(chunk) = res.chunk().await.map_err(|e| format!("{url}: {e}"))? {
        if body.len() + chunk.len() > BODY_CAP {
            return Err(format!("{url} sent more than {BODY_CAP} bytes"));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// A JSON response body, capped like `read_capped`.
pub async fn read_json<T: serde::de::DeserializeOwned>(
    res: reqwest::Response,
) -> Result<T, String> {
    let url = res.url().to_string();
    let body = read_capped(res).await?;
    serde_json::from_slice(&body).map_err(|e| format!("{url}: {e}"))
}

/// Whether an address is on the public internet.
pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => is_public_v4(v4),
            None => is_public_v6(v6),
        },
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_multicast()
        || a == 0
        || (a == 100 && (64..128).contains(&b)) // shared address space (CGNAT)
        || (a == 198 && (b == 18 || b == 19)) // benchmarking
        || a >= 240)
}

fn is_public_v6(ip: Ipv6Addr) -> bool {
    let first = ip.segments()[0];
    !(ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (first & 0xfe00) == 0xfc00 // unique local
        || (first & 0xffc0) == 0xfe80 // link-local
        || first == 0x2001 && ip.segments()[1] == 0x0db8) // documentation
}

/// A resolver that drops every non-public address it's given.
struct PublicOnly;

impl Resolve for PublicOnly {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            let host = name.as_str().to_owned();
            let addrs: Vec<SocketAddr> =
                tokio::net::lookup_host((host.as_str(), 0)).await?.collect();
            let public: Vec<SocketAddr> = addrs.into_iter().filter(|a| is_public(a.ip())).collect();
            if public.is_empty() {
                return Err(format!("{host} has no public address").into());
            }
            Ok(Box::new(public.into_iter()) as Addrs)
        })
    }
}

#[derive(Clone)]
pub struct Http {
    /// For configured services (the handle resolver and PLC directory).
    pub trusted: reqwest::Client,
    guarded: reqwest::Client,
    allow_private: bool,
}

impl Http {
    pub fn new(allow_private: bool) -> Self {
        let builder = || {
            reqwest::Client::builder().timeout(TIMEOUT).redirect(reqwest::redirect::Policy::none())
        };
        let trusted = builder().build().expect("the HTTP client has a valid static configuration");
        let guarded = if allow_private {
            trusted.clone()
        } else {
            builder()
                .dns_resolver(Arc::new(PublicOnly))
                .build()
                .expect("the HTTP client has a valid static configuration")
        };
        Self { trusted, guarded, allow_private }
    }

    /// The client for a URL found through a handle or DID document, or an
    /// error when it names a private address outright.
    pub fn guarded(&self, url: &str) -> Result<&reqwest::Client, String> {
        let parsed = url::Url::parse(url).map_err(|e| format!("{url} is not a URL: {e}"))?;
        if !matches!(parsed.scheme(), "https" | "http") {
            return Err(format!("{url} is not an http(s) URL"));
        }
        if !self.allow_private {
            if parsed.scheme() != "https" {
                return Err(format!("{url} is not https"));
            }
            let literal = match parsed.host() {
                Some(url::Host::Ipv4(ip)) => Some(IpAddr::V4(ip)),
                Some(url::Host::Ipv6(ip)) => Some(IpAddr::V6(ip)),
                Some(url::Host::Domain(d))
                    if d.eq_ignore_ascii_case("localhost") || d.ends_with(".localhost") =>
                {
                    return Err(format!("{url} is a private address"));
                }
                _ => None,
            };
            if literal.is_some_and(|ip| !is_public(ip)) {
                return Err(format!("{url} is a private address"));
            }
        }
        Ok(&self.guarded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_addresses_are_refused_unless_allowed() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.1.1",
            "100.64.0.1",
            "::1",
            "fd00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "0.0.0.0",
        ] {
            assert!(!is_public(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["1.1.1.1", "2606:4700::1111"] {
            assert!(is_public(ip.parse().unwrap()), "{ip}");
        }
        let strict = Http::new(false);
        assert!(strict.guarded("http://localhost:2580/x").is_err());
        assert!(strict.guarded("https://127.0.0.1/x").is_err());
        assert!(strict.guarded("https://[::1]/x").is_err());
        assert!(strict.guarded("http://pds.example/x").is_err());
        assert!(strict.guarded("https://pds.example/x").is_ok());
        assert!(Http::new(true).guarded("http://localhost:2580/x").is_ok());
    }
}
