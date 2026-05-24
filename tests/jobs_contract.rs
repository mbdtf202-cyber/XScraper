use xscraper::jobs::{JobCheckpointStore, JobItem};

#[test]
fn job_checkpoint_store_resumes_pending_items_without_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    let store = JobCheckpointStore::new(dir.path().join("jobs.db"));
    let job_id = store.create_job("search-batch").unwrap();

    store.enqueue_item(job_id, JobItem::new("SearchTimeline", "rust", None)).unwrap();
    store.enqueue_item(job_id, JobItem::new("SearchTimeline", "rust", None)).unwrap();
    store.mark_checkpoint(job_id, "SearchTimeline", Some("cursor-1"), Some("tweet-1")).unwrap();

    let pending = store.pending_items(job_id).unwrap();
    let checkpoint = store.checkpoint(job_id, "SearchTimeline").unwrap().unwrap();

    assert_eq!(pending.len(), 1);
    assert_eq!(checkpoint.cursor.as_deref(), Some("cursor-1"));
    assert_eq!(checkpoint.last_seen_id.as_deref(), Some("tweet-1"));
}
