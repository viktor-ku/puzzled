mod db;

use anyhow::Result;
use dotenvy::dotenv;
use shakmaty::{fen::Fen, uci::Uci, Chess, Outcome, Position};
use sqlx::PgPool;
use std::{borrow::BorrowMut, env, process::Stdio};
use tokio::{
    io::{self, AsyncBufReadExt, AsyncWriteExt},
    process::Command,
    sync::broadcast::{Receiver, Sender},
};

type Broadcast = (Sender<Message>, Receiver<Message>);

#[derive(Debug, Clone)]
pub struct AnalyzeConfig {
    fen: String,
    depth: usize,
}

#[derive(Debug, Clone)]
pub struct BestMove {
    pub fen: String,
    pub best_move: String,
    pub depth: usize,
}

#[derive(Debug, Clone)]
pub enum Message {
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

pub enum Winner {
    White,
    Black,
    Draw,
}

#[derive(Debug)]
pub struct Pgn {
    pub result: String,
    pub pgn: String,
}

impl Pgn {
    pub fn with(moves: Vec<String>, result: Outcome) -> Self {
        let mut pgn = String::new();
        let outcome_str = &result.to_string();

        for (i, pair) in moves.chunks_exact(2).enumerate() {
            let move_nr = i + 1;
            let white = pair.get(0).unwrap();
            let black = pair.get(1);

            let mut fullmove = format!("{}. {}", move_nr, white);
            if let Some(black) = black {
                fullmove.push_str(&format!(" {}", black));
            }

            pgn.push_str(&format!("{} ", fullmove));
        }

        pgn.push_str(outcome_str);

        Self {
            result: outcome_str.to_string(),
            pgn,
        }
    }
}

async fn worker(pool: PgPool, mut broadcast: Broadcast) -> Result<()> {
    let mut chess = Chess::new();

    let game_id = db::create_game(&pool).await?;

    let depth: usize = 1;

    let mut moves = Vec::<db::DbMove>::with_capacity(256);
    let mut nr = 1;

    while !chess.is_game_over() {
        let fen = Fen::from_position(chess.clone(), shakmaty::EnPassantMode::Legal);
        let best_move = find_best_move(&mut broadcast, fen.to_string(), depth).await?;

        let uci = best_move.best_move.parse::<Uci>()?;
        let m = uci.to_move(&chess)?;

        moves.push(db::DbMove {
            nr,
            uci: uci.to_string(),
        });
        nr += 1;

        chess.play_unchecked(&m);
    }

    db::save_moves(&pool, game_id, moves).await?;
    db::set_winner(&pool, game_id, chess.outcome()).await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv()?;

    let pool = PgPool::connect(&env::var("DATABASE_URL")?).await?;
    sqlx::query("SELECT now()").execute(&pool).await?;
    println!("Connection to db 🚀");

    let (tx, rx) = tokio::sync::broadcast::channel::<Message>(128);

    let stockfish_1 = tokio::spawn(stockfish((tx.clone(), rx)));

    let worker = tokio::spawn(worker(pool, (tx.clone(), tx.subscribe())));

    let _ = tokio::join!(worker, stockfish_1);

    Ok(())
}
