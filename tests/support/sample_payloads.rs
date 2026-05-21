#![allow(dead_code)]

use serde_json::{Value, json};

pub fn user_payload() -> Value {
    json!({
        "data": {
            "user": {
                "result": user_result("1001", "xscraper_dev", "XScraper Dev")
            }
        }
    })
}

pub fn tweet_payload() -> Value {
    json!({
        "data": {
            "threaded_conversation_with_injections_v2": {
                "instructions": [{
                    "entries": [{
                        "content": {
                            "itemContent": {
                                "tweet_results": {
                                    "result": tweet_result("2001", "Synthetic XScraper payload", "1001")
                                }
                            }
                        }
                    }]
                }]
            }
        }
    })
}

pub fn search_payload() -> Value {
    json!({
        "data": {
            "search_by_raw_query": {
                "search_timeline": {
                    "timeline": {
                        "instructions": [{
                            "entries": [
                                {
                                    "content": {
                                        "itemContent": {
                                            "tweet_results": {
                                                "result": tweet_result("2001", "Synthetic XScraper payload", "1001")
                                            }
                                        }
                                    }
                                },
                                {
                                    "content": {
                                        "itemContent": {
                                            "tweet_results": {
                                                "result": retweet_result()
                                            }
                                        }
                                    }
                                }
                            ]
                        }]
                    }
                }
            }
        }
    })
}

pub fn current_search_payload() -> Value {
    json!({
        "data": {
            "search_by_raw_query": {
                "search_timeline": {
                    "timeline": {
                        "instructions": [{
                            "entries": [{
                                "entryId": "tweet-3001",
                                "content": {
                                    "itemContent": {
                                        "tweet_results": {
                                            "result": {
                                                "__typename": "Tweet",
                                                "rest_id": "3001",
                                                "legacy": {
                                                    "created_at": "Sun May 17 20:47:51 +0000 2026",
                                                    "user_id_str": "4001",
                                                    "full_text": "Current X payload",
                                                    "lang": "en",
                                                    "reply_count": 0,
                                                    "retweet_count": 0,
                                                    "favorite_count": 1,
                                                    "quote_count": 0,
                                                    "bookmark_count": 0,
                                                    "conversation_id_str": "3001",
                                                    "entities": {"hashtags": [], "symbols": [], "user_mentions": []}
                                                },
                                                "core": {
                                                    "user_results": {
                                                        "result": current_user_result("4001", "current_user", "Current User")
                                                    }
                                                },
                                                "views": {"count": "12"}
                                            }
                                        }
                                    }
                                }
                            }]
                        }]
                    }
                }
            }
        }
    })
}

pub fn trend_payload() -> Value {
    json!({
        "data": {
            "viewer_v2": {
                "user_results": {
                    "result": {
                        "timeline": {
                            "timeline": {
                                "instructions": [{
                                    "entries": [{
                                        "content": {
                                            "__typename": "TimelineTimelineTrend",
                                            "trend": {
                                                "__typename": "TimelineTrend",
                                                "name": "XScraper",
                                                "rank": 1,
                                                "trend_url": {
                                                    "url": "twitter://search/?query=XScraper",
                                                    "urlType": "DeepLink",
                                                    "urtEndpointOptions": {
                                                        "requestParams": [{"key": "q", "value": "XScraper"}]
                                                    }
                                                },
                                                "trend_metadata": {
                                                    "domain_context": "Trending in Software",
                                                    "meta_description": "1,234 posts",
                                                    "url": {
                                                        "url": "twitter://search/?query=XScraper",
                                                        "urlType": "DeepLink",
                                                        "urtEndpointOptions": {
                                                            "requestParams": [{"key": "q", "value": "XScraper"}]
                                                        }
                                                    }
                                                },
                                                "grouped_trends": []
                                            }
                                        }
                                    }]
                                }]
                            }
                        }
                    }
                }
            }
        }
    })
}

pub fn list_payload() -> Value {
    json!({
        "data": {
            "list": list_result()
        }
    })
}

fn list_result() -> Value {
    json!({
        "__typename": "List",
        "id": "TGlzdDo1MDAx",
        "rest_id": "5001",
        "id_str": "5001",
        "name": "Rust Operators",
        "description": "A focused Rust engineering list",
        "mode": "public",
        "accessibility": "public",
        "slug": "rust-operators",
        "member_count": 42,
        "subscriber_count": 7,
        "following": true,
        "is_member": false,
        "muting": false,
        "pinning": false,
        "custom_banner_media_results": {
            "result": {
                "media_info": {
                    "original_img_url": "https://example.com/list-banner.jpg"
                }
            }
        },
        "owner_results": {
            "result": user_result("1001", "xscraper_dev", "XScraper Dev")
        }
    })
}

