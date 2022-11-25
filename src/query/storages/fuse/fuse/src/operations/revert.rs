//  Copyright 2021 Datafuse Labs.
//
//  Licensed under the Apache License, Version 2.0 (the "License");
//  you may not use this file except in compliance with the License.
//  You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
//  Unless required by applicable law or agreed to in writing, software
//  distributed under the License is distributed on an "AS IS" BASIS,
//  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//  See the License for the specific language governing permissions and
//  limitations under the License.

use common_catalog::table::NavigationPoint;
use common_catalog::table::Table;
use common_catalog::table_context::TableContext;
use common_exception::ErrorCode;
use common_exception::Result;
use common_meta_app::schema::UpdateTableMetaReq;
use common_meta_types::MatchSeq;

use crate::FuseTable;

impl FuseTable {
    pub async fn do_revert_to(
        &self,
        ctx: &dyn TableContext,
        point: &NavigationPoint,
    ) -> Result<()> {
        // 0. try navigate to the point
        let table = self.navigate_to(point).await?;
        let fuse_table = FuseTable::try_from_table(table.as_ref())?;
        let table_info = fuse_table.get_table_info();

        // 1. prepare table meta which being reverted to
        let table_meta_to_be_committed = fuse_table.table_info.meta.clone();

        // 2. prepare the request
        //  using the CURRENT version as the base table version
        let base_version = self.table_info.ident.seq;
        let catalog = ctx.get_catalog(&table_info.meta.catalog)?;
        let table_id = table_info.ident.table_id;
        let req = UpdateTableMetaReq {
            table_id,
            seq: MatchSeq::Exact(base_version),
            new_table_meta: table_meta_to_be_committed,
        };

        // 3. let's roll
        let tenant = ctx.get_tenant();
        let db_name = ctx.get_current_database(); // TODO is this SAFE?
        // 3.1 revert table meta
        let reply = catalog.update_table_meta(&tenant, &db_name, req).await;
        if reply.is_ok() {
            // try keep the snapshot hit
            let snapshot_location = fuse_table.snapshot_loc().await?.ok_or_else(|| {
                    ErrorCode::Internal("internal error, fuse table which navigated to given point has no snapshot location")
                })?;
            Self::write_last_snapshot_hint(
                &fuse_table.operator,
                &fuse_table.meta_location_generator,
                snapshot_location,
            )
            .await;
        };

        // ignore the reply of meta service
        reply.map(|_| ())
    }
}
