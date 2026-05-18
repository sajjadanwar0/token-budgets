//! TokenEstimator trait + ByteLength / Tiktoken / Anthropic implementations.
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
    fn name(&self) -> &'static str {
        "byte-length"
    }
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
        Ok(Self {
            bpe,
            name: "tiktoken/cl100k_base",
        })
    }

    /// Construct for the o200k_base encoding (GPT-4o family).
    pub fn o200k_base() -> anyhow::Result<Self> {
        let bpe = tiktoken_rs::o200k_base()?;
        Ok(Self {
            bpe,
            name: "tiktoken/o200k_base",
        })
    }
}

#[cfg(feature = "tiktoken")]
impl TokenEstimator for Tiktoken {
    fn estimate(&self, prompt: &str) -> u64 {
        self.bpe.encode_with_special_tokens(prompt).len() as u64
    }
    fn name(&self) -> &'static str {
        self.name
    }
}

// =============================================================
// AnthropicEstimator with configurable safety_margin
// =============================================================

/// Anthropic-targeted estimator with a configurable safety margin.
///
/// Wraps an underlying [`TokenEstimator`] and multiplies its output by
/// a margin to absorb *server-side expansion* that the client-side
/// tokenizer cannot predict. The cap-soundness contract for the
/// wrapped estimator is preserved under the assumption that the
/// product (base_estimate * margin) >= billable_tokens.
///
/// # Design contract
///
/// For A1 to hold under this estimator, the deployment must satisfy:
///
///   base.estimate(p) * safety_margin >= billable_tokens_anthropic(p)
///
/// for every prompt p actually transmitted. The default margin
/// (2.0) is calibrated from the adversarial audit at
/// experiments/anthropic_adversarial/, which measured the worst-case
/// gap between byte-length and Anthropic's billed input across seven
/// hypothesised under-counting paths (worst observed: 1.875x on
/// nested tool schemas).
///
/// # Configuration
///
/// - [`AnthropicEstimator::new`] uses [`ByteLength`] as the base
///   estimator and a 2.0x safety margin. This is the recommended
///   default for operators who have not run their own audit.
///
/// - [`AnthropicEstimator::with_base`] lets you wrap any
///   [`TokenEstimator`] (e.g., a tokenizer-direct estimator) with
///   the audit-calibrated 2.0x margin. Operators with a tokenizer-
///   direct base may wish to combine this with `with_margin` to
///   tighten the bound.
///
/// - [`AnthropicEstimator::with_margin`] lets you tune the margin
///   independently. Panics if margin < 1.0 because that would make
///   the estimator unsound.
///
/// - [`AnthropicEstimator::tight`] returns an unmargined wrapper.
///   Use only when the deployment has audited its prompt classes
///   against server-side expansion and confirmed the tight bound
///   holds.
pub struct AnthropicEstimator<E: TokenEstimator = ByteLength> {
    base: E,
    safety_margin: f64,
}

impl AnthropicEstimator<ByteLength> {
    /// Default: byte-length base + 2.0x safety margin.
    ///
    /// The 2.0x margin is calibrated by the adversarial audit at
    /// experiments/anthropic_adversarial/. The audit found A1
    /// violations at 1.05x margin on two classes:
    ///   - nested_tool_schema: 1.875x margin needed
    ///   - unicode_dense_tool_desc: 1.24x margin needed
    /// 2.0x absorbs the worst observed case with ~6% headroom and
    /// covers the audited prompt corpus uniformly. Operators with
    /// audited prompt classes that exclude deeply-nested schemas
    /// and dense Unicode tool descriptions may tighten via
    /// `with_margin()`.
    pub fn new() -> Self {
        Self {
            base: ByteLength,
            safety_margin: 2.0,
        }
    }
}

impl Default for AnthropicEstimator<ByteLength> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: TokenEstimator> AnthropicEstimator<E> {
    /// Wrap a custom base estimator with the audit-calibrated 2.0x
    /// safety margin. Use this to compose AnthropicEstimator with a
    /// tokenizer-direct base (e.g., Tiktoken under the `tiktoken`
    /// feature). Note: the 2.0x margin was calibrated against the
    /// byte-length base; with a tokenizer-direct base that more
    /// closely matches Anthropic's billing, a smaller margin
    /// (typically 1.10-1.25x) is usually sufficient and can be
    /// configured via `with_margin`.
    pub fn with_base(base: E) -> Self {
        Self {
            base,
            safety_margin: 2.0,
        }
    }

    /// Construct with a specific margin. Panics if margin < 1.0
    /// (would make the estimator unsound).
    pub fn with_margin(base: E, margin: f64) -> Self {
        assert!(
            margin >= 1.0,
            "safety_margin must be >= 1.0 to preserve A1; got {}",
            margin
        );
        Self {
            base,
            safety_margin: margin,
        }
    }

    /// Tight bound: no margin. Use only when the deployment has
    /// audited its prompt classes against server-side expansion and
    /// confirmed the tight bound holds.
    pub fn tight(base: E) -> Self {
        Self {
            base,
            safety_margin: 1.0,
        }
    }

    /// Inspect the configured margin (for logging / reproducibility).
    pub fn margin(&self) -> f64 {
        self.safety_margin
    }
}

