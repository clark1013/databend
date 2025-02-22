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

use chrono::DateTime;
use chrono::Utc;
use common_meta_app::background::BackgroundJobIdent;
use common_meta_app::background::BackgroundJobInfo;
use common_meta_app::background::BackgroundJobParams;
use common_meta_app::background::BackgroundJobState;
use common_meta_app::background::BackgroundJobState::FAILED;
use common_meta_app::background::BackgroundJobStatus;
use common_meta_app::background::BackgroundJobType::INTERVAL;
use common_meta_app::background::BackgroundJobType::ONESHOT;
use common_meta_app::background::BackgroundTaskIdent;
use common_meta_app::background::BackgroundTaskInfo;
use common_meta_app::background::BackgroundTaskState;
use common_meta_app::background::CreateBackgroundJobReq;
use common_meta_app::background::GetBackgroundJobReq;
use common_meta_app::background::GetBackgroundTaskReq;
use common_meta_app::background::ListBackgroundJobsReq;
use common_meta_app::background::ListBackgroundTasksReq;
use common_meta_app::background::UpdateBackgroundJobParamsReq;
use common_meta_app::background::UpdateBackgroundJobStatusReq;
use common_meta_app::background::UpdateBackgroundTaskReq;
use common_meta_kvapi::kvapi;
use common_meta_types::MetaError;
use tracing::info;

use crate::background_api::BackgroundApi;
use crate::SchemaApi;

fn new_background_task(
    state: BackgroundTaskState,
    created_at: DateTime<Utc>,
) -> BackgroundTaskInfo {
    BackgroundTaskInfo {
        last_updated: None,
        task_type: Default::default(),
        task_state: state,
        message: "".to_string(),
        compaction_task_stats: None,
        vacuum_stats: None,
        creator: None,
        created_at,
    }
}

fn new_background_job(state: BackgroundJobState, created_at: DateTime<Utc>) -> BackgroundJobInfo {
    BackgroundJobInfo {
        job_params: Some(BackgroundJobParams {
            job_type: ONESHOT,
            scheduled_job_interval: std::time::Duration::from_secs(0),
            scheduled_job_timezone: None,
            scheduled_job_cron: "".to_string(),
        }),
        last_updated: None,
        task_type: Default::default(),
        message: "".to_string(),
        creator: None,
        created_at,
        job_status: Some(BackgroundJobStatus {
            job_state: state,
            last_task_id: None,
            last_task_run_at: None,
            next_task_scheduled_time: None,
        }),
    }
}

/// Test suite of `BackgroundApi`.
///
/// It is not used by this crate, but is used by other crate that impl `ShareApi`,
/// to ensure an impl works as expected,
/// such as `meta/embedded` and `metasrv`.
#[derive(Copy, Clone)]
pub struct BackgroundApiTestSuite {}

