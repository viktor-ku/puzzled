use shakmaty::Outcome;
use sqlx::PgPool;
use uuid::Uuid;
use anyhow::{Result, bail};

#[derive(Debug)]
pub struct DbMove {
    pub nr: i16,
    pub uci: String,
}

pub async fn save_moves(pool: &PgPool, game_id: Uuid, moves: Vec<DbMove>) -> Result<()> {
    let vec_nr: Vec<i16> = moves.iter().map(|x| x.nr).collect();
    let vec_uci: Vec<String> = moves.iter().map(|x| x.uci.to_string()).collect();
    let vec_game = [game_id].repeat(moves.len());

    sqlx::query!(
        r#"
INSERT INTO moves (nr, uci, game_id) 
SELECT * FROM UNNEST($1::smallint[], $2::text[], $3::uuid[])
        "#,
        &vec_nr[..],
        &vec_uci[..],
        &vec_game[..],
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn create_game(pool: &PgPool) -> Result<Uuid> {
    let rec = sqlx::query!(
        r#"
INSERT INTO games (winner)
VALUES (NULL)
RETURNING id
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(rec.id)
}

pub async fn set_winner(pool: &PgPool, game_id: Uuid, outcome: Option<Outcome>) -> Result<()> {
    let winner = outcome.map(|val| match val {
        Outcome::Draw => 0,
        Outcome::Decisive { winner } => match winner {
            shakmaty::Color::White => 1,
            shakmaty::Color::Black => -1,
        },
    });

    match winner {
        Some(winner) => {
            sqlx::query!(
                r#"
UPDATE games
    SET winner = $1
    WHERE id = $2
    "#,
                winner,
                game_id
            )
            .execute(pool)
            .await?;
        }
        None => bail!("No winner yet?"),
    };

    Ok(())
}
