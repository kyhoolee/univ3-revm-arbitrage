use anyhow::Error;
use e_primitives::structs::{State, Transaction};
use revm::primitives::{ExecutionResult, ResultAndState};

pub trait SimulationStrategyTrait {
    /// Executes a transaction.
    ///
    /// # Arguments
    ///
    /// * `tx` - A reference to the `Transaction` struct that represents the transaction to be executed.
    ///
    /// # Returns
    ///
    /// * `Result<ResultAndState, Error>` - Returns a `Result` containing either the `ResultAndState` if the transaction is successful, or an `Error` if the transaction fails.
    fn transact(&mut self, tx: &Transaction) -> Result<ResultAndState, Error>;
    /// Executes and commits a transaction.
    ///
    /// # Arguments
    ///
    /// * `tx` - The transaction to be executed and committed.
    ///
    /// # Returns
    ///
    /// * `Result<ExecutionResult, Error>` - Returns the result of the execution if successful, otherwise returns an `Error`.
    fn transact_commit(&mut self, tx: &Transaction) -> Result<ExecutionResult, Error>;

    fn get_state(&self) -> State;
}
