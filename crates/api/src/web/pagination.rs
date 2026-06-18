//! List pagination: the `?page=&per_page=` query params and the `{data, page,
//! per_page, total}` response envelope used by every list endpoint.

use serde::{Deserialize, Serialize};

const DEFAULT_PER_PAGE: i64 = 20;
const MAX_PER_PAGE: i64 = 100;

#[derive(Debug, Deserialize)]
pub struct PageParams {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

impl PageParams {
    /// Resolve to `(limit, offset, page, per_page)`, clamped to sane bounds.
    pub fn resolve(&self) -> (i64, i64, i64, i64) {
        let page = self.page.unwrap_or(1).max(1);
        let per_page = self
            .per_page
            .unwrap_or(DEFAULT_PER_PAGE)
            .clamp(1, MAX_PER_PAGE);
        let offset = (page - 1) * per_page;
        (per_page, offset, page, per_page)
    }
}

/// The list response envelope.
#[derive(Debug, Serialize)]
pub struct Page<T> {
    pub data: Vec<T>,
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
}

impl<T> Page<T> {
    pub fn new(data: Vec<T>, page: i64, per_page: i64, total: i64) -> Self {
        Self {
            data,
            page,
            per_page,
            total,
        }
    }
}

/// A non-paginated list envelope: `{ "data": [...] }`.
#[derive(Debug, Serialize)]
pub struct Data<T> {
    pub data: Vec<T>,
}

impl<T> Data<T> {
    pub fn new(data: Vec<T>) -> Self {
        Self { data }
    }
}
