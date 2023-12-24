
/**
 * @param { import("knex").Knex } knex
 * @returns { Promise<void> }
 */
export const up = async (knex) => {
  await knex.schema.alterTable('games', t => {
    t.setNullable('winner')
    t.dropColumn('pgn')
  })
};

/**
 * @param { import("knex").Knex } knex
 * @returns { Promise<void> }
 */
export const down = async (knex) => {
  await knex.schema.alterTable('games', t => {
    t.setNullable('winner')
    t.text('pgn')
  })
};
