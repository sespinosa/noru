use anyhow::Result;

use crate::types::AuthStatus;

pub struct Auth;

impl Auth {
    pub fn new() -> Self {
        Self
    }

    pub fn status(&self) -> Result<AuthStatus> {
        unimplemented!("auth::Auth::status — Phase 2 auth-ai teammate")
    }

    pub fn start_sign_in(&self) -> Result<String> {
        unimplemented!("auth::Auth::start_sign_in — Phase 2 auth-ai teammate")
    }

    pub fn complete_sign_in(&self, _callback_url: &str) -> Result<AuthStatus> {
        unimplemented!("auth::Auth::complete_sign_in — Phase 2 auth-ai teammate")
    }

    pub fn sign_out(&self) -> Result<()> {
        unimplemented!("auth::Auth::sign_out — Phase 2 auth-ai teammate")
    }

    pub fn access_token(&self) -> Result<String> {
        unimplemented!("auth::Auth::access_token — Phase 2 auth-ai teammate")
    }
}

impl Default for Auth {
    fn default() -> Self {
        Self::new()
    }
}
