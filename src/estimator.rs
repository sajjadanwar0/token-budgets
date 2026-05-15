//! TokenEstimator trait + ByteLength / Tiktoken implementations.
//!
//! Drop into budget-typed-cap/src/estimator.rs and re-export from lib.rs.
//! Add to call_with_budget signature as a generic parameter (or as a
//! trait-object &dyn TokenEstimator if the operator wants late binding).
//!
//! Cargo.toml addition:
//!   [features]
//!   tiktoken = ["dep:tiktoken-rs"]
//!   [dependencies]
//!   tiktoken-rs = { version = "0.5", optional = true }

/// Sound estimator interface for input-token counts.
///
/// The cap-soundness contract requires:
///   self.estimate(p) >= tokenizer.encode(p).len()
/// for every prompt p the operator may submit. The byte-length default
/// (ByteLength) satisfies this for any BPE tokenizer; tokenizer-direct
/// implementations satisfy it for the specific tokenizer they wrap, with
/// the caveat that provider-side tokenizer rotation can violate the
/// contract if the implementation is not pinned.
pub trait TokenEstimator: Send + Sync {
    fn estimate(&self, prompt: &str) -> u64;

    /// Optional descriptor for logging / reproducibility.
    fn name(&self) -> &'static str {
        "anonymous"
    }
}

/// Conservative default: UTF-8 byte count.
///
/// Sound for any BPE-family tokenizer. Loose: median over-reservation
/// approximately 6.4x across our tested provider/model combinations.
pub struct ByteLength;

impl TokenEstimator for ByteLength {
    fn estimate(&self, prompt: &str) -> u64 {
        prompt.len() as u64
    }
    fn name(&self) -> &'static str { "byte-length" }
}

/// Tokenizer-direct estimator using tiktoken-rs.
///
/// Tighter than byte-length: median over-reservation expected to drop
/// from ~6.4x to ~1.4x for OpenAI-family models.
///
/// SOUNDNESS NOTE: this is only sound while the wrapped tokenizer
/// matches the provider's actual tokenizer at request time. If the
/// provider rotates the tokenizer without bumping tiktoken-rs, this
/// estimator may under-count and break cap-soundness. Operators using
/// this estimator MUST pin both the model and the tokenizer library
/// version, and re-validate after any provider model update.
#[cfg(feature = "tiktoken")]
pub struct Tiktoken {
    bpe: tiktoken_rs::CoreBPE,
    name: &'static str,
}

#[cfg(feature = "tiktoken")]
impl Tiktoken {
    /// Construct for the cl100k_base encoding (GPT-4 family).
    pub fn cl100k_base() -> anyhow::Result<Self> {
        let bpe = tiktoken_rs::cl100k_base()?;
        Ok(Self { bpe, name: "tiktoken/cl100k_base" })
    }

    /// Construct for the o200k_base encoding (GPT-4o family).
    pub fn o200k_base() -> anyhow::Result<Self> {
        let bpe = tiktoken_rs::o200k_base()?;
        Ok(Self { bpe, name: "tiktoken/o200k_base" })
    }
}

#[cfg(feature = "tiktoken")]
impl TokenEstimator for Tiktoken {
    fn estimate(&self, prompt: &str) -> u64 {
        // tiktoken returns Vec<usize>; use the actual count.
        self.bpe.encode_with_special_tokens(prompt).len() as u64
    }
    fn name(&self) -> &'static str { self.name }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_length_basic() {
        let e = ByteLength;
        assert_eq!(e.estimate("hello"), 5);
        assert_eq!(e.estimate(""), 0);
        // Multibyte: "café" is 5 UTF-8 bytes (c,a,f,e-acute=2 bytes).
        assert_eq!(e.estimate("café"), 5);
    }

    /// Contract check: byte length is an upper bound on tiktoken count
    /// for a variety of inputs. This is a regression check, not an
    /// exhaustive proof.
    #[cfg(feature = "tiktoken")]
    #[test]
    fn byte_length_dominates_tiktoken() {
        let bl = ByteLength;
        let tk = Tiktoken::cl100k_base().unwrap();
        for input in &[
            "hello world",
            "What's 17 * 23?",
            "Plan a 7-day trip to Tokyo with daily themes.",
            "{ \"role\": \"user\", \"content\": \"test\" }",
            "🎉🎊✨ emoji-dense content here 🎈🎁",
            "日本語のテキストもテストする必要がある",
        ] {
            let bl_est = bl.estimate(input);
            let tk_est = tk.estimate(input);
            assert!(bl_est >= tk_est,
                    "byte_length ({}) < tiktoken ({}) for {:?}",
                    bl_est, tk_est, input);
        }
    }
}