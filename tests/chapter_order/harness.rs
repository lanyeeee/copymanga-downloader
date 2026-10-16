use serde::Deserialize;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::Instrument;

// Only fields consumed by aggregation or the regression assertion are projected.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ChapterInGetChaptersRespData {
    uuid: String,
    name: String,
    ordered: i64,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct GetChaptersRespData {
    list: Vec<ChapterInGetChaptersRespData>,
    total: i64,
    offset: i64,
}

#[derive(Clone)]
struct CopyClient {
    pages: Arc<Vec<GetChaptersRespData>>,
    reverse_completion: bool,
    fail_offset: Option<i64>,
}

impl CopyClient {
    /* PRODUCTION_METHOD */

    // Replace only the HTTP boundary with synthetic successful pages.
    // The task scheduling, JoinSet, and append logic above remain production code.
    async fn get_chapters(
        &self,
        comic: &str,
        group: &str,
        limit: i64,
        offset: i64,
    ) -> eyre::Result<GetChaptersRespData> {
        assert_eq!(comic, "nvpengyoujiewoyixia");
        assert_eq!(group, "default");
        assert_eq!(limit, 100);
        if self.fail_offset == Some(offset) {
            eyre::bail!("page {offset} failed");
        }
        let total = self.pages[0].total;
        let page = self
            .pages
            .iter()
            .find(|p| p.offset == offset)
            .cloned()
            .unwrap_or_else(|| {
                assert_eq!(offset, total, "unexpected page offset");
                GetChaptersRespData {
                    list: vec![],
                    total,
                    offset,
                }
            });
        if offset > 0 {
            let last_page = (page.total / 100) as u64;
            let rank = if self.reverse_completion {
                last_page + 1 - offset as u64 / 100
            } else {
                offset as u64 / 100
            };
            tokio::time::sleep(std::time::Duration::from_millis(rank * 100)).await;
        }
        eprintln!(
            "completed offset={offset}, first={:?}, last={:?}",
            page.list.first().map(|c| &c.name),
            page.list.last().map(|c| &c.name)
        );
        Ok(page)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn check(total: i64, reverse_completion: bool) {
        let mut pages: Vec<GetChaptersRespData> = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/pages.json"
        )))
        .unwrap();
        pages.retain(|p| p.offset == 0 || p.offset < total);
        for page in &mut pages {
            page.total = total;
            page.list
                .truncate((total - page.offset).clamp(0, 100) as usize);
        }
        let mut expected: Vec<_> = pages.iter().flat_map(|p| p.list.clone()).collect();
        expected.sort_by_key(|c| c.ordered);
        let client = CopyClient {
            pages: Arc::new(pages),
            reverse_completion,
            fail_offset: None,
        };
        let actual = client
            .get_group_chapters("nvpengyoujiewoyixia", "default")
            .await
            .unwrap();
        assert_eq!(actual.len(), expected.len(), "must not lose chapters");
        let mut actual_uuids: Vec<_> = actual.iter().map(|c| &c.uuid).collect();
        let mut expected_uuids: Vec<_> = expected.iter().map(|c| &c.uuid).collect();
        actual_uuids.sort();
        expected_uuids.sort();
        assert_eq!(
            actual_uuids, expected_uuids,
            "chapter identity set must be unchanged"
        );
        for chapter in &actual {
            assert_eq!(
                Some(chapter),
                expected.iter().find(|c| c.uuid == chapter.uuid),
                "chapter metadata must be unchanged"
            );
        }
        if let Some((i, (a, b))) = actual
            .iter()
            .zip(&expected)
            .enumerate()
            .find(|(_, (a, b))| a.uuid != b.uuid)
        {
            eprintln!("first order mismatch position={i}: actual={} (ordered={}), expected={} (ordered={})",
                a.name, a.ordered, b.name, b.ordered);
            eprintln!("previous={:?}", i.checked_sub(1).map(|j| &actual[j].name));
        }
        assert_eq!(actual.iter().map(|c| &c.uuid).collect::<Vec<_>>(),
            expected.iter().map(|c| &c.uuid).collect::<Vec<_>>(),
            "chapter display sequence must follow numeric ordered metadata regardless of page completion order");
    }

    #[tokio::test]
    async fn minimal_three_pages_reverse_completion() {
        check(201, true).await;
    }

    #[tokio::test]
    async fn full_group_reverse_completion() {
        check(447, true).await;
    }

    #[tokio::test]
    async fn ordered_completion_control() {
        check(447, false).await;
    }

    #[tokio::test]
    async fn single_page_control() {
        check(99, true).await;
    }

    #[tokio::test]
    async fn empty_group() {
        check(0, true).await;
    }

    #[tokio::test]
    async fn exact_page_boundary() {
        check(200, true).await;
    }

    #[tokio::test]
    async fn first_page_error() {
        check_error(0).await;
    }

    #[tokio::test]
    async fn remaining_page_error() {
        check_error(100).await;
    }

    async fn check_error(offset: i64) {
        let pages = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/pages.json"
        )))
        .unwrap();
        let client = CopyClient {
            pages: Arc::new(pages),
            reverse_completion: true,
            fail_offset: Some(offset),
        };
        let error = client
            .get_group_chapters("nvpengyoujiewoyixia", "default")
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), format!("page {offset} failed"));
    }
}
