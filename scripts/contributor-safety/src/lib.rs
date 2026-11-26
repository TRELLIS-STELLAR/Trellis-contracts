#![no_std]
// Exercise production sources without building unrelated workspace modules.
#[path = "../../../shared/src/accessibility.rs"]
pub mod accessibility;
#[path = "../../../shared/src/errors.rs"]
pub mod errors;
#[path = "../../../shared/src/storage.rs"]
pub mod storage;
#[path = "../../../shared/src/auth.rs"]
pub mod auth;
#[path = "../../../shared/src/history.rs"]
pub mod history;
#[path = "../../../shared/src/ownership_history.rs"]
pub mod ownership_history;
#[path = "../../../shared/src/risk_scoring.rs"]
pub mod risk_scoring;
