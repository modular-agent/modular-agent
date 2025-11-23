use std::{io::stdin, path::PathBuf};

use agent_stream_kit::{ASKit, ASKitEvent, ASKitObserver, AgentData, AgentFlow};

#[tokio::main]
async fn main() {
    // Initialize ASKit
    let askit = ASKit::init().unwrap();

    // Subscribe events
    askit.subscribe(Box::new(BoardObserver));

    // Import board flow
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("examples");
    path.push("flows");
    path.push("board.json");
    let json = std::fs::read_to_string(path).unwrap();
    let flow = AgentFlow::from_json(&json).unwrap();
    askit.add_agent_flow(&flow).unwrap();

    // Run the agent flow
    askit.ready().await.unwrap();

    loop {
        println!("Enter input: ");
        let mut input = String::new();
        stdin().read_line(&mut input).unwrap();

        if input.trim() == "quit" {
            break;
        }

        askit
            .write_board_data("user input".to_string(), AgentData::string(input.trim()))
            .unwrap();
    }

    // Quit ASKit
    askit.quit();
}

struct BoardObserver;

impl ASKitObserver for BoardObserver {
    fn notify(&self, event: &ASKitEvent) {
        match event {
            ASKitEvent::Board(name, data) => {
                println!("Board: {}: {:?}", name, data);
            }
            _ => {
                dbg!(event);
            }
        }
    }
}
