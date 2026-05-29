use core::marker::PhantomData;

mod sealed {
    pub struct CapAuthorityToken {
        _private: (),
    }

    impl CapAuthorityToken {
        #[cfg(feature = "system-authority")]
        pub(super) fn new_sealed() -> Self {
            Self { _private: () }
        }
    }
}

pub struct CapAuthority {
    _seal: sealed::CapAuthorityToken,
    _not_send_sync: PhantomData<*const ()>,
}

impl CapAuthority {
    #[cfg(feature = "system-authority")]
    #[must_use]
    pub fn seal_at_startup() -> Self {
        Self {
            _seal: sealed::CapAuthorityToken::new_sealed(),
            _not_send_sync: PhantomData,
        }
    }

    #[cfg(feature = "system-authority")]
    #[must_use]
    pub fn take_authority() -> Self {
        Self::seal_at_startup()
    }
}

pub type BudgetMint = CapAuthority;

#[cfg(all(test, feature = "system-authority"))]
mod tests {
    use super::*;

    #[test]
    fn mint_can_be_acquired() {
        let _mint = BudgetMint::take_authority();
    }

    #[test]
    fn cap_authority_alias_works() {
        let _auth = CapAuthority::seal_at_startup();
    }
}