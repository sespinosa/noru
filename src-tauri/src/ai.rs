use anyhow::Result;

use crate::auth::Auth;
use crate::types::Transcript;

pub struct Ai<'a> {
    #[allow(dead_code)]
    auth: &'a Auth,
}

impl<'a> Ai<'a> {
    pub fn new(auth: &'a Auth) -> Self {
        Self { auth }
    }

    pub fn summarize(&self, _transcript: &Transcript) -> Result<String> {
        unimplemented!("ai::summarize — Phase 2 auth-ai teammate")
    }

    pub fn action_items(&self, _transcript: &Transcript) -> Result<Vec<String>> {
        unimplemented!("ai::action_items — Phase 2 auth-ai teammate")
    }

    pub fn key_decisions(&self, _transcript: &Transcript) -> Result<Vec<String>> {
        unimplemented!("ai::key_decisions — Phase 2 auth-ai teammate")
    }
}
