// Copyright 2021 Datafuse Labs
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::sync::Arc;

use common_catalog::table_context::TableContext;
use common_exception::Result;
use common_meta_kvapi::kvapi::KVApi;
use common_users::UserApiProvider;

/// Checks if a duplicate label exists in the meta store.
///
/// # Arguments
///
/// * `ctx` - The table context. Must implement the `TableContext` trait and be wrapped in an `Arc`.
///
/// # Returns
///
/// Returns a `Result` containing a `bool` indicating whether specific duplicate label exists (`true`) or not (`false`).
pub async fn check_deduplicate_label(ctx: Arc<dyn TableContext>) -> Result<bool> {
    match ctx.get_settings().get_deduplicate_label()? {
        None => Ok(false),
        Some(deduplicate_label) => {
            let kv_store = UserApiProvider::instance().get_meta_store_client();
            let raw = kv_store.get_kv(&deduplicate_label).await?;
            match raw {
                None => Ok(false),
                Some(_) => Ok(true),
            }
        }
    }
}
