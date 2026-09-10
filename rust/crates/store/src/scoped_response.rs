//! Shaping a SurrealDB response back into what a caller asked for.
//!
//! Two jobs, both about the RESULT rather than the query, which is why they are not in `open.rs`:
//!
//!   * [`ScopedResponse`] hides the `USE NS` that `query_ws` prepends. Every caller selects
//!     statements 0-based over its OWN statements and never learns the wall exists.
//!   * [`check_absent_table_as_empty`] restores SurrealDB 2's answer for a read of a table that was
//!     never written — no rows, rather than an error.

use crate::open::StoreError;

/// Raise the first real statement error, but treat "the table does not exist" as **no rows**.
///
/// SurrealDB 2 answered a `SELECT`/`UPDATE`/`DELETE` against a table that had never been written
/// with an empty result. SurrealDB 3 raises `NotFoundError::Table` instead: `dbs/iterator.rs`
/// bails whenever `Statement::requires_table_existence()` — true for `Select`, `Update`, `Delete`,
/// `Live`, `Show` and `Access` — finds no catalog entry. It is unconditional; no strict-mode or
/// capability flag turns it off.
///
/// lb reads before it writes all the time: a fresh node, an empty workspace, any `*.list` verb.
/// lb is also schemaless by design — a table springs into existence on first `CREATE`, and 42 query
/// sites bind the table name at runtime, so a central "DEFINE TABLE every name at boot" list would
/// be both incomplete and exactly the per-name special-casing Rule 10 forbids. So the contract "a
/// read of a table nothing has written yields nothing" is lb-store's to keep, and it is kept here.
///
/// Dropping the errored slot is what makes the caller side free: `take` on a missing statement index
/// already returns `vec![]` for `Vec<T>`, `None` for `Option<T>` and `Value::None` for `Value`
/// (`surrealdb::opt::query`) — byte-for-byte what SurrealDB 2 handed back for an empty table.
///
/// This narrows nothing that was previously reported: under SurrealDB 2 a misspelled table name
/// also read as empty rather than erroring. Matching is on the TYPED detail, never on the message
/// text.
pub(crate) fn check_absent_table_as_empty(
    mut resp: surrealdb::IndexedResults,
) -> Result<surrealdb::IndexedResults, surrealdb::Error> {
    // `take_errors` drains EVERY errored slot, so re-raise the lowest-indexed survivor to keep
    // `check()`'s "first error wins" ordering — a HashMap would otherwise report an arbitrary one.
    let mut real: Vec<_> = resp
        .take_errors()
        .into_iter()
        .filter(|(_, e)| {
            !matches!(
                e.not_found_details(),
                Some(surrealdb::types::NotFoundError::Table { .. })
            )
        })
        .collect();
    real.sort_by_key(|(i, _)| *i);
    match real.into_iter().next() {
        Some((_, e)) => Err(e),
        None => Ok(resp),
    }
}

/// A statement selector for [`ScopedResponse::take`], shifted by one to skip the injected `USE` at
/// real index 0. Mirrors the selectors SurrealDB's `Response::take` accepts — a statement index
/// (`usize`), a field of the first statement (`&str`), or a field of statement N (`(usize, &str)`) —
/// so every existing caller idiom works verbatim while the caller's index 0 maps to real index 1.
pub trait ScopedIndex {
    /// The real (USE-inclusive) selector this caller-facing one maps to.
    type Shifted;
    fn shift(self) -> Self::Shifted;
}
impl ScopedIndex for usize {
    type Shifted = usize;
    fn shift(self) -> usize {
        self + 1
    }
}
impl<'a> ScopedIndex for &'a str {
    type Shifted = (usize, &'a str);
    fn shift(self) -> (usize, &'a str) {
        // `take("field")` means "field of the caller's FIRST statement" — real statement 1.
        (1, self)
    }
}
impl<'a> ScopedIndex for (usize, &'a str) {
    type Shifted = (usize, &'a str);
    fn shift(self) -> (usize, &'a str) {
        (self.0 + 1, self.1)
    }
}

/// The result of a scoped store query. Wraps SurrealDB's `Response` and hides the leading `USE`
/// statement's result slot: `take(0)` returns the caller's FIRST statement (the USE lives at the
/// real index 0), so every one of the ~140 `query_ws` callers keeps its existing selectors.
pub struct ScopedResponse(pub(crate) surrealdb::IndexedResults);

impl ScopedResponse {
    /// Extract a result selected 0-based over the caller's OWN statements (the injected `USE` at real
    /// index 0 is invisible here). Accepts the same selectors as `Response::take`, each shifted past
    /// the USE by [`ScopedIndex`].
    // The selector is an `impl ScopedIndex` ARGUMENT (a hidden generic), so `R` is the only turbofish
    // param — `take::<Vec<Foo>>(0)` binds the result type exactly as `Response::take::<Vec<Foo>>(0)`
    // does. The associated-type bound threads the shifted selector into SurrealDB's `QueryResult`.
    // `surrealdb::Error` is ~144 bytes and is NOT ours to box: it is the type every one of the ~140
    // `query_ws` callers already matches on, so wrapping it here would be an API break across the
    // workspace to move bytes we do not own.
    #[allow(clippy::result_large_err)]
    pub fn take<R: surrealdb::types::SurrealValue>(
        &mut self,
        index: impl ScopedIndex<Shifted: surrealdb::opt::QueryResult<R>>,
    ) -> Result<R, surrealdb::Error> {
        self.0.take(index.shift())
    }

    /// The number of the caller's OWN statements (the injected `USE` is not counted).
    pub fn num_statements(&self) -> usize {
        self.0.num_statements().saturating_sub(1)
    }

    /// Extract the value of the `RETURN` in a `BEGIN … RETURN x; COMMIT;` transaction.
    ///
    /// SurrealDB 3 gives a transaction **one result slot per statement**. SurrealDB 2 collapsed the
    /// whole transaction to a single slot holding the `RETURN` value, so callers read it with
    /// `take(0)` — which now lands on `BEGIN` and yields `Null`. Where that value was a count read
    /// through `unwrap_or(0)`, the result was silent and wrong: `lb_jobs::retain` deleted 45 rows
    /// and reported 0. A retention verb that under-reports is exactly how a disc fills up quietly,
    /// so this exists to stop every caller hand-rolling an index.
    ///
    /// The `RETURN` is the statement immediately before `COMMIT`, so it is the second-to-last slot.
    /// Errors rather than defaulting when the shape is not what we expect.
    pub fn take_transaction_return<R>(&mut self) -> Result<R, StoreError>
    where
        R: surrealdb::types::SurrealValue,
        // The selector is a plain index, so it must be a `QueryResult` for `R` — the same bound
        // `take` derives through `ScopedIndex`, restated here because the index is chosen inside.
        usize: surrealdb::opt::QueryResult<R>,
    {
        let n = self.num_statements();
        // BEGIN … RETURN, COMMIT — at minimum BEGIN + RETURN + COMMIT.
        let idx = n.checked_sub(2).ok_or_else(|| {
            StoreError::Decode(format!(
                "expected a `BEGIN … RETURN x; COMMIT;` transaction, got {n} statement(s)"
            ))
        })?;
        self.take::<R>(idx)
            .map_err(|e| StoreError::Decode(e.to_string()))
    }

    /// Surface any statement error. `query_ws` already `check`s internally, so this is a no-op that
    /// preserves the `…await?.check()?` caller idiom.
    #[allow(clippy::result_large_err)] // see `take` above — the error type is surrealdb's.
    pub fn check(self) -> Result<Self, surrealdb::Error> {
        Ok(self)
    }
}
