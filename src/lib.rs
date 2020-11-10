use reqwest::{self, Url};
use scraper::{Html, Selector};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GetVideoError {
    #[error("Video not found")]
    NotFound(String),
    #[error("Failed to send get request")]
    SendGetRequest,
    #[error("Failed to get the text from the request")]
    RequestText,
    #[error("Failed to create a url")]
    CreateUrl,
    #[error("Failed to parse the json")]
    ParseJson,
}

pub async fn get_video(client: &reqwest::Client, episode: &str) -> Result<Vec<Url>, GetVideoError> {
    let base_url = Url::parse("https://gogoanime.so").map_err(|_| GetVideoError::CreateUrl)?;
    let video_iframe_suffix: Result<String, GetVideoError> = {
        let resp = client
            .get(
                base_url
                    .join(episode)
                    .map_err(|_| GetVideoError::CreateUrl)?,
            )
            .send()
            .await
            .map_err(|_| GetVideoError::SendGetRequest)?;

        let url = resp.url().to_string();

        let body = resp.text().await.map_err(|_| GetVideoError::RequestText)?;
        let fragment = Html::parse_document(&body);

        let selector = Selector::parse(".play-video > iframe").unwrap();

        let iframe = match (&mut fragment.select(&selector)).next() {
            Some(o) => o,
            None => return Err(GetVideoError::NotFound(url)),
        };

        Ok(iframe.value().attr("src").unwrap().to_owned())
    };
    let video_iframe_url = base_url
        .join(&video_iframe_suffix?)
        .map_err(|_| GetVideoError::CreateUrl)?;

    // the clone and set_path just replaces the path, keeps the query data
    let mut video_request_url = video_iframe_url.clone();
    video_request_url.set_path("/ajax.php");

    let resp = client
        .get(video_request_url)
        .send()
        .await
        .map_err(|_| GetVideoError::SendGetRequest)?;

    let content = resp.text().await.map_err(|_| GetVideoError::RequestText)?;
    let data = json::parse(&content).map_err(|_| GetVideoError::ParseJson)?;

    Ok(match &data["source"] {
        json::JsonValue::Array(sources) => {
            let futures = sources.iter().map(|o| {
                let file = o["file"].as_str().unwrap().to_owned();
                client.get(&file).send()
            });
            futures::executor::block_on(futures::future::join_all(futures))
                .into_iter()
                .map(|o| o.map(|o| o.url().to_owned()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| GetVideoError::SendGetRequest)?
        }
        _ => Vec::new(),
    })
}

pub struct SearchResultEntry {
    pub id: String,
    pub name: String,
}

#[derive(Error, Debug)]
pub enum SearchError {
    #[error("Failed to send get request")]
    SendGetRequest,
    #[error("Failed to get the text from the request")]
    RequestText,
    #[error("Failed to create a url")]
    CreateUrl,
}

pub async fn search(
    client: &reqwest::Client,
    query: &str,
) -> Result<Vec<SearchResultEntry>, SearchError> {
    let base_url = Url::parse("https://gogoanime.so").map_err(|_| SearchError::CreateUrl)?;

    let resp = client
        .get(
            base_url
                .join(&format!("/search.html&keyword={}", query))
                .map_err(|_| SearchError::CreateUrl)?,
        )
        .send()
        .await
        .map_err(|_| SearchError::SendGetRequest)?;

    let body = resp.text().await.map_err(|_| SearchError::RequestText)?;
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

#[derive(Error, Debug)]
pub enum GetEpisodesError {
    #[error("Failed to send get request")]
    SendGetRequest,
    #[error("Failed to get the text from the request")]
    RequestText,
    #[error("Failed to create a url")]
    CreateUrl,
}

pub async fn get_episodes(
    client: &reqwest::Client,
    series_id: &str,
) -> Result<Vec<String>, GetEpisodesError> {
    let base_url = Url::parse("https://gogoanime.so").map_err(|_| GetEpisodesError::CreateUrl)?;

    let fragment = {
        let resp = client
            .get(
                base_url
                    .join(&format!("/category/{}", series_id))
                    .map_err(|_| GetEpisodesError::CreateUrl)?,
            )
            .send()
            .await
            .map_err(|_| GetEpisodesError::SendGetRequest)?;

        let body = resp
            .text()
            .await
            .map_err(|_| GetEpisodesError::RequestText)?;
        Html::parse_document(&body)
    };

    let elem = fragment
        .select(&Selector::parse("#episode_page a.active").unwrap())
        .next()
        .unwrap()
        .value();

    let ep_start = elem.attr("ep_start").unwrap();
    let ep_end = elem.attr("ep_end").unwrap();

    let selector = Selector::parse("input#movie_id").unwrap();
    let id = fragment
        .select(&selector)
        .next()
        .unwrap()
        .value()
        .attr("value")
        .unwrap();

    let list_fragment = {
        let resp = client
            .get(&format!(
                "https://ajax.apimovie.xyz/ajax/load-list-episode?ep_start={}&ep_end={}&id={}",
                ep_start, ep_end, id
            ))
            .send()
            .await
            .map_err(|_| GetEpisodesError::SendGetRequest)?;

        let body = resp
            .text()
            .await
            .map_err(|_| GetEpisodesError::RequestText)?;
        Html::parse_document(&body)
    };

    let selector = Selector::parse("#episode_related > li > a").unwrap();

    Ok(list_fragment
        .select(&selector)
        .map(|e| e.value().attr("href").unwrap_or(""))
        .map(|s| s.to_owned())
        .collect::<Vec<_>>())
}
