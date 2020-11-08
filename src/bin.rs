use std::io::{BufRead, Write};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    loop {
        println!("Enter the anime you want to search for");

        let stdin = std::io::stdin();

        let query = {
            std::io::stdout().flush().unwrap();
            let mut input = String::new();
            stdin.lock().read_line(&mut input)?;
            input.trim().to_owned()
        };

        let series = gogoanime::search(&client, &query).await?;

        println!("Result: ");
        for (n, entry) in series.iter().enumerate() {
            println!("{}. {}", n, entry.name);
        }

        print!("Enter the number of the series you wish to browse: ");

        let target_series = {
            let n = {
                std::io::stdout().flush().unwrap();
                let mut input = String::new();
                stdin.lock().read_line(&mut input)?;
                input.trim().to_owned()
            }
            .parse::<usize>()?;

            &series[n]
        };

        let episodes = gogoanime::get_episodes(&client, &target_series.id).await?;

        println!("Found {} episodes for {}", episodes.len(), target_series.id);

        for (episode_number, episode) in episodes.iter().enumerate() {
            let video_sources = gogoanime::get_video(&client, &episode).await?;
            let video_sources = video_sources
                .into_iter()
                .map(|o| o.to_string())
                .collect::<Vec<String>>();
            println!("Episode {} -> {:?}", episode_number, video_sources);
        }
    }
}
