use std::{borrow::BorrowMut, process::Stdio};

use anyhow::Result;
use shakmaty::{fen::Fen, uci::Uci, Chess, Position};
use tokio::{
    io::{self, AsyncBufReadExt, AsyncWriteExt},
    process::Command,
    sync::broadcast::{Receiver, Sender},
};

type Broadcast = (Sender<Message>, Receiver<Message>);

#[derive(Debug, Clone)]
struct AnalyzeConfig {
    fen: String,
    depth: usize,
}

#[derive(Debug, Clone)]
struct BestMove {
    fen: String,
    best_move: String,
    depth: usize,
}

#[derive(Debug, Clone)]
enum Message {
    Uci,
    Ready,

    Kill,

    Analyze(AnalyzeConfig),
    BestMove(BestMove),
}

async fn stockfish((tx, mut rx): Broadcast) -> Result<()> {
    let mut child = Command::new("stockfish")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    let mut stdout = child
        .stdout
        .take()
        .expect("Could not get a handle on stdin");

    let mut stdin = child.stdin.take().unwrap();

    while let Ok(message) = rx.recv().await {
        match message {
            Message::Uci => {
                stdin.write_all(b"uci\n").await.unwrap();
                let reader = io::BufReader::new(stdout.borrow_mut());
                let mut lines = reader.lines();

                while let Some(line) = lines.next_line().await? {
                    if line.starts_with("uciok") {
                        tx.send(Message::Ready)?;
                        break;
                    }
                }
            }
            Message::Kill => {
                stdin.write_all(b"quit\n").await.unwrap();
            }
            Message::Analyze(config) => {
                stdin
                    .write_all(format!("position fen {}\n", config.fen).as_bytes())
                    .await
                    .unwrap();
                stdin
                    .write_all(format!("go depth {}\n", config.depth).as_bytes())
                    .await
                    .unwrap();

                let reader = io::BufReader::new(stdout.borrow_mut());
                let mut lines = reader.lines();

                while let Some(line) = lines.next_line().await? {
                    if line.starts_with("bestmove") {
                        let r = line.split(' ').collect::<Vec<&str>>();
                        match r.get(1) {
                            Some(best_move) => {
                                tx.send(Message::BestMove(BestMove {
                                    fen: config.fen,
                                    best_move: best_move.to_string(),
                                    depth: config.depth,
                                }))?;
                            }
                            None => {}
                        }
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}

async fn find_best_move((tx, rx): &mut Broadcast, fen: String, depth: usize) -> Result<BestMove> {
    tx.send(Message::Analyze(AnalyzeConfig {
        fen: fen.clone(),
        depth,
    }))?;

    while let Ok(message) = rx.recv().await {
        match message {
            Message::BestMove(best_move) => {
                if best_move.fen == fen {
                    return Ok(best_move);
                }
            }
            _ => {}
        }
    }

    todo!()
}

async fn worker(mut broadcast: Broadcast) -> Result<()> {
    let mut chess = Chess::new();

    let depth: usize = 1;

    while !chess.is_game_over() {
        let fen = Fen::from_position(chess.clone(), shakmaty::EnPassantMode::Legal);
        let best_move = find_best_move(&mut broadcast, fen.to_string(), depth).await?;
        let uci = best_move.best_move.parse::<Uci>()?;
        let m = uci.to_move(&chess)?;
        chess.play_unchecked(&m);
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let (tx, rx) = tokio::sync::broadcast::channel::<Message>(128);

    let stockfish_1 = tokio::spawn(stockfish((tx.clone(), rx)));

    let worker = tokio::spawn(worker((tx.clone(), tx.subscribe())));

    let _ = tokio::join!(worker, stockfish_1);

    Ok(())
}
