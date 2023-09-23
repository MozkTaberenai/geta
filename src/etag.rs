use bytes::{Buf, Bytes};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ETag(pub Bytes);

impl AsRef<[u8]> for ETag {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl<T: Buf> From<T> for ETag {
    fn from(mut buf: T) -> Self {
        let mut ctx = ring::digest::Context::new(&ring::digest::SHA256);
        while buf.has_remaining() {
            let chunk = buf.chunk();
            ctx.update(chunk);
            buf.advance(chunk.len());
        }
        let digest = ctx.finish();
        Self::new(digest)
    }
}

impl ETag {
    pub fn empty() -> Self {
        Self(Bytes::from_static(br#""""#))
    }

    pub fn new(digest: ring::digest::Digest) -> Self {
        use std::io::Write;
        let digest = digest.as_ref();
        let mut etag = Vec::with_capacity(digest.len() * 2 + 2);
        write!(etag, "\"").unwrap();
        for byte in digest {
            write!(etag, "{byte:02x}").unwrap();
        }
        write!(etag, "\"").unwrap();
        Self(Bytes::from(etag))
    }

    pub fn matches(&self, if_none_match_header: &[u8]) -> bool {
        if_none_match_header
            .windows(self.0.len())
            .any(|window| window == self.0)
    }
}
