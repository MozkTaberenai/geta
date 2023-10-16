#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Encoding {
    Identity,
    Br,
    Gzip,
    Deflate,
}

impl std::fmt::Display for Encoding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Encoding {
    pub fn as_bytes(&self) -> &'static [u8] {
        self.as_str().as_bytes()
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Br => "br",
            Self::Gzip => "gzip",
            Self::Deflate => "deflate",
        }
    }

    pub fn is_contained_in(&self, target: impl AsRef<[u8]>) -> bool {
        let pat = self.as_bytes();
        target
            .as_ref()
            .windows(pat.len())
            .any(|window| window == pat)
    }
}

impl From<Encoding> for http::HeaderValue {
    fn from(encoding: Encoding) -> Self {
        http::HeaderValue::from_static(encoding.as_str())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        let hv = http::HeaderValue::from_static("br, gzip");
        assert!(Encoding::Br.is_contained_in(&hv));
        assert!(Encoding::Gzip.is_contained_in(&hv));
        assert!(!Encoding::Identity.is_contained_in(&hv));
        assert!(!Encoding::Deflate.is_contained_in(&hv));
        // assert!(!Encoding::Zstd.is_contained_in(&hv));
    }
}
