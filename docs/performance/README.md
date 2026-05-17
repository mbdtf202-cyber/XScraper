# Performance Notes

XScraper measures parser throughput with Criterion using synthetic GraphQL payloads from `tests/support/`.

Run the benchmark:

```bash
cargo bench --bench parser_bench
```

The benchmark is intentionally parser-only. Live scraping performance depends on X rate limits, account health, proxy latency, and cursor depth, so network benchmarks are not stable enough to publish as a fair local performance signal.

Expected shape on the same machine:

| Workload | Expected bottleneck | Notes |
| --- | --- | --- |
| synthetic search payload to `Tweet` models | JSON traversal and typed model construction | release builds are the only meaningful benchmark mode |
| synthetic user payload to `User` models | small-payload overhead | small payloads are dominated by traversal and allocation |
| Live GraphQL pages | account/rate-limit bound | language speed is usually hidden by network and X server latency |

Record local numbers here before publishing release claims:

```text
parse_search_tweets: <criterion mean>
parse_user_by_login: <criterion mean>
```