impl<E: TokenEstimator> TokenEstimator for AnthropicEstimator<E> {
    fn estimate(&self, prompt: &str) -> u64 {
        let raw = self.base.estimate(prompt);
        // Apply the safety margin. Ceil to avoid losing the margin to
        // integer truncation on small prompts.
        ((raw as f64) * self.safety_margin).ceil() as u64
    }

    fn name(&self) -> &'static str {
        // Static-str constraint of the trait prevents formatting the
        // margin into the name. Operators that need per-instance
        // descriptors should log .margin() separately.
        "anthropic/byte-length+margin"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_length_basic() {
        let e = ByteLength;
        assert_eq!(e.estimate("hello"), 5);
        assert_eq!(e.estimate(""), 0);
        // Multibyte: "café" is 5 UTF-8 bytes (c=1, a=1, f=1, é=2).
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
            "the quick brown fox jumps over the lazy dog",
            "abcdefghijklmnopqrstuvwxyz0123456789",
        ] {
            let bl_est = bl.estimate(input);
            let tk_est = tk.estimate(input);
            assert!(
                bl_est >= tk_est,
                "byte_length ({}) < tiktoken ({}) for {:?}",
                bl_est,
                tk_est,
                input
            );
        }
    }

    /// The default constructor must produce a 2.0x margin (audit-
    /// calibrated). If this assertion changes, the paper's §5.14 and
    /// §5.15 numbers also need to change.
    #[test]
    fn anthropic_default_is_two_point_zero() {
        let e = AnthropicEstimator::new();
        // 100 byte prompt * 2.0 = 200
        let prompt: String = "A".repeat(100);
        assert_eq!(e.estimate(&prompt), 200);
        assert_eq!(e.margin(), 2.0);
    }

    /// `Default::default()` must be functionally identical to `new()`.
    #[test]
    fn anthropic_default_trait_matches_new() {
        let a = AnthropicEstimator::new();
        let b: AnthropicEstimator<ByteLength> = AnthropicEstimator::default();
        assert_eq!(a.margin(), b.margin());
        let prompt = "the quick brown fox";
        assert_eq!(a.estimate(prompt), b.estimate(prompt));
    }

    /// `with_base` also defaults to the 2.0x calibrated margin.
    #[test]
    fn anthropic_with_base_uses_calibrated_margin() {
        let e = AnthropicEstimator::with_base(ByteLength);
        let prompt: String = "B".repeat(50);
        assert_eq!(e.estimate(&prompt), 100); // 50 * 2.0
        assert_eq!(e.margin(), 2.0);
    }

    #[test]
    fn anthropic_with_margin_panics_below_one() {
        let result = std::panic::catch_unwind(|| {
            AnthropicEstimator::with_margin(ByteLength, 0.99)
        });
        assert!(result.is_err(), "with_margin(0.99) should panic");
    }

    #[test]
    fn anthropic_with_margin_accepts_one_point_zero() {
        // Exact 1.0 is the boundary case; should NOT panic.
        let e = AnthropicEstimator::with_margin(ByteLength, 1.0);
        assert_eq!(e.estimate("hello"), 5);
        assert_eq!(e.margin(), 1.0);
    }

    #[test]
    fn anthropic_tight_is_no_op_margin() {
        let e = AnthropicEstimator::tight(ByteLength);
        assert_eq!(e.estimate("A"), 1);
        assert_eq!(e.estimate(&"A".repeat(1000)), 1000);
        assert_eq!(e.margin(), 1.0);
    }

    #[test]
    fn anthropic_dominates_base_estimator() {
        // The whole point of the margin: the AnthropicEstimator must
        // never under-count relative to its base.
        let base = ByteLength;
        let wrapped = AnthropicEstimator::new();
        for input in &[
            "hello",
            "the quick brown fox jumps over the lazy dog",
            "",
            "{ \"role\": \"user\", \"content\": \"test\" }",
        ] {
            let base_est = base.estimate(input);
            let wrapped_est = wrapped.estimate(input);
            assert!(
                wrapped_est >= base_est,
                "AnthropicEstimator ({}) < base ({}) for {:?}",
                wrapped_est,
                base_est,
                input
            );
        }
    }

    /// If we used floor or integer truncation, a 1-byte prompt with
    /// a 2.0x margin would still round to 2, but with a 1.05x margin
    /// it would lose the margin entirely. Ceil ensures the margin is
    /// preserved on small inputs regardless of the configured value.
    #[test]
    fn anthropic_ceil_preserves_margin_on_small_inputs() {
        // Default (2.0x): 1 byte * 2.0 = 2.0 -> ceil -> 2
        let e_default = AnthropicEstimator::new();
        assert_eq!(e_default.estimate("A"), 2);

        // Tighter (1.05x): 1 byte * 1.05 = 1.05 -> ceil -> 2
        // This is the case ceil is essential for: with floor, the
        // margin would be lost on the 1-byte input.
        let e_tight = AnthropicEstimator::with_margin(ByteLength, 1.05);
        assert_eq!(e_tight.estimate("A"), 2);

        // Exact 1.0x: 1 byte * 1.0 = 1.0 -> ceil -> 1
        let e_one = AnthropicEstimator::tight(ByteLength);
        assert_eq!(e_one.estimate("A"), 1);
    }
}