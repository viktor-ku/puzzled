mod db;

use anyhow::Result;
use dotenvy::dotenv;
use shakmaty::{fen::Fen, uci::Uci, Chess, Position};
use sqlx::PgPool;
use std::{borrow::BorrowMut, env, process::Stdio};
use tokio::{
    io::{self, AsyncBufReadExt, AsyncWriteExt},
    process::Command,
    sync::{mpsc, oneshot},
};

#[derive(Debug)]
pub struct Analyze {
    pub fen: String,
    pub depth: usize,
    pub response: oneshot::Sender<BestMove>,
}

#[derive(Debug, Clone)]
pub struct BestMove {
    pub fen: String,
    pub best_move: String,
    pub depth: usize,
}

#[derive(Debug)]
pub enum StockfishCmd {
    Uci,
    Ready,

    Kill,

    Analyze(Analyze),
    BestMove(BestMove),
}

async fn stockfish(mut stockfish_rx: mpsc::Receiver<StockfishCmd>) -> Result<()> {
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

    while let Some(cmd) = stockfish_rx.recv().await {
        match cmd {
            StockfishCmd::Uci => {
                stdin.write_all(b"uci\n").await.unwrap();
                let reader = io::BufReader::new(stdout.borrow_mut());
                let mut lines = reader.lines();

                while let Some(line) = lines.next_line().await? {
                    if line.starts_with("uciok") {
                        // tx.send(Message::Ready)?;
                        break;
                    }
                }
            }
            StockfishCmd::Kill => {
                stdin.write_all(b"quit\n").await.unwrap();
            }
            StockfishCmd::Analyze(analyze) => {
                stdin
                    .write_all(format!("position fen {}\n", analyze.fen).as_bytes())
                    .await
                    .unwrap();
                stdin
                    .write_all(format!("go depth {}\n", analyze.depth).as_bytes())
                    .await
                    .unwrap();

                let reader = io::BufReader::new(stdout.borrow_mut());
                let mut lines = reader.lines();

                while let Some(line) = lines.next_line().await? {
                    if line.starts_with("bestmove") {
                        let r = line.split(' ').collect::<Vec<&str>>();
                        match r.get(1) {
                            Some(best_move) => {
                                let _ = analyze.response.send(BestMove {
                                    fen: analyze.fen,
                                    best_move: best_move.to_string(),
                                    depth: analyze.depth,
                                });
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

async fn find_best_move(
    tx: mpsc::Sender<StockfishCmd>,
    fen: String,
    depth: usize,
) -> Result<BestMove> {
    let (response_tx, response_rx) = oneshot::channel::<BestMove>();

    tx.send(StockfishCmd::Analyze(Analyze {
        fen: fen.clone(),
        depth,
        response: response_tx,
    }))
    .await?;

    Ok(response_rx.await?)
}

async fn worker(pool: PgPool, tx: mpsc::Sender<StockfishCmd>) -> Result<()> {
    let mut chess = Chess::new();

    let game_id = db::create_game(&pool).await?;

    let depth: usize = 1;

    let mut moves = Vec::<db::DbMove>::with_capacity(256);
    let mut nr = 1;

    while !chess.is_game_over() {
        let fen = Fen::from_position(chess.clone(), shakmaty::EnPassantMode::Legal);
        let best_move = find_best_move(tx.clone(), fen.to_string(), depth).await?;

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

    let (stockfish_tx, stockfish_rx) = tokio::sync::mpsc::channel::<StockfishCmd>(10);

    let stockfish_1 = tokio::spawn(stockfish(stockfish_rx));

    let worker = tokio::spawn(worker(pool, stockfish_tx.clone()));

    let _ = tokio::join!(worker, stockfish_1);

    Ok(())
}