fn user_result(id: &str, username: &str, displayname: &str) -> Value {
    json!({
        "__typename": "User",
        "id": format!("VXNlcjo{id}"),
        "rest_id": id,
        "legacy": {
            "screen_name": username,
            "name": displayname,
            "description": "Synthetic account for XScraper tests",
            "created_at": "Mon Jan 02 03:04:05 +0000 2023",
            "followers_count": 42,
            "friends_count": 7,
            "statuses_count": 11,
            "favourites_count": 13,
            "listed_count": 2,
            "media_count": 3,
            "location": "Local",
            "profile_image_url_https": "https://example.com/avatar.jpg",
            "profile_banner_url": "https://example.com/banner.jpg",
            "protected": false,
            "verified": false,
            "is_blue_verified": true,
            "verified_type": "Business",
            "entities": {
                "description": {
                    "urls": [{
                        "expanded_url": "https://example.com",
                        "display_url": "example.com",
                        "url": "https://t.co/example"
                    }]
                }
            },
            "pinned_tweet_ids_str": ["2001"]
        }
    })
}

fn current_user_result(id: &str, username: &str, displayname: &str) -> Value {
    json!({
        "__typename": "User",
        "id": format!("VXNlcjo{id}"),
        "rest_id": id,
        "core": {
            "screen_name": username,
            "name": displayname,
            "created_at": "Thu Dec 20 21:22:10 +0000 2012"
        },
        "legacy": {
            "description": "Current profile",
            "followers_count": 42,
            "friends_count": 7,
            "statuses_count": 11,
            "favourites_count": 13,
            "listed_count": 2,
            "media_count": 3,
            "profile_banner_url": "https://example.com/current-banner.jpg",
            "entities": {
                "description": {"urls": []}
            },
            "pinned_tweet_ids_str": []
        },
        "avatar": {"image_url": "https://example.com/current-avatar.jpg"},
        "privacy": {"protected": false},
        "verification": {"verified": false},
        "profile_bio": {"description": "Current profile"},
        "location": {"location": "Current City"},
        "is_blue_verified": true
    })
}

fn tweet_result(id: &str, text: &str, user_id: &str) -> Value {
    json!({
        "__typename": "Tweet",
        "rest_id": id,
        "legacy": {
            "created_at": "Tue Feb 07 08:09:10 +0000 2023",
            "user_id_str": user_id,
            "full_text": text,
            "lang": "en",
            "reply_count": 1,
            "retweet_count": 2,
            "favorite_count": 3,
            "quote_count": 4,
            "bookmark_count": 5,
            "conversation_id_str": id,
            "entities": {
                "hashtags": [{"text": "XScraper"}],
                "symbols": [{"text": "RUST"}],
                "user_mentions": [{
                    "id_str": "1002",
                    "screen_name": "helper",
                    "name": "Helper Account"
                }],
                "urls": [{
                    "expanded_url": "https://example.com/article",
                    "display_url": "example.com/article",
                    "url": "https://t.co/article"
                }]
            },
            "extended_entities": {
                "media": [
                    {
                        "type": "photo",
                        "media_url_https": "https://example.com/photo.jpg"
                    },
                    {
                        "type": "video",
                        "media_url_https": "https://example.com/video.jpg",
                        "video_info": {
                            "duration_millis": 1000,
                            "variants": [{
                                "content_type": "video/mp4",
                                "bitrate": 832000,
                                "url": "https://example.com/video.mp4"
                            }]
                        },
                        "mediaStats": {"viewCount": "77"}
                    },
                    {
                        "type": "animated_gif",
                        "media_url_https": "https://example.com/gif.jpg",
                        "video_info": {
                            "variants": [{"url": "https://example.com/gif.mp4"}]
                        }
                    }
                ]
            },
            "card": {
                "legacy": {
                    "name": "summary",
                    "binding_values": [
                        {"key": "title", "value": {"type": "STRING", "string_value": "XScraper Card"}},
                        {"key": "description", "value": {"type": "STRING", "string_value": "Synthetic card"}},
                        {"key": "vanity_url", "value": {"type": "STRING", "string_value": "example.com"}},
                        {"key": "card_url", "value": {"type": "STRING", "string_value": "https://example.com/card"}},
                        {"key": "thumbnail", "value": {"type": "IMAGE", "image_value": {"height": 512, "url": "https://example.com/card.jpg"}}}
                    ]
                }
            },
            "source": "<a href=\"https://example.com/client\" rel=\"nofollow\">XScraper Test Client</a>"
        },
        "views": {"count": "99"},
        "core": {"user_results": {"result": user_result(user_id, "xscraper_dev", "XScraper Dev")}}
    })
}

fn retweet_result() -> Value {
    json!({
        "__typename": "Tweet",
        "rest_id": "2002",
        "legacy": {
            "created_at": "Tue Feb 07 08:10:10 +0000 2023",
            "user_id_str": "1001",
            "full_text": "RT @helper: Truncated…",
            "lang": "en",
            "reply_count": 0,
            "retweet_count": 1,
            "favorite_count": 1,
            "quote_count": 0,
            "conversation_id_str": "2002",
            "retweeted_status_id_str": "2003",
            "entities": {"hashtags": [], "symbols": [], "user_mentions": []}
        },
        "core": {"user_results": {"result": user_result("1001", "xscraper_dev", "XScraper Dev")}},
        "retweeted_status_result": {
            "result": tweet_result("2003", "Original helper post", "1002")
        }
    })
}
