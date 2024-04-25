use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
use cid::{multihash::Multihash, Cid};
use redb::{MultimapTableDefinition, ReadableMultimapTable};
use tabled::{
    settings::{Alignment, Style},
    Table, Tabled,
};

use crate::context::Context;
type Sha1Bytes = [u8; 20];

type RepoIdTuple = (String, String); // (owner, repo)

#[derive(Debug, Clone)]
pub struct RepoId {
    pub owner: String,
    pub repo: String,
}

impl From<RepoIdTuple> for RepoId {
    fn from(repo_id: RepoIdTuple) -> Self {
        let (owner, repo) = repo_id;
        Self { owner, repo }
    }
}

impl From<RepoId> for RepoIdTuple {
    fn from(repo_id: RepoId) -> Self {
        (repo_id.owner, repo_id.repo)
    }
}

const REPO_COMMIT_TABLE: MultimapTableDefinition<RepoIdTuple, Sha1Bytes> =
    MultimapTableDefinition::new("repo_commit_table");

const COMMIT_LOOKUP_TABLE: MultimapTableDefinition<Sha1Bytes, RepoIdTuple> =
    MultimapTableDefinition::new("commit_lookup_table");

pub struct Db {
    db: redb::Database,
}

impl Db {
    pub fn init(db_file: PathBuf) -> Result<Self> {
        let db = redb::Database::create(db_file)?;

        let tx = db.begin_write()?;
        {
            tx.open_multimap_table(REPO_COMMIT_TABLE)?;
            tx.open_multimap_table(COMMIT_LOOKUP_TABLE)?;
        }
        tx.commit()?;

        Ok(Self { db })
    }

    pub async fn add_commits_for_repo(
        &self,
        owner: String,
        repo: String,
        ctx: Arc<Context>,
    ) -> Result<()> {
        let Context { octocrab, .. } = &*ctx;

        let repo_id = RepoId::from((owner.clone(), repo.clone()));

        let mut page: u32 = 1;
        loop {
            let commits = octocrab
                .repos(&owner, &repo)
                .list_commits()
                .per_page(100)
                .page(page)
                .send()
                .await?;

            if matches!(commits.number_of_pages(), Some(0) | None) {
                break;
            } else {
                for commit in commits {
                    let sha1: [u8; 20] = hex::decode(commit.sha)?.as_slice().try_into()?;

                    self.insert_commit(repo_id.clone(), sha1)?;
                }
                page += 1;
            }
        }

        Ok(())
    }

    pub fn insert_commit(&self, repo_id: RepoId, sha1: Sha1Bytes) -> Result<()> {
        log::trace!(
            "insert_commit: {}/{} sha={}",
            repo_id.owner,
            repo_id.repo,
            hex::encode(sha1)
        );

        let tx = self.db.begin_write()?;
        {
            tx.open_multimap_table(REPO_COMMIT_TABLE)?
                .insert(RepoIdTuple::from(repo_id.clone()), sha1)?;
            tx.open_multimap_table(COMMIT_LOOKUP_TABLE)?
                .insert(sha1, RepoIdTuple::from(repo_id))?;
        }
        tx.commit()?;

        Ok(())
    }
}

#[derive(Tabled)]
pub struct CommitTableRow {
    pub owner: String,
    pub repo: String,
    pub sha1: String,
}

#[derive(Tabled)]
pub struct CidLookupTableRow {
    pub cid: String,
    pub owner: String,
    pub repo: String,
    pub commit: String,
}

impl Db {
    pub fn get_all_repo_commits(&self) -> Result<Vec<CommitTableRow>> {
        let mut rows = vec![];

        let tx = self.db.begin_read()?;
        {
            let commit_table = tx.open_multimap_table(REPO_COMMIT_TABLE)?;

            for entry in commit_table.iter()? {
                let (repo_id, sha1s) = entry?;

                let repo_id = RepoId::from(repo_id.value());

                for sha1 in sha1s {
                    let sha1 = sha1?.value();

                    rows.push(CommitTableRow {
                        owner: repo_id.owner.clone(),
                        repo: repo_id.repo.clone(),
                        sha1: hex::encode(sha1),
                    });
                }
            }
        }

        Ok(rows)
    }

    pub fn get_all_repo_commits_ascii_table(&self) -> Result<String> {
        let rows = self.get_all_repo_commits()?;

        let table = Table::new(rows)
            .with(Style::sharp())
            .with(Alignment::left())
            .to_string();

        Ok(table)
    }

    pub fn get_repos_with_commits_for_cid(&self, cid: &Cid) -> Result<Vec<RepoId>> {
        let mut repos = vec![];

        let sha1: Sha1Bytes = cid.hash().digest().try_into()?;

        let tx = self.db.begin_read()?;
        {
            let commit_lookup_table = tx.open_multimap_table(COMMIT_LOOKUP_TABLE)?;

            for entry in commit_lookup_table.get(sha1)? {
                repos.push(entry?.value().into());
            }
        }

        Ok(repos)
    }

    pub fn get_all_cid_lookups(&self) -> Result<Vec<CidLookupTableRow>> {
        let mut rows = vec![];

        let tx = self.db.begin_read()?;
        {
            let commit_table = tx.open_multimap_table(COMMIT_LOOKUP_TABLE)?;

            for entry in commit_table.iter()? {
                let (sha1, repo_ids) = entry?;

                let sha1 = sha1.value();

                let cid = {
                    let multihash = Multihash::wrap(0x12, &sha1)
                        .expect("unexpectedly failed to wrap a multihash");
                    Cid::new_v1(0x78, multihash)
                };

                for repo_id in repo_ids {
                    let commit = hex::encode(cid.hash().digest());
                    let cid = cid.to_string();
                    let RepoId { owner, repo } = repo_id?.value().into();

                    rows.push(CidLookupTableRow {
                        cid,
                        owner,
                        repo,
                        commit,
                    });
                }
            }
        }

        Ok(rows)
    }

    pub fn get_all_cid_lookups_ascii_table(&self) -> Result<String> {
        let rows = self.get_all_cid_lookups()?;

        let table = Table::new(rows)
            .with(Style::sharp())
            .with(Alignment::left())
            .to_string();

        Ok(table)
    }
}
