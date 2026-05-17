use std::sync::{Arc, Barrier, Mutex};
use std::thread;

use tempfile::tempdir;
use xscraper::pool::{AccountsPool, AddAccount};

#[test]
fn sqlite_account_store_tolerates_parallel_writers() {
    let dir = tempdir().unwrap();
    let pool = Arc::new(AccountsPool::new(dir.path().join("accounts.db")));

    for idx in 0..24 {
        let username = format!("user{idx:02}");
        pool.add_account(AddAccount {
            username: username.clone(),
            password: "pass".into(),
            email: format!("{username}@example.com"),
            email_password: "mail-pass".into(),
            cookies: Some("ct0=csrf; auth_token=token".into()),
            ..AddAccount::default()
        })
        .unwrap();
    }

    let start = Arc::new(Barrier::new(24));
    let locked = Arc::new(Barrier::new(24));
    let usernames = Arc::new(Mutex::new(Vec::new()));

    let handles = (0..24)
        .map(|_| {
            let pool = Arc::clone(&pool);
            let start = Arc::clone(&start);
            let locked = Arc::clone(&locked);
            let usernames = Arc::clone(&usernames);
            thread::spawn(move || {
                start.wait();
                let account = pool.get_for_queue("SearchTimeline").unwrap().unwrap();
                usernames.lock().unwrap().push(account.username.clone());
                locked.wait();
                pool.unlock(&account.username, "SearchTimeline", 1).unwrap();
            })
        })
        .collect::<Vec<_>>();

    for handle in handles {
        handle.join().unwrap();
    }

    let accounts = pool.get_all().unwrap();
    assert_eq!(accounts.len(), 24);
    let usernames = Arc::try_unwrap(usernames).unwrap().into_inner().unwrap();
    let unique = usernames.iter().collect::<std::collections::HashSet<_>>();
    assert_eq!(unique.len(), 24);
    assert!(accounts.iter().all(|account| account.stats["SearchTimeline"] == 1));
}
