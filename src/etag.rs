use bytes::Buf;
use http::HeaderValue;

#[derive(Debug, Clone)]
pub(crate) struct ETag(pub HeaderValue);

impl ETag {
    pub const fn empty() -> Self {
        Self(HeaderValue::from_static(r#""""#))
    }

    pub fn from_buf<T: Buf>(mut buf: T) -> Self {
        let mut ctx = ring::digest::Context::new(&ring::digest::SHA256);
        while buf.has_remaining() {
            let chunk = buf.chunk();
            ctx.update(chunk);
            buf.advance(chunk.len());
        }
        let digest = ctx.finish();
        Self::from_digest(digest)
    }

    pub fn from_digest(digest: ring::digest::Digest) -> Self {
        use std::io::Write;
        const QUOTE: u8 = br#"""#[0];
        let digest = digest.as_ref();
        let mut etag = Vec::with_capacity(digest.len() * 2 + 2);
        etag.push(QUOTE);
        for byte in digest {
            write!(etag, "{byte:02x}").unwrap();
        }
        etag.push(QUOTE);
        Self(etag.try_into().unwrap())
    }

    pub fn matches(&self, if_none_match_header: &[u8]) -> bool {
        let etag = self.0.as_bytes();
        if_none_match_header
            .windows(self.0.len())
            .any(|window| window == etag)
    }
}
