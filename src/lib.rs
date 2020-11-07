use reqwest::{self, Url};
use scraper::{Html, Selector};
use std::ops::Range;

pub async fn get_video(
    client: &reqwest::Client,
    category: &str,
    ep_num: u32,
) -> anyhow::Result<Vec<Url>> {
    let base_url = Url::parse("https://gogoanime.so")?;
    let video_iframe_url = base_url.join({
        let resp = client
            .get(base_url.join(&format!("{}-episode-{}", category, ep_num))?)
            .send()
            .await?;

        let body = resp.text().await?;
        let fragment = Html::parse_document(&body);

        let selector = Selector::parse(".play-video > iframe").unwrap();

        &fragment
            .select(&selector)
            .next()
            .unwrap()
            .value()
            .attr("src")
            .unwrap()
            .to_owned()
    })?;

    // the clone and set_path just replaces the path, keeps the query data
    let mut video_request_url = video_iframe_url.clone();
    video_request_url.set_path("/ajax.php");

    let resp = client.get(video_request_url).send().await?;

    let content = resp.text().await?;
    let data = json::parse(&content)?;

    Ok(match &data["source"] {
        json::JsonValue::Array(sources) => {
            let futures = sources.iter().map(|o| {
                let file = o["file"].as_str().unwrap().to_owned();
                client.get(&file).send()
            });
            futures::executor::block_on(futures::future::join_all(futures))
                .into_iter()
                .map(|o| o.map(|o| o.url().to_owned()))
                .collect::<Result<Vec<_>, _>>()?
        }
        _ => Vec::new(),
    })
}

pub struct SearchResultEntry {
    pub id: String,
    pub name: String,
}

pub async fn search(
    client: &reqwest::Client,
    query: &str,
) -> anyhow::Result<Vec<SearchResultEntry>> {
    let base_url = Url::parse("https://gogoanime.so")?;

    let resp = client
        .get(base_url.join(&format!("/search.html&keyword={}", query))?)
        .send()
        .await?;

    let body = resp.text().await?;
    let fragment = Html::parse_document(&body);

    let selector = Selector::parse(".last_episodes > ul > li .img a").unwrap();

    Ok(fragment
        .select(&selector)
        .filter_map(|e| {
            let id = e.value().attr("href").map(|s| s.replace("/category/", ""));
            let name = e.value().attr("title").map(|s| s.to_owned());
            match (id, name) {
                (Some(id), Some(name)) => Some(SearchResultEntry { id, name }),
                _ => None,
            }
        })
        .collect::<Vec<_>>())
}

pub async fn get_episodes_range(
    client: &reqwest::Client,
    series_id: &str,
) -> anyhow::Result<Range<u32>> {
    let base_url = Url::parse("https://gogoanime.so")?;

    let resp = client
        .get(base_url.join(&format!("/category/{}", series_id))?)
        .send()
        .await?;

    let body = resp.text().await?;
    let fragment = Html::parse_document(&body);

    let selector = Selector::parse("#episode_page a.active").unwrap();

    let elem = fragment.select(&selector).next().unwrap().value();

    // ep_start starts at 0 for some reason so add 1 and range upper is exclusive so add 1 there too
    Ok(elem.attr("ep_start").unwrap().parse::<u32>()? + 1
        ..elem.attr("ep_end").unwrap().parse::<u32>()? + 1)
}