impl BackgroundApiTestSuite {
    /// Test BackgroundTaskApi on a single node
    pub async fn test_single_node<B, MT>(b: B) -> anyhow::Result<()>
    where
        B: kvapi::ApiBuilder<MT>,
        MT: BackgroundApi + kvapi::AsKVApi<Error = MetaError> + SchemaApi,
    {
        let suite = BackgroundApiTestSuite {};

        suite.update_background_tasks(&b.build().await).await?;
        suite.update_background_jobs(&b.build().await).await?;
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn update_background_tasks<MT: BackgroundApi + kvapi::AsKVApi<Error = MetaError>>(
        &self,
        mt: &MT,
    ) -> anyhow::Result<()> {
        let tenant = "tenant1";
        let task_id = "uuid1";
        let task_name = BackgroundTaskIdent {
            tenant: tenant.to_string(),
            task_id: task_id.to_string(),
        };

        info!("--- list background tasks when their is no tasks");
        {
            let req = ListBackgroundTasksReq {
                tenant: tenant.to_string(),
            };

            let res = mt.list_background_tasks(req).await;
            assert!(res.is_ok());
            let resp = res.unwrap();
            assert!(resp.is_empty());
        }

        info!("--- create a background task");
        let create_on = Utc::now();
        // expire after 5 secs
        let expire_at = create_on + chrono::Duration::seconds(5);
        {
            let req = UpdateBackgroundTaskReq {
                task_name: task_name.clone(),
                task_info: new_background_task(BackgroundTaskState::STARTED, create_on),
                expire_at: expire_at.timestamp() as u64,
            };

            let res = mt.update_background_task(req).await;
            info!("update log res: {:?}", res);
            let res = mt
                .get_background_task(GetBackgroundTaskReq {
                    name: task_name.clone(),
                })
                .await;
            info!("get log res: {:?}", res);
            let res = res.unwrap();
            assert_eq!(
                BackgroundTaskState::STARTED,
                res.task_info.unwrap().task_state,
                "first state is started"
            );
        }
        {
            let req = UpdateBackgroundTaskReq {
                task_name: task_name.clone(),
                task_info: new_background_task(BackgroundTaskState::DONE, create_on),
                expire_at: expire_at.timestamp() as u64,
            };

            let res = mt.update_background_task(req).await;
            info!("update log res: {:?}", res);
            let res = mt
                .get_background_task(GetBackgroundTaskReq {
                    name: task_name.clone(),
                })
                .await;
            info!("get log res: {:?}", res);
            let res = res.unwrap();
            assert_eq!(
                BackgroundTaskState::DONE,
                res.task_info.unwrap().task_state,
                "first state is done"
            );
        }
        {
            let req = ListBackgroundTasksReq {
                tenant: tenant.to_string(),
            };

            let res = mt.list_background_tasks(req).await;
            info!("update log res: {:?}", res);
            let res = res.unwrap();
            assert_eq!(1, res.len(), "there is one task");
            assert_eq!(task_id, res[0].1, "task name");
            assert_eq!(
                BackgroundTaskState::DONE,
                res[0].2.task_state,
                "first state is done"
            );
        }
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn update_background_jobs<MT: BackgroundApi + kvapi::AsKVApi<Error = MetaError>>(
        &self,
        mt: &MT,
    ) -> anyhow::Result<()> {
        let tenant = "tenant1";
        let job_name = "uuid1";
        let job_ident = BackgroundJobIdent {
            tenant: tenant.to_string(),
            name: job_name.to_string(),
        };

        info!("--- list background jobs when their is no tasks");
        {
            let req = ListBackgroundJobsReq {
                tenant: tenant.to_string(),
            };

            let res = mt.list_background_jobs(req).await;
            assert!(res.is_ok());
            let resp = res.unwrap();
            assert!(resp.is_empty());
        }

        info!("--- create a background job");
        let create_on = Utc::now();
        {
            let req = CreateBackgroundJobReq {
                if_not_exists: true,
                job_name: job_ident.clone(),
                job_info: new_background_job(BackgroundJobState::RUNNING, create_on),
            };

            let res = mt.create_background_job(req).await;
            info!("update log res: {:?}", res);
            let res = mt
                .get_background_job(GetBackgroundJobReq {
                    name: job_ident.clone(),
                })
                .await;
            info!("get log res: {:?}", res);
            let res = res.unwrap();
            assert_eq!(
                BackgroundJobState::RUNNING,
                res.info.job_status.unwrap().job_state,
                "first state is started"
            );
        }

        info!("--- update a background job params");
        {
            let req = UpdateBackgroundJobParamsReq {
                job_name: job_ident.clone(),
                params: BackgroundJobParams {
                    job_type: INTERVAL,
                    scheduled_job_interval: std::time::Duration::from_secs(3600),
                    scheduled_job_cron: "".to_string(),
                    scheduled_job_timezone: None,
                },
            };

            let res = mt.update_background_job_params(req).await;
            info!("update log res: {:?}", res);
            let res = mt
                .get_background_job(GetBackgroundJobReq {
                    name: job_ident.clone(),
                })
                .await;
            info!("get log res: {:?}", res);
            let res = res.unwrap();
            assert_eq!(
                INTERVAL,
                res.info.job_params.clone().unwrap().job_type,
                "first state is started"
            );
            assert_eq!(
                std::time::Duration::from_secs(3600),
                res.info.job_params.unwrap().scheduled_job_interval
            )
        }

        info!("--- update a background job params");
        {
            let req = UpdateBackgroundJobStatusReq {
                job_name: job_ident.clone(),
                status: BackgroundJobStatus {
                    job_state: FAILED,
                    last_task_id: Some("newid".to_string()),
                    last_task_run_at: None,
                    next_task_scheduled_time: None,
                },
            };

            let res = mt.update_background_job_status(req).await;
            info!("update log res: {:?}", res);
            let res = mt
                .get_background_job(GetBackgroundJobReq {
                    name: job_ident.clone(),
                })
                .await;
            info!("get log res: {:?}", res);
            let res = res.unwrap();
            assert_eq!(
                FAILED,
                res.info.job_status.clone().unwrap().job_state,
                "first state is started"
            );
            assert_eq!(
                Some("newid".to_string()),
                res.info.job_status.unwrap().last_task_id
            )
        }

        info!("--- list background jobs when their is 1 tasks");
        {
            let req = ListBackgroundJobsReq {
                tenant: tenant.to_string(),
            };

            let res = mt.list_background_jobs(req).await;
            assert!(res.is_ok());
            let resp = res.unwrap();
            assert_eq!(1, resp.len());
            assert_eq!(job_ident.name, resp[0].1, "expect same ident name");
            assert_eq!(
                BackgroundJobState::FAILED,
                resp[0].2.job_status.clone().unwrap().job_state,
                "first state is started"
            );
            assert_eq!(
                INTERVAL,
                resp[0].2.job_params.clone().unwrap().job_type,
                "first state is started"
            );
        }
        Ok(())
    }
}
