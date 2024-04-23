use std::sync::Arc;

use anyhow::Result;
use tokio::time::{Duration, Instant};

use crate::{config::IndexingStrategy, context::Context};

pub async fn blob_indexer_task(ctx: Arc<Context>) -> Result<()> {
    match ctx.indexing_strategy {
        IndexingStrategy::PollInterval(interval) => {
            let interval = Duration::from_secs(interval);

            loop {
                let next_update_time = Instant::now() + interval;

                update_commit_index(ctx.clone()).await?;

                if Instant::now() < next_update_time {
                    tokio::time::sleep_until(next_update_time).await;
                }
            }
        }
    }
}

pub async fn update_commit_index(ctx: Arc<Context>) -> Result<()> {
    let Context {
        db,
        octocrab,
        repos,
        ..
    } = &*ctx;

    for repo_filter in repos {
        let search_list = repo_filter.get_repo_search_list();

        let mut repo_list = vec![];

        for (owner, repo) in search_list {
            if let Some(repo) = repo {
                repo_list.push((owner, repo));
            } else {
                let repos = octocrab.orgs(owner.clone()).list_repos().send().await?;

                for repo in repos {
                    repo_list.push((owner.clone(), repo.name));
                }
            }
        }

        for (owner, repo) in repo_list {
            if repo_filter.is_match(&owner, &repo) {
                db.add_commits_for_repo(owner.clone(), repo.clone(), ctx.clone())
                    .await?;
            }
        }
    }

    Ok(())
}
