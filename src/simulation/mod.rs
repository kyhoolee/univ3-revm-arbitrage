mod revm;
mod state_db;
mod traits;

use alloy_primitives::{b256, B256};
pub use revm::*;
pub use traits::*;

pub const UNISWAP_V2_TOPIC: &B256 = &b256!("1c411e9a96e071241c2f21f7726b17ae89e3cab4c78be50e062b03a9fffbbad1");
